---
name: jsonl-trim
description: Trim oversized content from JSONL files to prevent context pollution. Use when session logs or JSONL files contain massive strings (base64 images, large tool outputs) that would bloat context when read. Supports string-level truncation (preserves JSON validity) and line-level truncation.
---

# jsonl-trim

Trim oversized lines and string fields in JSONL files.

## When to Use

- Before reading old session logs that may contain large embedded content
- When context is getting polluted by massive strings from prior tool calls
- To clean up JSONL archives for long-term storage

## Installation

Binary is installed at `~/.local/bin/jsonl-trim` (already in PATH on Atlas).

```bash
jsonl-trim --version
```

To rebuild from source:
```bash
cd ~/jsonl-trim && cargo build --release
```

## Usage

### String Mode (Recommended)

Truncates individual string fields while preserving JSON structure:

```bash
jsonl-trim ./sessions --pattern "*.jsonl" --string-max 10000 --verbose
```

Output shows `[truncated: 45000 bytes]` placeholders — JSON remains valid and parseable.

### Line Mode

Truncates entire lines over threshold (faster, but breaks JSON validity):

```bash
jsonl-trim ./sessions --pattern "*.jsonl" --line-max 50000
```

### Combined

Apply string truncation first, then line truncation:

```bash
jsonl-trim ./sessions --pattern "*.jsonl" --string-max 10000 --line-max 50000
```

### Dry Run

Preview changes without writing:

```bash
jsonl-trim ./sessions --pattern "*.jsonl" --string-max 10000 --dry-run --verbose
```

## Interpreting Results

```
session-abc.jsonl: 992KB → 94KB (trimmed 0 lines, 5 strings)
session-xyz.jsonl: unchanged
Total: 1 file modified, 898KB saved
```

- **Strings trimmed**: Large string fields replaced with `[truncated: N bytes]`
- **Lines trimmed**: Entire lines cut (JSON invalid for those lines)
- **Unchanged**: File had no content over threshold

## Recommended Thresholds

| Use Case | --string-max | --line-max |
|----------|--------------|------------|
| Session cleanup | 10000 | — |
| Aggressive trim | 5000 | 50000 |
| Preserve more context | 20000 | — |

## Common Patterns

Trim all session files before reading:

```bash
jsonl-trim ~/.openclaw/agents/main/sessions --pattern "*.jsonl" --string-max 10000
```

Check a specific file:

```bash
jsonl-trim session.jsonl --string-max 10000 --dry-run --verbose
```
