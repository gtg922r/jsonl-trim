use anyhow::{Context, Result};
use clap::Parser;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "jsonl-trim")]
#[command(about = "Trim oversized lines and string fields in JSONL files")]
#[command(version)]
struct Args {
    /// File or directory to process
    path: PathBuf,

    /// Glob pattern for files (used with directory input)
    #[arg(short, long, default_value = "*.jsonl")]
    pattern: String,

    /// Maximum bytes per line (truncates entire line)
    #[arg(long)]
    line_max: Option<usize>,

    /// Maximum bytes per string field (preserves JSON structure)
    #[arg(long)]
    string_max: Option<usize>,

    /// Show detailed output
    #[arg(short, long)]
    verbose: bool,

    /// Preview changes without writing
    #[arg(long)]
    dry_run: bool,
}

#[derive(Default)]
struct FileStats {
    original_bytes: usize,
    final_bytes: usize,
    lines_truncated: usize,
    strings_truncated: usize,
}

#[derive(Default)]
struct TotalStats {
    files_processed: usize,
    files_modified: usize,
    bytes_saved: usize,
}

fn main() -> Result<()> {
    let args = Args::parse();

    if args.line_max.is_none() && args.string_max.is_none() {
        eprintln!("Error: Must specify at least one of --line-max or --string-max");
        std::process::exit(1);
    }

    let files = collect_files(&args.path, &args.pattern)?;

    if files.is_empty() {
        eprintln!("No files matched");
        std::process::exit(1);
    }

    let mut total = TotalStats::default();

    for file in &files {
        match process_file(file, &args) {
            Ok(stats) => {
                total.files_processed += 1;
                if stats.original_bytes != stats.final_bytes {
                    total.files_modified += 1;
                    total.bytes_saved += stats.original_bytes - stats.final_bytes;
                }

                if args.verbose {
                    print_file_stats(file, &stats);
                }
            }
            Err(e) => {
                eprintln!("Error processing {}: {}", file.display(), e);
            }
        }
    }

    print_summary(&total, args.dry_run);
    Ok(())
}

fn collect_files(path: &Path, pattern: &str) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if path.is_file() {
        files.push(path.to_path_buf());
    } else if path.is_dir() {
        let glob_pattern = glob::Pattern::new(pattern).context("Invalid glob pattern")?;

        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            if entry_path.is_file()
                && let Some(name) = entry_path.file_name().and_then(|n| n.to_str())
                && glob_pattern.matches(name)
            {
                files.push(entry_path.to_path_buf());
            }
        }
    } else {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    Ok(files)
}

fn process_file(path: &Path, args: &Args) -> Result<FileStats> {
    let content = fs::read_to_string(path).context("Failed to read file")?;

    let mut stats = FileStats {
        original_bytes: content.len(),
        ..Default::default()
    };

    let mut output_lines = Vec::new();

    for line in content.lines() {
        let mut processed_line = line.to_string();

        // String mode first (if enabled)
        if let Some(max_string) = args.string_max
            && let Ok(mut json) = serde_json::from_str::<Value>(&processed_line)
        {
            let count = truncate_strings(&mut json, max_string);
            if count > 0 {
                stats.strings_truncated += count;
                processed_line = serde_json::to_string(&json).unwrap_or(processed_line);
            }
        }

        // Line mode second (if enabled)
        if let Some(max_line) = args.line_max
            && processed_line.len() > max_line
        {
            let original_len = processed_line.len();
            processed_line.truncate(max_line.saturating_sub(50));
            processed_line.push_str(&format!("... [line truncated: {} bytes]", original_len));
            stats.lines_truncated += 1;
        }

        output_lines.push(processed_line);
    }

    let output = output_lines.join("\n");
    // Preserve trailing newline if original had one
    let output = if content.ends_with('\n') {
        format!("{}\n", output)
    } else {
        output
    };

    stats.final_bytes = output.len();

    if !args.dry_run && stats.original_bytes != stats.final_bytes {
        fs::write(path, &output).context("Failed to write file")?;
    }

    Ok(stats)
}

