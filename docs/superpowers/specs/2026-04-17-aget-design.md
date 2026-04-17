# aget — Design Specification

**Date:** 2026-04-17  
**Status:** Approved

---

## Overview

`aget` is a command-line tool in the spirit of `wget`/`curl` that fetches a URL and outputs its content as Markdown. It tries a chain of "engines" in order to get the best Markdown representation of a page — from native Markdown responses, to appending `.md` to the URL, to full HTML extraction via `dom_smoothie`. Per-domain config rules allow URL transforms, engine overrides, and custom headers.

**Stack:** Rust, Cargo workspace, Tokio async runtime.

---

## Repository Layout

```
aget/
├── Cargo.toml              # workspace, shared deps
├── Makefile                # build / test / fmt shortcuts
├── aget.toml.example       # example user config
├── lib/                    # aget-lib crate (core logic, independently testable)
│   └── src/
│       ├── lib.rs
│       ├── error.rs        # AgetError (thiserror)
│       ├── config.rs       # Config, DomainRule, TOML loading
│       ├── fetcher.rs      # reqwest HTTP wrapper
│       ├── engine/
│       │   ├── mod.rs          # Engine trait + EngineResult enum
│       │   ├── registry.rs     # ordered engine chain builder
│       │   ├── accept_md.rs    # engine 1: Accept: text/markdown
│       │   ├── dot_md.rs       # engine 2: append .md to URL path
│       │   └── html_extract.rs # engine 3: dom_smoothie fallback
│       ├── quality.rs      # markdown quality heuristic
│       └── pipeline.rs     # orchestrates engines → final output
└── cli/                    # aget binary crate
    └── src/
        ├── main.rs
        └── cli.rs          # clap arg definitions
```

---

## Data Flow

```
cli parses args
  → load Config (~/.aget/config.toml)
  → apply domain rules (URL transform, engine override, headers)
  → build engine chain
  → pipeline: try engines in order
      → first Success that passes quality check → done
      → if none pass → best-effort (last Success, always html_extract)
  → write markdown to stdout or -o <file>
```

---

## Engine Trait

```rust
#[async_trait]
pub trait Engine: Send + Sync {
    fn name(&self) -> &'static str;
    async fn fetch(&self, url: &Url, fetcher: &Fetcher) -> Result<EngineResult>;
}

pub enum EngineResult {
    Success(String),  // markdown content
    Skip(String),     // reason this engine gave up
}
```

Engines are stateless and held as `Vec<Box<dyn Engine>>`. The pipeline iterates them in order, stopping at the first `Success` that passes the quality check.

### Built-in Engines (default order)

| # | Name | Strategy |
|---|------|----------|
| 1 | `accept_md` | `GET` with `Accept: text/markdown`; skip if response content-type is not `text/markdown` or `text/plain` |
| 2 | `dot_md` | Append `.md` to the URL path; skip if response is 4xx/5xx or content-type is HTML |
| 3 | `html_extract` | Fetch raw HTML; run `dom_smoothie` to extract core content; always returns `Success` |

---

## Quality Heuristic

HTTP status and content-type filtering are enforced inside each engine's `Skip` logic — by the time a `Success` reaches the pipeline, those are already validated. The quality module only checks body content:

A `EngineResult::Success` passes quality if:
- Body length > 100 chars **and** contains at least one markdown structural element: `#`, `**`, ` ``` `, `---`, `- `, or `[`

`html_extract` always returns `Success` and the body check is still applied — but if it fails quality too, the pipeline still uses its output as best-effort (it's the final fallback, always produces something).

---

## Config File

Location: `~/.aget/config.toml`

```toml
[domains."github.com"]
url_transform = "https://raw.githubusercontent.com/{owner}/{repo}/refs/heads/main/readme.md"
engine = "direct"   # skip engine chain, fetch transformed URL as-is

[domains."docs.example.com"]
engines = ["accept_md", "dot_md", "html_extract"]  # override engine order

[domains."docs.example.com".headers]
Authorization = "Bearer my-token"

[domains."api.example.com".headers]
X-API-Key = "secret"
```

### Config Structs

```rust
pub struct Config {
    pub domains: HashMap<String, DomainRule>,
}

pub struct DomainRule {
    pub url_transform: Option<String>,       // {variable} substitution from URL path segments
    pub engine: Option<String>,              // "direct" or a single engine name
    pub engines: Option<Vec<String>>,        // override full engine chain
    pub headers: HashMap<String, String>,    // merged into every request for this domain
}
```

### Domain Rule Resolution (per request)

1. Extract domain from URL
2. Exact-match lookup in `config.domains` (no wildcards in v1)
3. If `url_transform` present: rewrite URL using `{variable}` substitution from path segments
4. If `engine = "direct"`: fetch transformed URL directly, skip engine chain
5. If `engines = [...]`: use that list instead of default chain
6. Merge domain `headers` into every request for this domain

---

## CLI Interface

```
aget [OPTIONS] <URL>

Arguments:
  <URL>  URL to fetch and convert to markdown

Options:
  -o, --output <FILE>    Write output to file instead of stdout
  -C, --config <PATH>    Config file path [default: ~/.aget/config.toml]
  -v, --verbose          Print engine attempts and quality results to stderr
      --engine <NAME>    Force a specific engine (overrides domain rules)
  -h, --help             Print help
  -V, --version          Print version
```

### Verbose Output (stderr only)

```
[aget] trying engine: accept_md
[aget] accept_md: skipped (content-type: text/html)
[aget] trying engine: dot_md
[aget] dot_md: skipped (404)
[aget] trying engine: html_extract
[aget] html_extract: success (quality check passed, 3421 chars)
```

---

## Error Handling

- **`aget-lib`**: typed `AgetError` via `thiserror`; covers config parse, HTTP, IO, URL parse errors
- **`cli`**: `anyhow::Result` with `.context()` for human-readable messages
- **Exit codes**: `0` = success, `1` = general error, `2` = bad arguments
- **All engines exhausted**: always produce output (best-effort `html_extract`), exit `0`

---

## Key Dependencies

| Crate | Purpose |
|-------|---------|
| `tokio` | async runtime |
| `reqwest` | HTTP client (rustls TLS) |
| `clap` (derive) | CLI argument parsing |
| `dom_smoothie` | HTML → readable content extraction |
| `serde` + `toml` | config deserialization |
| `thiserror` | library error types |
| `anyhow` | CLI error context |
| `async-trait` | async trait methods |
| `url` | URL parsing and manipulation |

---

## Testing Strategy

- Unit tests in `lib/src/**` alongside source files
- Each engine tested independently with mock HTTP responses
- Quality heuristic tested with known-good and known-bad markdown samples
- Integration tests in `cli/` using `assert_cmd` + a local HTTP test server
- `make check`: fmt + clippy (`-D warnings`) + tests + build

---

## Adding a New Engine

1. Create `lib/src/engine/<name>.rs`, implement `Engine` trait
2. Add to `registry.rs` default chain (or document the config name)
3. Update `aget.toml.example` with the new engine name

---

## Configuration & Environment

| Variable | Purpose |
|----------|---------|
| `AGET_CONFIG` | Override config file path |
| `NO_COLOR` | Disable colored stderr output |
| `RUST_LOG` | Control log verbosity |
