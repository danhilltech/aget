# --head Mode Design

**Date:** 2026-04-18
**Status:** Approved

## Problem

Agents using `aget` have no lightweight way to inspect what a URL will yield before committing to full extraction. They need to know the size, token count, title, and description of the *extracted Markdown* — not the raw HTTP response — to make routing or budget decisions.

## Goals

1. Add `--head` flag that runs the full pipeline and reports metadata on the extracted output.
2. Support plain-text (default) and JSON (`--json`) output formats.
3. Token count uses `tiktoken-rs` with `cl100k_base` encoding (GPT-4 compatible).
4. Title and description are extracted from the Markdown output, not the raw HTML.
5. Reuses the existing `Pipeline` and `Fetch` infrastructure — no duplicate fetch logic.

## Architecture

Two changed units, one new unit:

```
lib/src/
├── head.rs        NEW — HeadResult struct, head() function, extraction helpers
└── lib.rs         MOD — pub mod head

cli/src/
├── cli.rs         MOD — --head and --json flags
└── main.rs        MOD — detect --head, call head(), format and print result
```

`lib/Cargo.toml`: add `tiktoken-rs`.

## head() Function

```rust
pub async fn head(
    url: &Url,
    pipeline: &Pipeline,
    rule: Option<&DomainRule>,
) -> Result<HeadResult>
```

Calls `pipeline.run(url, rule, false)` internally, then derives all fields from `PipelineResult.content` (the extracted Markdown string).

## Data Model

```rust
pub struct HeadResult {
    pub url: String,           // URL passed in (post-transform if rule applies)
    pub engine_used: String,   // e.g. "html_extract", "accept_md", "none"
    pub size_bytes: usize,     // content.len()
    pub size_kb: f64,          // size_bytes as KB, rounded to 1 decimal
    pub token_count: usize,    // tiktoken cl100k_base token count
    pub title: Option<String>, // first "# Heading" line, stripped of leading "# "
    pub description: Option<String>, // first non-heading, non-empty paragraph (≤200 chars)
}
```

### Field Extraction Rules

**title**: Scan lines of the Markdown. Take the text of the first line that starts with `# ` (one `#` only), strip the `# ` prefix. If none found, `None`.

**description**: After finding (or skipping) the title line, find the first non-empty line that does not start with `#`. Truncate to 200 characters, appending `…` if truncated.

**size_kb**: `(size_bytes as f64 / 1024.0 * 10.0).round() / 10.0`

**token_count**: `tiktoken_rs::cl100k_base()?.encode_with_special_tokens(content).len()`

## Output Formats

### Plain text (default with --head)

```
URL:         https://example.com/article
Engine:      html_extract
Size:        12.4 KB (12,701 bytes)
Tokens:      3,142
Title:       My Article Title
Description: First paragraph of the extracted content...
```

`None` fields print as `-`.

### JSON (--head --json)

```json
{
  "url": "https://example.com/article",
  "engine_used": "html_extract",
  "size_bytes": 12701,
  "size_kb": 12.4,
  "token_count": 3142,
  "title": "My Article Title",
  "description": "First paragraph of the extracted content..."
}
```

`None` fields serialize as `null`.

## CLI

```
--head      Print a content summary instead of outputting the extracted Markdown
--json      Output --head result as JSON (ignored if --head is not set; warns to stderr if used alone)
```

`--head` and `--output` are mutually exclusive: if both are set, exit with error.

Exit code is 0 in all cases where the fetch itself succeeds, even if quality check failed (`engine_used` may be `"none"` and content may be empty — token count 0, size 0).

## Dependencies

Add to `lib/Cargo.toml`:
- `tiktoken-rs = "0.6"`

Add to `lib/Cargo.toml` (serde already present via existing deps, but add if missing):
- `serde = { version = "1", features = ["derive"] }` (for JSON serialization)
- `serde_json = "1"`

## Testing

- `head.rs` unit tests:
  - `test_title_from_h1`: content with `# My Title` → title is `"My Title"`
  - `test_title_none_when_no_h1`: content with no `#` heading → title is `None`
  - `test_title_ignores_h2`: content where first heading is `## Sub` → title is `None`
  - `test_description_first_paragraph`: extracts first non-heading paragraph
  - `test_description_truncates_at_200`: long first paragraph is truncated with `…`
  - `test_description_skips_headings`: heading lines not used as description
  - `test_size_kb_rounding`: 1024 bytes → 1.0 KB, 1536 bytes → 1.5 KB
  - `test_token_count_known_string`: known string produces expected token count
- CLI integration tests:
  - `test_head_plain_text`: `aget --head <url>` prints lines starting with `URL:`, `Engine:`, `Size:`, `Tokens:`
  - `test_head_json`: `aget --head --json <url>` stdout is valid JSON with keys `url`, `engine_used`, `size_bytes`, `size_kb`, `token_count`
  - `test_head_and_output_are_mutually_exclusive`: `aget --head -o out.md <url>` exits non-zero