fn truncate_strings(value: &mut Value, max_bytes: usize) -> usize {
    let mut count = 0;
    truncate_strings_recursive(value, max_bytes, &mut count);
    count
}

fn truncate_strings_recursive(value: &mut Value, max_bytes: usize, count: &mut usize) {
    match value {
        Value::String(s) => {
            if s.len() > max_bytes {
                let original_len = s.len();
                *s = format!("[truncated: {} bytes]", original_len);
                *count += 1;
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                truncate_strings_recursive(item, max_bytes, count);
            }
        }
        Value::Object(obj) => {
            for (_, v) in obj.iter_mut() {
                truncate_strings_recursive(v, max_bytes, count);
            }
        }
        _ => {}
    }
}

fn print_file_stats(path: &Path, stats: &FileStats) {
    if stats.original_bytes == stats.final_bytes {
        println!("{}: unchanged", path.display());
    } else {
        let orig_kb = stats.original_bytes as f64 / 1024.0;
        let final_kb = stats.final_bytes as f64 / 1024.0;
        println!(
            "{}: {:.1}KB → {:.1}KB (trimmed {} lines, {} strings)",
            path.display(),
            orig_kb,
            final_kb,
            stats.lines_truncated,
            stats.strings_truncated
        );
    }
}

fn print_summary(total: &TotalStats, dry_run: bool) {
    let prefix = if dry_run { "[DRY RUN] " } else { "" };
    let saved_kb = total.bytes_saved as f64 / 1024.0;

    if total.files_modified == 0 {
        println!(
            "{}No files needed trimming ({} files checked)",
            prefix, total.files_processed
        );
    } else {
        println!(
            "{}Total: {} files modified, {:.1}KB saved",
            prefix, total.files_modified, saved_kb
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    // ==================== String Truncation Tests ====================

    #[test]
    fn test_truncate_simple_string() {
        let mut value = json!({"key": "x".repeat(100)});
        let count = truncate_strings(&mut value, 50);

        assert_eq!(count, 1);
        assert_eq!(value["key"], "[truncated: 100 bytes]");
    }

    #[test]
    fn test_truncate_preserves_short_strings() {
        let mut value = json!({"key": "hello"});
        let count = truncate_strings(&mut value, 50);

        assert_eq!(count, 0);
        assert_eq!(value["key"], "hello");
    }

    #[test]
    fn test_truncate_nested_object() {
        let mut value = json!({
            "outer": {
                "inner": {
                    "data": "x".repeat(100)
                }
            }
        });
        let count = truncate_strings(&mut value, 50);

        assert_eq!(count, 1);
        assert_eq!(value["outer"]["inner"]["data"], "[truncated: 100 bytes]");
    }

    #[test]
    fn test_truncate_array_of_strings() {
        let mut value = json!({
            "items": ["short", "x".repeat(100), "also short", "y".repeat(200)]
        });
        let count = truncate_strings(&mut value, 50);

        assert_eq!(count, 2);
        assert_eq!(value["items"][0], "short");
        assert_eq!(value["items"][1], "[truncated: 100 bytes]");
        assert_eq!(value["items"][2], "also short");
        assert_eq!(value["items"][3], "[truncated: 200 bytes]");
    }

    #[test]
    fn test_truncate_mixed_types() {
        let mut value = json!({
            "string": "x".repeat(100),
            "number": 42,
            "bool": true,
            "null": null,
            "nested": {"big": "y".repeat(100)}
        });
        let count = truncate_strings(&mut value, 50);

        assert_eq!(count, 2);
        assert_eq!(value["number"], 42);
        assert_eq!(value["bool"], true);
        assert!(value["null"].is_null());
    }

    #[test]
    fn test_truncate_empty_structures() {
        let mut value = json!({"empty_obj": {}, "empty_arr": []});
        let count = truncate_strings(&mut value, 50);

        assert_eq!(count, 0);
    }

    #[test]
    fn test_truncate_boundary_exactly_at_limit() {
        let mut value = json!({"key": "x".repeat(50)});
        let count = truncate_strings(&mut value, 50);

        // Exactly at limit should NOT be truncated
        assert_eq!(count, 0);
        assert_eq!(value["key"], "x".repeat(50));
    }

    #[test]
    fn test_truncate_boundary_one_over_limit() {
        let mut value = json!({"key": "x".repeat(51)});
        let count = truncate_strings(&mut value, 50);

        // One over limit SHOULD be truncated
        assert_eq!(count, 1);
        assert_eq!(value["key"], "[truncated: 51 bytes]");
    }

    #[test]
    fn test_json_remains_valid_after_truncation() {
        let mut value = json!({
            "content": "x".repeat(10000),
            "metadata": {"size": 10000}
        });
        truncate_strings(&mut value, 100);

        // Should be able to serialize back to valid JSON
        let serialized = serde_json::to_string(&value);
        assert!(serialized.is_ok());

        // And parse it back
        let parsed: Result<Value, _> = serde_json::from_str(&serialized.unwrap());
        assert!(parsed.is_ok());
    }

    // ==================== Line Truncation Tests ====================

    #[test]
    fn test_line_truncation() {
        let long_line = "x".repeat(1000);
        let max_line: usize = 100;

        let mut line = long_line.clone();
        let original_len = line.len();
        line.truncate(max_line.saturating_sub(50));
        line.push_str(&format!("... [line truncated: {} bytes]", original_len));

        assert!(line.len() <= max_line + 30); // Allow for marker text
        assert!(line.contains("[line truncated: 1000 bytes]"));
    }

    // ==================== File Processing Tests ====================

    #[test]
    fn test_process_file_string_mode() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.jsonl");

        let content = format!(r#"{{"small":"hello","big":"{}"}}"#, "x".repeat(1000));
        fs::write(&file_path, &content).unwrap();

        let args = Args {
            path: file_path.clone(),
            pattern: "*.jsonl".to_string(),
            line_max: None,
            string_max: Some(100),
            verbose: false,
            dry_run: false,
        };

        let stats = process_file(&file_path, &args).unwrap();

        assert_eq!(stats.strings_truncated, 1);
        assert!(stats.final_bytes < stats.original_bytes);

        // Verify file was modified
        let modified = fs::read_to_string(&file_path).unwrap();
        assert!(modified.contains("[truncated: 1000 bytes]"));

        // Verify JSON is still valid
        let parsed: Result<Value, _> = serde_json::from_str(&modified);
        assert!(parsed.is_ok());
    }

    #[test]
    fn test_process_file_dry_run() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.jsonl");

        let content = format!(r#"{{"big":"{}"}}"#, "x".repeat(1000));
        fs::write(&file_path, &content).unwrap();
        let original_content = content.clone();

        let args = Args {
            path: file_path.clone(),
            pattern: "*.jsonl".to_string(),
            line_max: None,
            string_max: Some(100),
            verbose: false,
            dry_run: true, // DRY RUN
        };

        let stats = process_file(&file_path, &args).unwrap();

        assert_eq!(stats.strings_truncated, 1);

        // File should NOT be modified in dry run
        let after = fs::read_to_string(&file_path).unwrap();
        assert_eq!(after, original_content);
    }

    #[test]
    fn test_process_file_preserves_trailing_newline() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.jsonl");

        let content = format!(
            "{}\n{}\n",
            r#"{"a":"hello"}"#,
            format!(r#"{{"b":"{}"}}"#, "x".repeat(1000))
        );
        fs::write(&file_path, &content).unwrap();

        let args = Args {
            path: file_path.clone(),
            pattern: "*.jsonl".to_string(),
            line_max: None,
            string_max: Some(100),
            verbose: false,
            dry_run: false,
        };

        process_file(&file_path, &args).unwrap();

        let modified = fs::read_to_string(&file_path).unwrap();
        assert!(modified.ends_with('\n'));
    }

    #[test]
    fn test_process_multiline_jsonl() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.jsonl");

        let lines = vec![
            r#"{"line":1,"data":"short"}"#.to_string(),
            format!(r#"{{"line":2,"data":"{}"}}"#, "x".repeat(500)),
            r#"{"line":3,"data":"also short"}"#.to_string(),
            format!(r#"{{"line":4,"data":"{}"}}"#, "y".repeat(800)),
        ];
        fs::write(&file_path, lines.join("\n")).unwrap();

        let args = Args {
            path: file_path.clone(),
            pattern: "*.jsonl".to_string(),
            line_max: None,
            string_max: Some(100),
            verbose: false,
            dry_run: false,
        };

        let stats = process_file(&file_path, &args).unwrap();

        assert_eq!(stats.strings_truncated, 2); // Lines 2 and 4

        // Verify each line is valid JSON
        let modified = fs::read_to_string(&file_path).unwrap();
        for line in modified.lines() {
            let parsed: Result<Value, _> = serde_json::from_str(line);
            assert!(parsed.is_ok(), "Line is not valid JSON: {}", line);
        }
    }

    // ==================== File Discovery Tests ====================

    #[test]
    fn test_collect_files_single_file() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.jsonl");
        fs::write(&file_path, "{}").unwrap();

        let files = collect_files(&file_path, "*.jsonl").unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0], file_path);
    }

    #[test]
    fn test_collect_files_directory_with_pattern() {
        let dir = TempDir::new().unwrap();

        // Create matching files
        fs::write(dir.path().join("a.jsonl"), "{}").unwrap();
        fs::write(dir.path().join("b.jsonl"), "{}").unwrap();

        // Create non-matching files
        fs::write(dir.path().join("c.txt"), "{}").unwrap();
        fs::write(dir.path().join("d.json"), "{}").unwrap();

        let files = collect_files(dir.path(), "*.jsonl").unwrap();

        assert_eq!(files.len(), 2);
        assert!(files.iter().all(|f| f.extension().unwrap() == "jsonl"));
    }

    #[test]
    fn test_collect_files_nested_directories() {
        let dir = TempDir::new().unwrap();

        // Create nested structure
        let sub = dir.path().join("subdir");
        fs::create_dir(&sub).unwrap();

        fs::write(dir.path().join("root.jsonl"), "{}").unwrap();
        fs::write(sub.join("nested.jsonl"), "{}").unwrap();

        let files = collect_files(dir.path(), "*.jsonl").unwrap();

        assert_eq!(files.len(), 2);
    }

    #[test]
    fn test_collect_files_nonexistent_path() {
        let result = collect_files(Path::new("/nonexistent/path"), "*.jsonl");
        assert!(result.is_err());
    }

    // ==================== Combined Mode Tests ====================

    #[test]
    fn test_string_then_line_truncation() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.jsonl");

        // Create a line with a huge string that even after string truncation
        // might still be over line limit (if we had many fields)
        let content = format!(
            r#"{{"a":"{}","b":"{}","c":"{}"}}"#,
            "x".repeat(1000),
            "y".repeat(1000),
            "z".repeat(1000)
        );
        fs::write(&file_path, &content).unwrap();

        let args = Args {
            path: file_path.clone(),
            pattern: "*.jsonl".to_string(),
            line_max: Some(200),
            string_max: Some(100),
            verbose: false,
            dry_run: false,
        };

        let stats = process_file(&file_path, &args).unwrap();

        // All 3 strings should be truncated
        assert_eq!(stats.strings_truncated, 3);
        // After string truncation, line should be under 200, so no line truncation
        assert_eq!(stats.lines_truncated, 0);
    }
}
