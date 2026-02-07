# AGENTS.md

Guidelines for AI agents contributing to this project.

## Project Overview

`jsonl-trim` is a Rust CLI tool that trims oversized content from JSONL files. Primary use case: preventing AI session logs from containing massive lines that pollute context windows.

## Architecture

```
src/
└── main.rs    # Core implementation + tests
```

Single-file architecture is intentional — this is a focused utility, not a framework.

## Code Style

Follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/) and standard Rust idioms.

### Formatting & Linting

```bash
# Format code (required before commit)
cargo fmt

# Lint (must pass with no warnings)
cargo clippy -- -D warnings
```

### Style Rules

- Use `rustfmt` defaults (no custom rustfmt.toml)
- All public items need doc comments (`///`)
- Prefer `?` over `.unwrap()` in fallible code
- Use `thiserror` or `anyhow` for errors, not strings
- Keep functions small and focused

## Commit Convention

Use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

### Types

- `feat`: New feature
- `fix`: Bug fix
- `docs`: Documentation only
- `style`: Formatting, no code change
- `refactor`: Code change that neither fixes nor adds
- `test`: Adding or updating tests
- `chore`: Build, CI, tooling

### Examples

```
feat(cli): add --backup flag for preserving originals
fix(truncate): handle empty string fields correctly
test(string-mode): add edge cases for nested arrays
docs(readme): add installation from crates.io
```

## Testing

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture
```

Tests live in `src/main.rs` under `#[cfg(test)]` module. Cover:
- String truncation (nested objects, arrays, edge cases)
- Line truncation
- File discovery (glob patterns)
- JSON validity after truncation

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

1. Run `cargo fmt` before committing
2. Run `cargo clippy -- -D warnings` — fix all warnings
3. Run `cargo test` — all tests must pass
4. Use conventional commit messages
5. Keep it simple — resist features that don't serve the core use case

## Building

```bash
cargo build --release
```

Binary goes to `./target/release/jsonl-trim`.
