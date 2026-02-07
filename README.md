# jsonl-trim

Trim oversized lines and string fields in JSONL files. Built for keeping AI agent session logs from exploding your context window.

## The Problem

Session logs (JSONL format) can contain massive single lines — base64 images, huge tool outputs, embedded documents. When an AI agent reads these files, even with `head` or `tail`, one giant line permanently pollutes the context.

## The Solution

`jsonl-trim` offers two modes:

1. **Line mode**: Truncate entire lines over N bytes
2. **String mode**: Recursively find and truncate string fields over N bytes (preserves JSON validity)

## Installation

```bash
# From source
cargo install --path .

# Or build release binary
cargo build --release
# Binary at ./target/release/jsonl-trim
```

## Usage

```bash
# Trim string fields over 10KB (preserves JSON structure)
jsonl-trim ./sessions --pattern "*.out.json" --string-max 10000

# Trim entire lines over 50KB
jsonl-trim ./sessions --pattern "*.out.json" --line-max 50000

# Both modes (string truncation first, then line check)
jsonl-trim ./sessions --pattern "*.out.json" --string-max 10000 --line-max 50000

# Single file
jsonl-trim session.out.json --string-max 10000

# Dry run (see what would change)
jsonl-trim ./sessions --pattern "*.out.json" --string-max 10000 --dry-run

# Verbose output
jsonl-trim ./sessions --pattern "*.out.json" --string-max 10000 --verbose
```

## Output

With `--verbose`:
```
sessions/abc123.out.json: 847KB → 52KB (trimmed 12 lines, 34 strings)
sessions/def456.out.json: unchanged
Total: 2 files modified, 795KB saved
```

## How It Works

### String Mode (`--string-max`)
- Parses each line as JSON
- Recursively walks all fields
- Replaces oversized strings with `[truncated: 12345 bytes]`
- Re-serializes valid JSON

### Line Mode (`--line-max`)
- Measures raw byte length of each line
- Truncates and appends `... [line truncated: 98765 bytes]`
- Faster but breaks JSON validity for affected lines

## License

MIT
