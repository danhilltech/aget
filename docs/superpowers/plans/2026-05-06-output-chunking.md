# Output Chunking Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `--chunk-size <N>` flag (in characters) that splits the extracted markdown into multiple files at semantic boundaries (H2 → H3 → blank line → newline → hard cut), so very large pages can be consumed by tools or agents that have per-file size limits.

**Architecture:** A new `lib/src/chunk.rs` module exposes one public function, `chunk_markdown(content: &str, max_chars: usize) -> Vec<String>`. It mirrors curl.md's `chunk.ts` algorithm: try splitting at increasingly finer markdown boundaries; greedily group sections up to `max_chars`; recurse for sections still too large; hard-cut by char count if no boundary works. The CLI requires `--chunk-size` to be paired with `--output FILE`. When the output is multi-chunk, files are written as `FILE-001.md`, `FILE-002.md`, … (preserving the original extension if present, otherwise appending `.md`).

**Tech Stack:** Rust (no new dependencies — pure string handling on `&str`/`String`).

---

## File Map

| File | Change |
|---|---|
| `lib/src/chunk.rs` | **NEW** — `chunk_markdown` and supporting private helpers |
| `lib/src/lib.rs` | Add `pub mod chunk;` |
| `cli/src/cli.rs` | Add `--chunk-size <N>` flag (requires `--output`, conflicts with `--head`) |
| `cli/src/main.rs` | When `chunk_size` set: chunk content, write multi-file output |
| `cli/tests/integration.rs` | Add three integration tests |
| `README.md` | Document the flag |

---

### Task 1: Add `chunk.rs` skeleton with public API and one trivial test

**Files:**
- Create: `lib/src/chunk.rs`
- Modify: `lib/src/lib.rs`

- [ ] **Step 1: Write the failing test (skeleton)**

Create `lib/src/chunk.rs` with:

```rust
pub fn chunk_markdown(content: &str, max_chars: usize) -> Vec<String> {
    if content.len() <= max_chars {
        return vec![content.to_string()];
    }
    todo!("implement boundary-based chunking")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_content_returns_single_chunk() {
        let content = "# Hello\n\nShort enough.";
        let chunks = chunk_markdown(content, 1000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], content);
    }
}
```

- [ ] **Step 2: Register the module**

In `lib/src/lib.rs`, add the line `pub mod chunk;` alongside the other `pub mod` declarations. The full block should read:

```rust
pub mod cache;
pub mod caching_fetcher;
pub mod chunk;
pub mod config;
pub mod engine;
pub mod error;
pub mod fetch;
pub mod fetcher;
pub mod head;
pub mod pipeline;
pub mod quality;

pub use config::Config;
pub use error::{AgetError, Result};
pub use head::HeadResult;
pub use pipeline::{Pipeline, PipelineResult};
```

- [ ] **Step 3: Run test (sanity)**

Run: `cargo test -p aget-lib chunk::tests::test_short_content_returns_single_chunk`
Expected: PASS (the early-return path is covered).

- [ ] **Step 4: Commit**

```bash
git add lib/src/chunk.rs lib/src/lib.rs
git commit -m "feat(lib): scaffold chunk module with short-content fast path"
```

---

### Task 2: TDD — H2 boundary splitting

**Files:**
- Modify: `lib/src/chunk.rs`

- [ ] **Step 1: Write the failing test**

Append to the `tests` mod in `lib/src/chunk.rs`:

```rust
#[test]
fn test_splits_at_h2_boundary() {
    let content = "# Title\n\nIntro paragraph.\n\n## Section A\n\nLots of text in section A.\n\n## Section B\n\nLots of text in section B.\n";
    // Force splitting by setting max_chars below total length but above each section length
    let chunks = chunk_markdown(content, 80);
    assert!(chunks.len() >= 2, "expected at least 2 chunks, got {}", chunks.len());
    // No chunk should exceed max_chars (allowing slack for boundary inclusion)
    for c in &chunks {
        assert!(c.len() <= 120, "chunk too long: {} chars", c.len());
    }
    // Reassembling chunks must reproduce the original content exactly
    assert_eq!(chunks.join(""), content);
    // Each chunk after the first should start with "## "
    for c in chunks.iter().skip(1) {
        assert!(c.starts_with("## "), "chunk should start with '## ', got: {:?}", &c[..c.len().min(20)]);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p aget-lib chunk::tests::test_splits_at_h2_boundary`
