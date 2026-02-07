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
        let glob_pattern = glob::Pattern::new(pattern)
            .context("Invalid glob pattern")?;

        for entry in WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
            let entry_path = entry.path();
            if entry_path.is_file() {
                if let Some(name) = entry_path.file_name().and_then(|n| n.to_str()) {
                    if glob_pattern.matches(name) {
                        files.push(entry_path.to_path_buf());
                    }
                }
            }
        }
    } else {
        anyhow::bail!("Path does not exist: {}", path.display());
    }

    Ok(files)
}

fn process_file(path: &Path, args: &Args) -> Result<FileStats> {
    let content = fs::read_to_string(path)
        .context("Failed to read file")?;
    
    let mut stats = FileStats {
        original_bytes: content.len(),
        ..Default::default()
    };

    let mut output_lines = Vec::new();

    for line in content.lines() {
        let mut processed_line = line.to_string();

        // String mode first (if enabled)
        if let Some(max_string) = args.string_max {
            if let Ok(mut json) = serde_json::from_str::<Value>(&processed_line) {
                let count = truncate_strings(&mut json, max_string);
                if count > 0 {
                    stats.strings_truncated += count;
                    processed_line = serde_json::to_string(&json)
                        .unwrap_or_else(|_| processed_line);
                }
            }
        }

        // Line mode second (if enabled)
        if let Some(max_line) = args.line_max {
            if processed_line.len() > max_line {
                let original_len = processed_line.len();
                processed_line.truncate(max_line.saturating_sub(50));
                processed_line.push_str(&format!("... [line truncated: {} bytes]", original_len));
                stats.lines_truncated += 1;
            }
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
        fs::write(path, &output)
            .context("Failed to write file")?;
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
        println!("{}No files needed trimming ({} files checked)", prefix, total.files_processed);
    } else {
        println!(
            "{}Total: {} files modified, {:.1}KB saved",
            prefix,
            total.files_modified,
            saved_kb
        );
    }
}
