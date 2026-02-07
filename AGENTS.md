# AGENTS.md

Guidelines for AI agents contributing to this project.

## Project Overview

`jsonl-trim` is a Rust CLI tool that trims oversized content from JSONL files. Primary use case: preventing AI session logs from containing massive lines that pollute context windows.

## Architecture

```
src/
└── main.rs    # Everything lives here (it's a simple tool)
```

Single-file architecture is intentional — this is a focused utility, not a framework.

## Key Design Decisions

1. **Two modes, composable**: Line mode and string mode can be used independently or together
2. **JSON validity in string mode**: When truncating strings, we preserve valid JSON structure
3. **Line mode breaks JSON**: This is acceptable — it's the "nuclear option" for truly massive lines
4. **No backup by default**: We trust the user. They can `--dry-run` first.

## Dependencies

- `clap`: CLI parsing (derive mode)
- `serde_json`: JSON handling with `Value` for dynamic traversal
- `walkdir`: Recursive directory traversal
- `glob`: Pattern matching for file selection
- `anyhow`: Error handling

## Making Changes

1. Keep it simple — resist adding features that don't serve the core use case
2. Test with real session files from `~/.openclaw/sessions/`
3. Performance matters less than correctness
4. Verbose output should be human-readable

## Building

```bash
cargo build --release
```

Binary goes to `./target/release/jsonl-trim`.

## Testing Locally

```bash
# Dry run on real data
./target/release/jsonl-trim ~/.openclaw/sessions --pattern "*.out.json" --string-max 10000 --dry-run --verbose
```