Expected: FAIL with `not yet implemented` (the `todo!()`).

- [ ] **Step 3: Implement the boundary splitter**

Replace the body of `chunk_markdown` and add helpers. The full file becomes:

```rust
pub fn chunk_markdown(content: &str, max_chars: usize) -> Vec<String> {
    if content.len() <= max_chars {
        return vec![content.to_string()];
    }
    split_at_boundary(content, max_chars, 0)
}

const BOUNDARIES: &[&str] = &["\n## ", "\n### ", "\n\n", "\n"];

fn split_at_boundary(text: &str, max_chars: usize, level: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }

    let separator = match BOUNDARIES.get(level) {
        Some(s) => *s,
        None => return hard_split(text, max_chars),
    };

    let sections = split_keeping_separator(text, separator);
    if sections.len() <= 1 {
        return split_at_boundary(text, max_chars, level + 1);
    }

    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    for section in sections {
        if !current.is_empty() && current.len() + section.len() > max_chars {
            chunks.extend(split_at_boundary(&current, max_chars, level + 1));
            current = section;
        } else {
            current.push_str(&section);
        }
    }
    if !current.is_empty() {
        chunks.extend(split_at_boundary(&current, max_chars, level + 1));
    }
    chunks
}

fn split_keeping_separator(text: &str, separator: &str) -> Vec<String> {
    let mut sections: Vec<String> = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        // Skip the first char so a separator at index 0 doesn't produce an empty leading section
        let search_start = remaining.char_indices().nth(1).map(|(i, _)| i).unwrap_or(remaining.len());
        match remaining[search_start..].find(separator) {
            Some(rel_idx) => {
                let abs_idx = search_start + rel_idx;
                sections.push(remaining[..abs_idx].to_string());
                remaining = &remaining[abs_idx..];
            }
            None => {
                sections.push(remaining.to_string());
                break;
            }
        }
    }
    sections
}

fn hard_split(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut buf = String::new();
    for ch in text.chars() {
        if buf.len() + ch.len_utf8() > max_chars && !buf.is_empty() {
            chunks.push(std::mem::take(&mut buf));
        }
        buf.push(ch);
    }
    if !buf.is_empty() {
        chunks.push(buf);
    }
    chunks
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo test -p aget-lib chunk::tests::test_splits_at_h2_boundary`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add lib/src/chunk.rs
git commit -m "feat(lib): chunk markdown at H2 boundaries"
```

---

### Task 3: TDD — fall through to H3, blank line, then hard cut

**Files:**
- Modify: `lib/src/chunk.rs`

- [ ] **Step 1: Add three more failing tests**

Append to the `tests` mod in `lib/src/chunk.rs`:

```rust
#[test]
fn test_falls_through_to_h3_when_no_h2() {
    let content = "# Title\n\nIntro.\n\n### Sub A\n\nContent A here.\n\n### Sub B\n\nContent B here.\n";
    let chunks = chunk_markdown(content, 60);
    assert!(chunks.len() >= 2);
    assert_eq!(chunks.join(""), content);
}

#[test]
fn test_hard_cut_when_no_boundaries() {
    // One long line of repeated chars with no boundary characters at all
    let content = "a".repeat(500);
    let chunks = chunk_markdown(&content, 100);
    assert!(chunks.len() >= 5, "expected at least 5 chunks, got {}", chunks.len());
    for c in &chunks {
        assert!(c.len() <= 100, "chunk exceeded max: {}", c.len());
    }
    assert_eq!(chunks.join(""), content);
}

