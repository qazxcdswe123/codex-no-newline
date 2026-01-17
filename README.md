# codex-no-newline

This repository provides a small Rust tool to remove an *unwanted* “newline at end of file” that was introduced by an edit, and (optionally) rewrite commits to drop that newline.

It is intended to mitigate the issue described in:
https://github.com/openai/codex/issues/7640

In short: some workflows/files intentionally do **not** end with a trailing newline (byte `0x0A`). Certain automated edits may always append one, creating noisy diffs and churn.

## What it does

The binary `git-fix-eof-newline` looks for cases where a file’s content changed from:

- old version: **no** line terminator at EOF
- new version: **has** a line terminator at EOF (`\n` or `\r\n`)

When such a change is detected, it removes exactly one trailing line terminator from the file.

It supports three modes:

- `n = 0`: fix the current working tree / index diffs (not committed)
- `n = 1`: inspect `HEAD` and amend the `HEAD` commit if needed
- `n > 1`: inspect the most recent `n` commits, then rewrite history to fix matching commits

You can also filter commits by author name/email.

## Installation

Prerequisites:

- Rust toolchain (Cargo)
- `git` available on `PATH`

Build:

```bash
cargo build --release
```

Run:

```bash
./target/release/git-fix-eof-newline --help
```

Project layout note:

- The CLI entrypoint lives in `src/main.rs`.
- The binary name is set explicitly in `Cargo.toml` as `git-fix-eof-newline`.

## Usage

### Fix uncommitted changes (`--n 0`)

Fixes files where the working tree or index added a trailing newline compared to `HEAD`.

```bash
cargo run -- --n 0
```

Notes:

- If a file is “partially staged” (has both staged and unstaged changes), it is skipped to avoid accidentally staging extra changes.

### Fix `HEAD` (`--n 1`)

Checks `HEAD` vs its first parent. If a trailing newline was added by `HEAD`, it removes the newline in the working tree and amends `HEAD`.

```bash
cargo run -- --n 1
```

Notes:

- Requires a clean working tree (`git status --porcelain` must be empty).
- Uses `git commit --amend --no-edit --allow-empty` to handle the case where the only change in the commit was adding the EOF newline.

### Fix recent history (`--n > 1`)

Scans the most recent `n` commits on the first-parent chain. If any commit matches, it rewrites the range to remove the added EOF newline(s).

```bash
cargo run -- --n 10
```

Implementation detail:

- This uses `git filter-branch --tree-filter` on the minimal range that needs fixing.

### Author filters

Only rewrite commits whose author matches a substring filter (case-insensitive):

```bash
cargo run -- --n 10 --author-email alice@example.com
cargo run -- --n 10 --author-name Alice
```

### Dry run

Print what would be touched without modifying files or rewriting commits:

```bash
cargo run -- --n 10 --dry-run
```

## Safety / Caveats

- `n = 1` rewrites `HEAD` (new commit hash).
- `n > 1` rewrites history (many commit hashes change). Do not run on branches that others are already using unless you coordinate.
- `git filter-branch` typically leaves backup references under `refs/original/*`. Review and clean them if needed.
- Merge commits are not supported in the rewritten range (first-parent scanning is used).
- Files larger than ~10MB are skipped.

## Running tests

```bash
cargo test
```