#[test]
fn test_preserves_unicode() {
    let content = "# 日本語\n\n本文がここにあります。たくさんの文字があります。\n\n## セクション2\n\nもっとテキスト。\n";
    let chunks = chunk_markdown(content, 30);
    assert!(chunks.len() >= 2);
    // Concatenation must equal original (no byte-boundary corruption)
    assert_eq!(chunks.join(""), content);
}

#[test]
fn test_zero_max_returns_per_char_chunks_for_no_boundary_input() {
    // Edge case: very small max with no boundaries — should not infinite-loop
    let content = "abcdef";
    let chunks = chunk_markdown(content, 1);
    assert_eq!(chunks.len(), 6);
    assert_eq!(chunks.join(""), content);
}
```

- [ ] **Step 2: Run tests to confirm**

Run: `cargo test -p aget-lib chunk::tests`
Expected: PASS for all five tests in `chunk::tests`. The implementation from Task 2 already covers these cases; if any fail, fix the underlying logic before continuing.

- [ ] **Step 3: Commit**

```bash
git add lib/src/chunk.rs
git commit -m "test: cover h3, hard-cut, unicode, and zero-max edge cases for chunking"
```

---

### Task 4: Add `--chunk-size` CLI flag

**Files:**
- Modify: `cli/src/cli.rs`

- [ ] **Step 1: Add the flag definition**

In `cli/src/cli.rs`, add a new field to the `Cli` struct (place it after the `json` field, before the closing brace):

```rust
    /// Split output into multiple files of this max char count (requires --output)
    #[arg(
        long = "chunk-size",
        value_name = "N",
        requires = "output",
        conflicts_with = "head"
    )]
    pub chunk_size: Option<usize>,
```

- [ ] **Step 2: Verify build**

Run: `cargo build -p aget`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add cli/src/cli.rs
git commit -m "feat(cli): add --chunk-size flag definition"
```

---

### Task 5: Wire the flag into `main.rs` with multi-file output

**Files:**
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Replace the file-output branch**

In `cli/src/main.rs`, locate the `match &cli.output` block at the bottom of `run()`. Replace the entire block with:

```rust
    match (&cli.output, cli.chunk_size) {
        (Some(path), Some(max_chars)) => {
            let chunks = aget_lib::chunk::chunk_markdown(&result.content, max_chars);
            if chunks.len() == 1 {
                std::fs::write(path, &chunks[0])
                    .with_context(|| format!("failed to write to {}", path.display()))?;
            } else {
                let (stem, ext) = split_path(path);
                for (i, chunk) in chunks.iter().enumerate() {
                    let part_path = stem.with_file_name(format!(
                        "{}-{:03}{}",
                        stem.file_name().and_then(|s| s.to_str()).unwrap_or("output"),
                        i + 1,
                        ext.as_deref().unwrap_or(".md"),
                    ));
                    std::fs::write(&part_path, chunk)
                        .with_context(|| format!("failed to write to {}", part_path.display()))?;
                }
                eprintln!("[aget] wrote {} chunks", chunks.len());
            }
        }
        (Some(path), None) => {
            std::fs::write(path, &result.content)
                .with_context(|| format!("failed to write to {}", path.display()))?;
        }
        (None, _) => {
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            out.write_all(result.content.as_bytes())
                .context("failed to write to stdout")?;
            if !result.content.ends_with('\n') {
                out.write_all(b"\n").ok();
            }
        }
    }
```

- [ ] **Step 2: Add the `split_path` helper**

At the bottom of `cli/src/main.rs`, add:

```rust
fn split_path(path: &std::path::Path) -> (std::path::PathBuf, Option<String>) {
    let stem = path
        .file_stem()
        .map(std::ffi::OsStr::to_os_string)
        .unwrap_or_else(|| std::ffi::OsString::from("output"));
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| format!(".{}", e));
    let parent = path.parent().unwrap_or(std::path::Path::new("."));
    let stem_path = parent.join(stem);
    (stem_path, ext)
}
```

- [ ] **Step 3: Verify build**

Run: `cargo build -p aget`
Expected: PASS.

Note: clap's `requires = "output"` already ensures `chunk_size` cannot be supplied without `output`, so we don't need a runtime check for the `(None, Some(_))` case — that combination is unreachable.

- [ ] **Step 4: Commit**

```bash
git add cli/src/main.rs
git commit -m "feat(cli): wire --chunk-size to multi-file output"
```

---

### Task 6: Integration tests

**Files:**
- Modify: `cli/tests/integration.rs`

- [ ] **Step 1: Append integration tests**

Append to `cli/tests/integration.rs`:

```rust
#[tokio::test]
async fn test_chunk_size_writes_single_file_when_content_fits() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "text/markdown")
        .with_body("# Title\n\nShort content with **bold** and a [link](http://example.com) — well under any chunk size.")
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    let out_path = dir.path().join("page.md");

    let output = aget()
        .arg(server.url())
        .arg("-o")
        .arg(&out_path)
        .arg("--chunk-size")
        .arg("10000")
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(out_path.exists(), "single-file output should be at original path");
    // Multi-file outputs should NOT have been created
    assert!(!dir.path().join("page-001.md").exists());
}

#[tokio::test]
async fn test_chunk_size_writes_multiple_files_when_content_large() {
    let mut server = Server::new_async().await;
    // Build content with multiple H2 sections so chunking has somewhere to split
    let body = format!(
        "# Title\n\n{}",
        (1..=5)
            .map(|i| format!("## Section {}\n\n{}\n\n", i, "Content text here. ".repeat(20)))
            .collect::<String>()
    );
    let _mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "text/markdown")
        .with_body(body)
        .create_async()
        .await;

    let dir = tempfile::tempdir().unwrap();
    let out_path = dir.path().join("page.md");

    let output = aget()
        .arg(server.url())
        .arg("-o")
        .arg(&out_path)
        .arg("--chunk-size")
        .arg("300")
        .output()
        .unwrap();

    assert!(output.status.success(), "stderr: {}", String::from_utf8_lossy(&output.stderr));
    assert!(dir.path().join("page-001.md").exists(), "first chunk file should exist");
    assert!(dir.path().join("page-002.md").exists(), "second chunk file should exist");
    // The original `page.md` path should NOT have been written when chunked
    assert!(!out_path.exists(), "single-file path should not be written when chunked");
}

#[test]
fn test_chunk_size_without_output_exits_nonzero() {
    let output = aget()
        .arg("https://example.com")
        .arg("--chunk-size")
        .arg("1000")
        .output()
        .unwrap();
    assert!(!output.status.success());
}
```

- [ ] **Step 2: Run the new tests**

Run: `cargo test -p aget --test integration test_chunk_size`
Expected: PASS for all three.

- [ ] **Step 3: Run full check**

Run: `make check`
Expected: PASS — fmt, clippy (no warnings), all tests, build.

- [ ] **Step 4: Commit**

```bash
git add cli/tests/integration.rs
git commit -m "test: cover --chunk-size with single-file, multi-file, and missing-output cases"
```

---

### Task 7: Document in README

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Add to options list and add an example**

In `README.md`, add to the Options block:

```
      --chunk-size <N>     Split output into N-char chunks (requires --output)
```

In the Examples block, add:

````markdown
```bash
# Save a long page as multiple files (page-001.md, page-002.md, ...)
aget -o page.md --chunk-size 8000 https://example.com/long-doc
```
````

- [ ] **Step 2: Commit**

```bash
git add README.md
git commit -m "docs: document --chunk-size flag"
```

---

## Self-Review Checklist (run before merging)

- [ ] `cargo test -p aget-lib chunk` — all unit tests pass.
- [ ] `cargo test -p aget --test integration test_chunk_size` — all integration tests pass.
- [ ] `make check` — fmt, clippy (no warnings), all tests, build.
- [ ] Manual test: fetch a real long page (e.g. MDN) with `--chunk-size 5000 -o foo.md`; confirm `foo-001.md`, `foo-002.md`, … are produced and concatenating them reproduces the same content as without chunking.
- [ ] No regressions to existing tests (`--head`, `--output`, `--engine`, etc.).
