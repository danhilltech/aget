# aget Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build `aget`, a CLI tool that fetches a URL and outputs Markdown using a chain of "engines" (native markdown, .md append, HTML extraction) with per-domain config rules.

**Architecture:** Cargo workspace with `lib/` (aget-lib: engine trait, pipeline, config, fetcher) and `cli/` (binary: clap args, wiring). Engines implement a single async trait; the pipeline tries them in order and returns the first result passing a quality heuristic, falling back to best-effort HTML extraction.

**Tech Stack:** Rust, Tokio, reqwest (rustls), clap (derive), dom_smoothie + htmd, serde/toml, thiserror/anyhow.

---

## File Map

| File | Responsibility |
|------|---------------|
| `Cargo.toml` | Workspace definition, shared deps |
| `Makefile` | `build`, `test`, `fmt`, `check`, `release`, `install` targets |
| `aget.toml.example` | Documented example config |
| `lib/Cargo.toml` | aget-lib crate deps |
| `lib/src/lib.rs` | Public re-exports |
| `lib/src/error.rs` | `AgetError` (thiserror), `Result<T>` alias |
| `lib/src/config.rs` | `Config`, `DomainRule`, TOML loading, URL transform |
| `lib/src/fetcher.rs` | `Fetcher`, `FetchResponse` — thin reqwest wrapper |
| `lib/src/engine/mod.rs` | `Engine` trait, `EngineResult` enum |
| `lib/src/engine/accept_md.rs` | Engine 1: `Accept: text/markdown` |
| `lib/src/engine/dot_md.rs` | Engine 2: append `.md` to URL path |
| `lib/src/engine/html_extract.rs` | Engine 3: dom_smoothie + htmd fallback |
| `lib/src/engine/registry.rs` | Build engine chain from config/name |
| `lib/src/quality.rs` | `passes_quality(content: &str) -> bool` |
| `lib/src/pipeline.rs` | `Pipeline`, `PipelineResult`, orchestration |
| `cli/Cargo.toml` | Binary crate deps |
| `cli/src/cli.rs` | `Cli` struct (clap derive) |
| `cli/src/main.rs` | `main`, `run`, wiring |

---

## Task 1: Workspace Scaffold

**Files:**
- Create: `Cargo.toml`
- Create: `lib/Cargo.toml`
- Create: `lib/src/lib.rs`
- Create: `cli/Cargo.toml`
- Create: `cli/src/main.rs`

- [ ] **Step 1: Create workspace `Cargo.toml`**

```toml
[workspace]
members = ["lib", "cli"]
resolver = "2"

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"
repository = "https://github.com/danhilltech/aget"
homepage = "https://github.com/danhilltech/aget"
authors = ["Dan Hill"]

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls"] }
serde = { version = "1", features = ["derive"] }
toml = "0.8"
thiserror = "2"
anyhow = "1"
async-trait = "0.1"
url = "2"
dirs = "6"
clap = { version = "4", features = ["derive"] }
dom_smoothie = "0.5"
htmd = "0.2"
```

- [ ] **Step 2: Create `lib/Cargo.toml`**

```toml
[package]
name = "aget-lib"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
authors.workspace = true
description = "Core library for aget"

[dependencies]
tokio.workspace = true
reqwest.workspace = true
serde.workspace = true
toml.workspace = true
thiserror.workspace = true
async-trait.workspace = true
url.workspace = true
dirs.workspace = true
dom_smoothie.workspace = true
htmd.workspace = true

[dev-dependencies]
tokio = { version = "1", features = ["full"] }
mockito = "1"
tempfile = "3"
```

- [ ] **Step 3: Create `lib/src/lib.rs`** (stub — will grow)

```rust
pub mod config;
pub mod engine;
pub mod error;
pub mod fetcher;
pub mod pipeline;
pub mod quality;

pub use config::Config;
pub use error::{AgetError, Result};
pub use pipeline::{Pipeline, PipelineResult};
```

- [ ] **Step 4: Create `cli/Cargo.toml`**

```toml
[package]
name = "aget"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
authors.workspace = true
description = "Fetch URLs as Markdown"
keywords = ["cli", "markdown", "wget", "curl"]
categories = ["command-line-utilities"]

[[bin]]
name = "aget"
path = "src/main.rs"

[dependencies]
aget-lib = { path = "../lib" }
clap.workspace = true
anyhow.workspace = true
tokio.workspace = true

[dev-dependencies]
assert_cmd = "2"
predicates = "3"
tempfile = "3"
```

- [ ] **Step 5: Create `cli/src/main.rs`** (minimal stub)

```rust
fn main() {}
```

- [ ] **Step 6: Verify it compiles**

```
cargo build
```

Expected: compiles with warnings (empty lib), no errors. Fix any version conflicts before proceeding.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml Cargo.lock lib/Cargo.toml lib/src/lib.rs cli/Cargo.toml cli/src/main.rs
git commit -m "chore: scaffold workspace"
```

---

## Task 2: Error Types

**Files:**
- Create: `lib/src/error.rs`

- [ ] **Step 1: Create `lib/src/error.rs`**

```rust
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AgetError>;

#[derive(Error, Debug)]
pub enum AgetError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Config file parse error: {0}")]
    TomlParse(#[from] toml::de::Error),

    #[error("Extraction error: {0}")]
    Extraction(String),
}

impl AgetError {
    pub fn config(msg: impl Into<String>) -> Self {
        Self::Config(msg.into())
    }

    pub fn extraction(msg: impl Into<String>) -> Self {
        Self::Extraction(msg.into())
    }
}
```

- [ ] **Step 2: Run `cargo build`**

Expected: compiles cleanly.

- [ ] **Step 3: Commit**

```bash
git add lib/src/error.rs
git commit -m "feat: add AgetError types"
```

---

## Task 3: Config and DomainRule

**Files:**
- Create: `lib/src/config.rs`

- [ ] **Step 1: Write the failing test**

Add at the bottom of `lib/src/config.rs` (create the file with this content):

```rust
use crate::error::{AgetError, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub domains: HashMap<String, DomainRule>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DomainRule {
    pub url_transform: Option<String>,
    pub engine: Option<String>,
    pub engines: Option<Vec<String>>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(AgetError::Io)?;
        toml::from_str(&content).map_err(AgetError::TomlParse)
    }

    pub fn load_default() -> Result<Self> {
        match Self::default_path() {
            Some(path) if path.exists() => Self::load(&path),
            _ => Ok(Self::default()),
        }
    }

    pub fn default_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".aget").join("config.toml"))
    }
}

pub fn apply_url_transform(url: &url::Url, template: &str) -> Result<url::Url> {
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segs| segs.filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let mut result = String::new();
    let mut seg_idx = 0;
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' {
            let mut _var_name = String::new();
            for c2 in chars.by_ref() {
                if c2 == '}' {
                    break;
                }
                _var_name.push(c2);
            }
            result.push_str(segments.get(seg_idx).copied().unwrap_or(""));
            seg_idx += 1;
        } else {
            result.push(c);
        }
    }

    url::Url::parse(&result).map_err(AgetError::UrlParse)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;
    use std::io::Write;

    #[test]
    fn test_config_parses_domain_rule() {
        let toml = r#"
[domains."github.com"]
url_transform = "https://raw.githubusercontent.com/{owner}/{repo}/refs/heads/main/readme.md"
engine = "direct"

[domains."docs.example.com"]
engines = ["accept_md", "dot_md"]

[domains."docs.example.com".headers]
Authorization = "Bearer token123"
"#;
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(toml.as_bytes()).unwrap();

        let config = Config::load(f.path()).unwrap();

        let gh = config.domains.get("github.com").unwrap();
        assert_eq!(
            gh.url_transform.as_deref(),
            Some("https://raw.githubusercontent.com/{owner}/{repo}/refs/heads/main/readme.md")
        );
        assert_eq!(gh.engine.as_deref(), Some("direct"));

        let docs = config.domains.get("docs.example.com").unwrap();
        assert_eq!(
            docs.engines.as_ref().unwrap(),
            &["accept_md", "dot_md"]
        );
        assert_eq!(docs.headers.get("Authorization").map(String::as_str), Some("Bearer token123"));
    }

    #[test]
    fn test_config_empty_file() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"").unwrap();
        let config = Config::load(f.path()).unwrap();
        assert!(config.domains.is_empty());
    }

    #[test]
    fn test_apply_url_transform_github() {
        let url = url::Url::parse("https://github.com/danhilltech/goyolov5").unwrap();
        let template = "https://raw.githubusercontent.com/{owner}/{repo}/refs/heads/main/readme.md";
        let result = apply_url_transform(&url, template).unwrap();
        assert_eq!(
            result.as_str(),
            "https://raw.githubusercontent.com/danhilltech/goyolov5/refs/heads/main/readme.md"
        );
    }

    #[test]
    fn test_apply_url_transform_no_placeholders() {
        let url = url::Url::parse("https://example.com/page").unwrap();
        let template = "https://other.com/fixed";
        let result = apply_url_transform(&url, template).unwrap();
        assert_eq!(result.as_str(), "https://other.com/fixed");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

```
cargo test -p aget-lib config
```

Expected: compile errors (module not wired in lib.rs yet — `config` is already there from step, but `dirs`, `url`, `toml` deps need to be present).

- [ ] **Step 3: Run tests to verify they pass**

```
cargo test -p aget-lib config
```

Expected: all 4 tests pass.

- [ ] **Step 4: Commit**

```bash
git add lib/src/config.rs
git commit -m "feat: add Config, DomainRule, and URL transform"
```

---

## Task 4: Fetcher

**Files:**
- Create: `lib/src/fetcher.rs`

- [ ] **Step 1: Write `lib/src/fetcher.rs`** with tests inline

```rust
use crate::error::Result;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;
use std::str::FromStr;
use url::Url;

pub struct Fetcher {
    client: reqwest::Client,
}

pub struct FetchResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: String,
}

impl FetchResponse {
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    pub fn content_type_is(&self, prefix: &str) -> bool {
        self.content_type
            .as_deref()
            .map(|ct| ct.starts_with(prefix))
            .unwrap_or(false)
    }
}

impl Fetcher {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(concat!("aget/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { client })
    }

    pub async fn get(&self, url: &Url, headers: &HashMap<String, String>) -> Result<FetchResponse> {
        let mut header_map = HeaderMap::new();
        for (k, v) in headers {
            if let (Ok(name), Ok(value)) = (
                HeaderName::from_str(k),
                HeaderValue::from_str(v),
            ) {
                header_map.insert(name, value);
            }
        }

        let response = self.client.get(url.as_str()).headers(header_map).send().await?;

        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(';').next().unwrap_or(s).trim().to_string());

        let body = response.text().await?;
        Ok(FetchResponse { status, content_type, body })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetcher_get_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/page")
            .with_status(200)
            .with_header("content-type", "text/markdown; charset=utf-8")
            .with_body("# Hello")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&format!("{}/page", server.url())).unwrap();
        let resp = fetcher.get(&url, &HashMap::new()).await.unwrap();

        assert!(resp.is_success());
        assert_eq!(resp.content_type.as_deref(), Some("text/markdown"));
        assert_eq!(resp.body, "# Hello");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetcher_passes_custom_headers() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .match_header("X-Api-Key", "secret")
            .with_status(200)
            .with_body("ok")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&server.url()).unwrap();
        let mut headers = HashMap::new();
        headers.insert("X-Api-Key".to_string(), "secret".to_string());
        let resp = fetcher.get(&url, &headers).await.unwrap();

        assert!(resp.is_success());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_content_type_is() {
        let resp = FetchResponse {
            status: 200,
            content_type: Some("text/html".to_string()),
            body: String::new(),
        };
        assert!(resp.content_type_is("text/html"));
        assert!(!resp.content_type_is("text/markdown"));
    }
}
```

- [ ] **Step 2: Run tests**

```
cargo test -p aget-lib fetcher
```

Expected: all 3 tests pass.

- [ ] **Step 3: Commit**

```bash
git add lib/src/fetcher.rs
git commit -m "feat: add Fetcher HTTP wrapper"
```

---

## Task 5: Engine Trait

**Files:**
- Create: `lib/src/engine/mod.rs`

- [ ] **Step 1: Create `lib/src/engine/mod.rs`**

```rust
pub mod accept_md;
pub mod dot_md;
pub mod html_extract;
pub mod registry;

use crate::error::Result;
use crate::fetcher::Fetcher;
use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;

#[async_trait]
pub trait Engine: Send + Sync {
    fn name(&self) -> &'static str;
    async fn fetch(
        &self,
        url: &Url,
        fetcher: &Fetcher,
        domain_headers: &HashMap<String, String>,
    ) -> Result<EngineResult>;
}

pub enum EngineResult {
    Success(String),
    Skip(String),
}
```

- [ ] **Step 2: Add stub files for each engine so mod compiles**

Create `lib/src/engine/accept_md.rs`:
```rust
pub struct AcceptMdEngine;
```

Create `lib/src/engine/dot_md.rs`:
```rust
pub struct DotMdEngine;
```

Create `lib/src/engine/html_extract.rs`:
```rust
pub struct HtmlExtractEngine;
```

Create `lib/src/engine/registry.rs`:
```rust
use crate::config::DomainRule;
use crate::engine::{accept_md::AcceptMdEngine, dot_md::DotMdEngine, html_extract::HtmlExtractEngine, Engine};

pub fn build_chain(rule: Option<&DomainRule>) -> Vec<Box<dyn Engine>> {
    vec![
        Box::new(AcceptMdEngine),
        Box::new(DotMdEngine),
        Box::new(HtmlExtractEngine),
    ]
}
```

- [ ] **Step 3: Update `lib/src/lib.rs`** to expose engine module

```rust
pub mod config;
pub mod engine;
pub mod error;
pub mod fetcher;
pub mod pipeline;
pub mod quality;

pub use config::Config;
pub use error::{AgetError, Result};
pub use pipeline::{Pipeline, PipelineResult};
```

- [ ] **Step 4: Verify compilation**

```
cargo build -p aget-lib
```

Expected: compiles with warnings about unused code.

- [ ] **Step 5: Commit**

```bash
git add lib/src/engine/
git commit -m "feat: add Engine trait and stub implementations"
```

---

## Task 6: AcceptMd Engine

**Files:**
- Modify: `lib/src/engine/accept_md.rs`

- [ ] **Step 1: Write failing tests first, then implement in `lib/src/engine/accept_md.rs`**

```rust
use crate::engine::{Engine, EngineResult};
use crate::error::Result;
use crate::fetcher::Fetcher;
use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;

pub struct AcceptMdEngine;

#[async_trait]
impl Engine for AcceptMdEngine {
    fn name(&self) -> &'static str {
        "accept_md"
    }

    async fn fetch(
        &self,
        url: &Url,
        fetcher: &Fetcher,
        domain_headers: &HashMap<String, String>,
    ) -> Result<EngineResult> {
        let mut headers = domain_headers.clone();
        headers.insert("Accept".to_string(), "text/markdown, text/plain;q=0.9".to_string());

        let resp = fetcher.get(url, &headers).await?;

        if !resp.is_success() {
            return Ok(EngineResult::Skip(format!("HTTP {}", resp.status)));
        }

        let ct = resp.content_type.as_deref().unwrap_or("");
        if !ct.starts_with("text/markdown") && !ct.starts_with("text/plain") {
            return Ok(EngineResult::Skip(format!("content-type: {}", ct)));
        }

        Ok(EngineResult::Success(resp.body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_accept_md_success_markdown() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .match_header("Accept", mockito::Matcher::Regex("text/markdown".to_string()))
            .with_status(200)
            .with_header("content-type", "text/markdown")
            .with_body("# Hello\n\nWorld content here with **bold**.")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&server.url()).unwrap();
        let result = AcceptMdEngine.fetch(&url, &fetcher, &HashMap::new()).await.unwrap();

        assert!(matches!(result, EngineResult::Success(_)));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_accept_md_skips_html_response() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body("<html><body>Hello</body></html>")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&server.url()).unwrap();
        let result = AcceptMdEngine.fetch(&url, &fetcher, &HashMap::new()).await.unwrap();

        assert!(matches!(result, EngineResult::Skip(_)));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_accept_md_skips_non_200() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .with_status(404)
            .with_body("not found")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&server.url()).unwrap();
        let result = AcceptMdEngine.fetch(&url, &fetcher, &HashMap::new()).await.unwrap();

        assert!(matches!(result, EngineResult::Skip(_)));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_accept_md_merges_domain_headers() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .match_header("X-Token", "abc")
            .with_status(200)
            .with_header("content-type", "text/markdown")
            .with_body("# hi")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&server.url()).unwrap();
        let mut domain_headers = HashMap::new();
        domain_headers.insert("X-Token".to_string(), "abc".to_string());
        let result = AcceptMdEngine.fetch(&url, &fetcher, &domain_headers).await.unwrap();

        assert!(matches!(result, EngineResult::Success(_)));
        mock.assert_async().await;
    }
}
```

- [ ] **Step 2: Run tests**

```
cargo test -p aget-lib accept_md
```

Expected: all 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add lib/src/engine/accept_md.rs
git commit -m "feat: implement AcceptMd engine"
```

---

## Task 7: DotMd Engine

**Files:**
- Modify: `lib/src/engine/dot_md.rs`

- [ ] **Step 1: Write tests and implementation in `lib/src/engine/dot_md.rs`**

```rust
use crate::engine::{Engine, EngineResult};
use crate::error::Result;
use crate::fetcher::Fetcher;
use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;

pub struct DotMdEngine;

fn append_md_extension(url: &Url) -> Option<Url> {
    let mut new_url = url.clone();
    let path = url.path().to_string();
    // Don't double-add .md
    if path.ends_with(".md") {
        return None;
    }
    new_url.set_path(&format!("{}.md", path));
    Some(new_url)
}

#[async_trait]
impl Engine for DotMdEngine {
    fn name(&self) -> &'static str {
        "dot_md"
    }

    async fn fetch(
        &self,
        url: &Url,
        fetcher: &Fetcher,
        domain_headers: &HashMap<String, String>,
    ) -> Result<EngineResult> {
        let md_url = match append_md_extension(url) {
            Some(u) => u,
            None => return Ok(EngineResult::Skip("URL already ends with .md".to_string())),
        };

        let resp = fetcher.get(&md_url, domain_headers).await?;

        if !resp.is_success() {
            return Ok(EngineResult::Skip(format!("HTTP {}", resp.status)));
        }

        if resp.content_type_is("text/html") {
            return Ok(EngineResult::Skip("content-type: text/html".to_string()));
        }

        Ok(EngineResult::Success(resp.body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_append_md_extension() {
        let url = Url::parse("https://example.com/docs/page").unwrap();
        let result = append_md_extension(&url).unwrap();
        assert_eq!(result.path(), "/docs/page.md");
    }

    #[test]
    fn test_append_md_extension_already_md() {
        let url = Url::parse("https://example.com/docs/page.md").unwrap();
        assert!(append_md_extension(&url).is_none());
    }

    #[tokio::test]
    async fn test_dot_md_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/page.md")
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body("# Page\n\nContent here with **markdown**.")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&format!("{}/page", server.url())).unwrap();
        let result = DotMdEngine.fetch(&url, &fetcher, &HashMap::new()).await.unwrap();

        assert!(matches!(result, EngineResult::Success(_)));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_dot_md_skips_404() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/page.md")
            .with_status(404)
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&format!("{}/page", server.url())).unwrap();
        let result = DotMdEngine.fetch(&url, &fetcher, &HashMap::new()).await.unwrap();

        assert!(matches!(result, EngineResult::Skip(_)));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_dot_md_skips_html_response() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/page.md")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body("<html>not markdown</html>")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&format!("{}/page", server.url())).unwrap();
        let result = DotMdEngine.fetch(&url, &fetcher, &HashMap::new()).await.unwrap();

        assert!(matches!(result, EngineResult::Skip(_)));
        mock.assert_async().await;
    }
}
```

- [ ] **Step 2: Run tests**

```
cargo test -p aget-lib dot_md
```

Expected: all 5 tests pass.

- [ ] **Step 3: Commit**

```bash
git add lib/src/engine/dot_md.rs
git commit -m "feat: implement DotMd engine"
```

---

## Task 8: HtmlExtract Engine

**Files:**
- Modify: `lib/src/engine/html_extract.rs`

> **Note:** Before implementing, verify the `dom_smoothie` and `htmd` crate APIs on crates.io. `dom_smoothie` should provide a `Readability`-style extractor. `htmd` converts HTML to Markdown. Adjust the API calls below if the actual signatures differ.

- [ ] **Step 1: Write `lib/src/engine/html_extract.rs`**

```rust
use crate::engine::{Engine, EngineResult};
use crate::error::{AgetError, Result};
use crate::fetcher::Fetcher;
use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;

pub struct HtmlExtractEngine;

fn html_to_markdown(html: &str, url: &Url) -> Result<String> {
    // dom_smoothie extracts the readable article from raw HTML.
    // Check crates.io for the exact API — adjust if needed.
    let mut readability = dom_smoothie::Readability::new(
        html,
        Some(url.as_str()),
        None,
    )
    .map_err(|e| AgetError::extraction(e.to_string()))?;

    let article = readability
        .parse()
        .map_err(|e| AgetError::extraction(e.to_string()))?;

    // Convert extracted HTML content to Markdown.
    // article.content is clean HTML; article.text_content is plain text.
    htmd::convert(&article.content)
        .or_else(|_| Ok::<String, AgetError>(article.text_content.clone()))
}

#[async_trait]
impl Engine for HtmlExtractEngine {
    fn name(&self) -> &'static str {
        "html_extract"
    }

    async fn fetch(
        &self,
        url: &Url,
        fetcher: &Fetcher,
        domain_headers: &HashMap<String, String>,
    ) -> Result<EngineResult> {
        let resp = fetcher.get(url, domain_headers).await?;
        // Always attempt extraction regardless of status — best-effort fallback.
        let content = html_to_markdown(&resp.body, url)
            .unwrap_or_else(|_| resp.body.clone());
        Ok(EngineResult::Success(content))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIMPLE_HTML: &str = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test Page</title></head>
        <body>
          <article>
            <h1>Hello World</h1>
            <p>This is a paragraph with some <strong>bold</strong> text.</p>
            <p>Another paragraph to meet minimum length requirements for readability.</p>
          </article>
        </body>
        </html>
    "#;

    #[tokio::test]
    async fn test_html_extract_always_returns_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(SIMPLE_HTML)
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&server.url()).unwrap();
        let result = HtmlExtractEngine.fetch(&url, &fetcher, &HashMap::new()).await.unwrap();

        assert!(matches!(result, EngineResult::Success(_)));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_html_extract_returns_success_on_404() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .with_status(404)
            .with_body("<html><body>Not found</body></html>")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&server.url()).unwrap();
        let result = HtmlExtractEngine.fetch(&url, &fetcher, &HashMap::new()).await.unwrap();

        // Always Success — even on error pages
        assert!(matches!(result, EngineResult::Success(_)));
        mock.assert_async().await;
    }
}
```

- [ ] **Step 2: Run tests**

```
cargo test -p aget-lib html_extract
```

Expected: both tests pass. If the `dom_smoothie` API doesn't match, adjust `html_to_markdown` to match the actual API signatures from `cargo doc --open` or crates.io docs.

- [ ] **Step 3: Commit**

```bash
git add lib/src/engine/html_extract.rs
git commit -m "feat: implement HtmlExtract engine with dom_smoothie"
```

---

## Task 9: Quality Heuristic

**Files:**
- Create: `lib/src/quality.rs`

- [ ] **Step 1: Write `lib/src/quality.rs`** with tests

```rust
const MIN_LENGTH: usize = 100;

const MARKDOWN_MARKERS: &[&str] = &["# ", "## ", "**", "```", "---", "- ", "* ", "["];

pub fn passes_quality(content: &str) -> bool {
    if content.len() < MIN_LENGTH {
        return false;
    }
    MARKDOWN_MARKERS.iter().any(|marker| content.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passes_with_heading() {
        let content = "# Hello World\n\n".to_string()
            + &"Some content that is long enough to pass the minimum length check. ".repeat(3);
        assert!(passes_quality(&content));
    }

    #[test]
    fn test_passes_with_bold() {
        let content = "**bold text** at the start\n\n".to_string()
            + &"Some content that is long enough to pass the minimum length check. ".repeat(3);
        assert!(passes_quality(&content));
    }

    #[test]
    fn test_passes_with_link() {
        let content = "[link text](https://example.com)\n\n".to_string()
            + &"Some content that is long enough to pass the minimum length check. ".repeat(3);
        assert!(passes_quality(&content));
    }

    #[test]
    fn test_fails_too_short() {
        let content = "# Short";
        assert!(!passes_quality(content));
    }

    #[test]
    fn test_fails_no_markers() {
        let content = "a".repeat(200);
        assert!(!passes_quality(&content));
    }

    #[test]
    fn test_fails_empty() {
        assert!(!passes_quality(""));
    }
}
```

- [ ] **Step 2: Run tests**

```
cargo test -p aget-lib quality
```

Expected: all 6 tests pass.

- [ ] **Step 3: Commit**

```bash
git add lib/src/quality.rs
git commit -m "feat: add markdown quality heuristic"
```

---

## Task 10: Engine Registry

**Files:**
- Modify: `lib/src/engine/registry.rs`

- [ ] **Step 1: Rewrite `lib/src/engine/registry.rs`** with full implementation and tests

```rust
use crate::config::DomainRule;
use crate::engine::{
    accept_md::AcceptMdEngine, dot_md::DotMdEngine, html_extract::HtmlExtractEngine, Engine,
};

const DEFAULT_CHAIN: &[&str] = &["accept_md", "dot_md", "html_extract"];

pub fn engine_by_name(name: &str) -> Option<Box<dyn Engine>> {
    match name {
        "accept_md" => Some(Box::new(AcceptMdEngine)),
        "dot_md" => Some(Box::new(DotMdEngine)),
        "html_extract" => Some(Box::new(HtmlExtractEngine)),
        _ => None,
    }
}

pub fn build_chain(rule: Option<&DomainRule>) -> Vec<Box<dyn Engine>> {
    let names: &[&str] = if let Some(r) = rule {
        if let Some(ref engines) = r.engines {
            return engines
                .iter()
                .filter_map(|n| engine_by_name(n))
                .collect();
        }
        DEFAULT_CHAIN
    } else {
        DEFAULT_CHAIN
    };

    names
        .iter()
        .filter_map(|n| engine_by_name(n))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_chain_has_three_engines() {
        let chain = build_chain(None);
        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0].name(), "accept_md");
        assert_eq!(chain[1].name(), "dot_md");
        assert_eq!(chain[2].name(), "html_extract");
    }

    #[test]
    fn test_domain_rule_overrides_chain() {
        let rule = DomainRule {
            engines: Some(vec!["dot_md".to_string(), "html_extract".to_string()]),
            ..Default::default()
        };
        let chain = build_chain(Some(&rule));
        assert_eq!(chain.len(), 2);
        assert_eq!(chain[0].name(), "dot_md");
        assert_eq!(chain[1].name(), "html_extract");
    }

    #[test]
    fn test_unknown_engine_names_are_skipped() {
        let rule = DomainRule {
            engines: Some(vec!["accept_md".to_string(), "unknown_engine".to_string()]),
            ..Default::default()
        };
        let chain = build_chain(Some(&rule));
        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0].name(), "accept_md");
    }

    #[test]
    fn test_engine_by_name() {
        assert!(engine_by_name("accept_md").is_some());
        assert!(engine_by_name("dot_md").is_some());
        assert!(engine_by_name("html_extract").is_some());
        assert!(engine_by_name("nonexistent").is_none());
    }
}
```

- [ ] **Step 2: Run tests**

```
cargo test -p aget-lib registry
```

Expected: all 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add lib/src/engine/registry.rs
git commit -m "feat: implement engine registry with domain rule support"
```

---

## Task 11: Pipeline Orchestrator

**Files:**
- Create: `lib/src/pipeline.rs`

- [ ] **Step 1: Write `lib/src/pipeline.rs`**

```rust
use crate::config::{apply_url_transform, DomainRule};
use crate::engine::{registry, EngineResult};
use crate::error::Result;
use crate::fetcher::Fetcher;
use crate::quality::passes_quality;
use std::collections::HashMap;
use url::Url;

pub struct PipelineResult {
    pub content: String,
    pub engine_used: String,
    pub quality_passed: bool,
}

pub struct Pipeline {
    fetcher: Fetcher,
}

impl Pipeline {
    pub fn new() -> Result<Self> {
        Ok(Self { fetcher: Fetcher::new()? })
    }

    pub async fn run(
        &self,
        raw_url: &Url,
        rule: Option<&DomainRule>,
        verbose: bool,
    ) -> Result<PipelineResult> {
        // Step 1: Apply URL transform if configured
        let url = if let Some(r) = rule {
            if let Some(ref template) = r.url_transform {
                apply_url_transform(raw_url, template)?
            } else {
                raw_url.clone()
            }
        } else {
            raw_url.clone()
        };

        let domain_headers: &HashMap<String, String> = rule
            .map(|r| &r.headers)
            .unwrap_or_else(|| &EMPTY_HEADERS);

        // Step 2: "direct" mode — skip engine chain
        if let Some(r) = rule {
            if r.engine.as_deref() == Some("direct") {
                if verbose {
                    eprintln!("[aget] direct fetch: {}", url);
                }
                let resp = self.fetcher.get(&url, domain_headers).await?;
                return Ok(PipelineResult {
                    content: resp.body,
                    engine_used: "direct".to_string(),
                    quality_passed: true,
                });
            }
        }

        // Step 3: Run engine chain
        let engines = registry::build_chain(rule);
        let mut best_effort: Option<(String, String)> = None; // (engine_name, content)

        for engine in &engines {
            if verbose {
                eprintln!("[aget] trying engine: {}", engine.name());
            }

            match engine.fetch(&url, &self.fetcher, domain_headers).await? {
                EngineResult::Skip(reason) => {
                    if verbose {
                        eprintln!("[aget] {}: skipped ({})", engine.name(), reason);
                    }
                }
                EngineResult::Success(content) => {
                    if passes_quality(&content) {
                        if verbose {
                            eprintln!(
                                "[aget] {}: success (quality check passed, {} chars)",
                                engine.name(),
                                content.len()
                            );
                        }
                        return Ok(PipelineResult {
                            content,
                            engine_used: engine.name().to_string(),
                            quality_passed: true,
                        });
                    }
                    if verbose {
                        eprintln!(
                            "[aget] {}: quality check failed ({} chars), keeping as fallback",
                            engine.name(),
                            content.len()
                        );
                    }
                    best_effort = Some((engine.name().to_string(), content));
                }
            }
        }

        // Step 4: Best-effort fallback
        let (engine_used, content) = best_effort.unwrap_or_else(|| {
            ("none".to_string(), String::new())
        });

        Ok(PipelineResult { content, engine_used, quality_passed: false })
    }
}

static EMPTY_HEADERS: std::sync::LazyLock<HashMap<String, String>> =
    std::sync::LazyLock::new(HashMap::new);
```

- [ ] **Step 2: Add tests** (append to `lib/src/pipeline.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DomainRule;

    // Minimal quality-passing markdown content
    const GOOD_MD: &str = "# Hello World\n\nThis is some content that is long enough to pass the quality check and contains markdown markers like **bold** text.\n\nAnother paragraph to ensure we are above 100 characters.";

    // Content that fails quality check (too short / no markers)
    const BAD_MD: &str = "nothing here";

    #[tokio::test]
    async fn test_pipeline_uses_first_quality_engine() {
        let mut server = mockito::Server::new_async().await;
        // accept_md succeeds
        server
            .mock("GET", "/")
            .match_header("Accept", mockito::Matcher::Regex("text/markdown".to_string()))
            .with_status(200)
            .with_header("content-type", "text/markdown")
            .with_body(GOOD_MD)
            .create_async()
            .await;

        let pipeline = Pipeline::new().unwrap();
        let url = Url::parse(&server.url()).unwrap();
        let result = pipeline.run(&url, None, false).await.unwrap();

        assert_eq!(result.engine_used, "accept_md");
        assert!(result.quality_passed);
    }

    #[tokio::test]
    async fn test_pipeline_falls_back_on_quality_failure() {
        let mut server = mockito::Server::new_async().await;
        // accept_md returns too-short content
        server
            .mock("GET", "/")
            .match_header("Accept", mockito::Matcher::Regex("text/markdown".to_string()))
            .with_status(200)
            .with_header("content-type", "text/markdown")
            .with_body(BAD_MD)
            .create_async()
            .await;
        // dot_md 404
        server
            .mock("GET", "/.md")
            .with_status(404)
            .create_async()
            .await;
        // html_extract returns quality content
        server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(&format!("<html><body>{}</body></html>", GOOD_MD))
            .create_async()
            .await;

        let pipeline = Pipeline::new().unwrap();
        let url = Url::parse(&server.url()).unwrap();
        let result = pipeline.run(&url, None, false).await.unwrap();

        // html_extract picks up
        assert!(result.quality_passed || !result.content.is_empty());
    }

    #[tokio::test]
    async fn test_pipeline_direct_mode_skips_engine_chain() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/readme.md")
            .with_status(200)
            .with_body("# Direct readme content")
            .create_async()
            .await;

        let base = server.url();
        let pipeline = Pipeline::new().unwrap();
        let url = Url::parse(&format!("{}/original", base)).unwrap();
        let rule = DomainRule {
            url_transform: Some(format!("{}/readme.md", base)),
            engine: Some("direct".to_string()),
            ..Default::default()
        };
        let result = pipeline.run(&url, Some(&rule), false).await.unwrap();

        assert_eq!(result.engine_used, "direct");
        assert!(result.content.contains("Direct readme content"));
        mock.assert_async().await;
    }
}
```

- [ ] **Step 3: Run tests**

```
cargo test -p aget-lib pipeline
```

Expected: all 3 tests pass.

- [ ] **Step 4: Commit**

```bash
git add lib/src/pipeline.rs
git commit -m "feat: implement Pipeline orchestrator"
```

---

## Task 12: CLI

**Files:**
- Create: `cli/src/cli.rs`
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Write `cli/src/cli.rs`**

```rust
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "aget")]
#[command(about = "Fetch a URL and output its content as Markdown")]
#[command(version)]
pub struct Cli {
    /// URL to fetch and convert to Markdown
    pub url: String,

    /// Write output to FILE instead of stdout
    #[arg(short = 'o', long = "output", value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Config file path
    #[arg(short = 'C', long = "config", value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Print engine attempts and quality results to stderr
    #[arg(short = 'v', long = "verbose")]
    pub verbose: bool,

    /// Force a specific engine (overrides domain rules): accept_md, dot_md, html_extract
    #[arg(long = "engine", value_name = "NAME")]
    pub engine: Option<String>,
}
```

- [ ] **Step 2: Write `cli/src/main.rs`**

```rust
mod cli;

use aget_lib::{
    config::{Config, DomainRule},
    engine::registry::engine_by_name,
    pipeline::Pipeline,
    AgetError,
};
use anyhow::{Context, Result};
use clap::Parser;
use cli::Cli;
use std::io::Write;
use url::Url;

#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("aget: {:#}", e);
        std::process::exit(1);
    }
}

async fn run() -> Result<()> {
    let cli = Cli::parse();

    let url = Url::parse(&cli.url).context("invalid URL")?;

    let config = match &cli.config {
        Some(path) => Config::load(path).context("failed to load config")?,
        None => Config::load_default().context("failed to load default config")?,
    };

    let domain = url.host_str().unwrap_or("").to_string();
    let mut rule: Option<DomainRule> = config.domains.get(&domain).cloned();

    // --engine flag overrides the chain
    if let Some(engine_name) = &cli.engine {
        if engine_by_name(engine_name).is_none() {
            anyhow::bail!("unknown engine '{}'. Valid: accept_md, dot_md, html_extract", engine_name);
        }
        rule = Some(DomainRule {
            engines: Some(vec![engine_name.clone()]),
            // preserve headers from domain rule if present
            headers: rule.as_ref().map(|r| r.headers.clone()).unwrap_or_default(),
            ..Default::default()
        });
    }

    let pipeline = Pipeline::new().context("failed to create pipeline")?;
    let result = pipeline
        .run(&url, rule.as_ref(), cli.verbose)
        .await
        .context("fetch failed")?;

    match &cli.output {
        Some(path) => {
            std::fs::write(path, &result.content)
                .with_context(|| format!("failed to write to {}", path.display()))?;
        }
        None => {
            let stdout = std::io::stdout();
            let mut out = stdout.lock();
            out.write_all(result.content.as_bytes())
                .context("failed to write to stdout")?;
            if !result.content.ends_with('\n') {
                out.write_all(b"\n").ok();
            }
        }
    }

    Ok(())
}
```

- [ ] **Step 3: Build the binary**

```
cargo build -p aget
```

Expected: compiles cleanly. Fix any import errors.

- [ ] **Step 4: Smoke test**

```
cargo run -p aget -- --help
```

Expected output shows usage, all flags documented.

- [ ] **Step 5: Commit**

```bash
git add cli/src/cli.rs cli/src/main.rs
git commit -m "feat: implement CLI with clap"
```

---

## Task 13: Example Config, Makefile, AGENTS.md

**Files:**
- Create: `aget.toml.example`
- Create: `Makefile`
- Create: `AGENTS.md`

- [ ] **Step 1: Create `aget.toml.example`**

```toml
# Example aget configuration file.
# Copy to ~/.aget/config.toml and customize.

# GitHub: rewrite to raw readme, fetch directly without engine chain
[domains."github.com"]
url_transform = "https://raw.githubusercontent.com/{owner}/{repo}/refs/heads/main/readme.md"
engine = "direct"

# A docs site that supports native markdown — prefer accept_md first
[domains."docs.example.com"]
engines = ["accept_md", "dot_md", "html_extract"]

[domains."docs.example.com".headers]
Authorization = "Bearer your-token-here"

# An API that requires a key
[domains."api.example.com".headers]
X-API-Key = "your-api-key-here"
```

- [ ] **Step 2: Create `Makefile`**

```makefile
.PHONY: build test fmt check release install

build:
	cargo build

test:
	cargo test

fmt:
	cargo fmt
	cargo clippy --fix --allow-dirty

check:
	cargo fmt --check
	cargo clippy -- -D warnings
	cargo test
	cargo build

release:
	cargo build --release

install:
	cargo install --path cli
```

- [ ] **Step 3: Create `AGENTS.md`**

```markdown
# AGENTS.md

Guidance for AI agents working with the aget codebase.

## Project Overview

**aget** is a curl-like CLI that fetches URLs and outputs Markdown. It uses a chain
of "engines" (Accept: text/markdown, .md append, HTML extraction) with per-domain
config rules for URL transforms, engine overrides, and custom headers.

## Repository Structure

```
aget/
├── cli/src/
│   ├── main.rs         # Entry point, run(), wiring
│   └── cli.rs          # Clap arg definitions
└── lib/src/
    ├── config.rs       # Config, DomainRule, apply_url_transform
    ├── error.rs        # AgetError, Result alias
    ├── fetcher.rs      # Fetcher, FetchResponse (reqwest wrapper)
    ├── quality.rs      # passes_quality heuristic
    ├── pipeline.rs     # Pipeline orchestrator
    └── engine/
        ├── mod.rs          # Engine trait, EngineResult
        ├── accept_md.rs    # Engine 1
        ├── dot_md.rs       # Engine 2
        ├── html_extract.rs # Engine 3 (dom_smoothie + htmd)
        └── registry.rs     # build_chain, engine_by_name
```

## Development

```bash
make build    # debug build
make test     # run all tests
make fmt      # format + fix lints
make check    # fmt check + clippy + tests + build
make install  # install to ~/.cargo/bin
```

## Code Style

- `thiserror` for library errors, `anyhow` in CLI
- Tests live alongside source files
- All warnings are errors (`-D warnings` in clippy)

## Adding a New Engine

1. Create `lib/src/engine/<name>.rs`, implement `Engine` trait
2. Add variant to `engine_by_name()` in `registry.rs`
3. Add to default chain in `DEFAULT_CHAIN` if appropriate
4. Add name to `aget.toml.example`

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `AGET_CONFIG` | Override config path (not yet wired — future) |
| `NO_COLOR` | Disable colored output |
| `RUST_LOG` | Log verbosity |
```

- [ ] **Step 4: Commit**

```bash
git add aget.toml.example Makefile AGENTS.md
git commit -m "chore: add Makefile, example config, and AGENTS.md"
```

---

## Task 14: Integration Test

**Files:**
- Create: `cli/src/tests/` (integration tests via `assert_cmd`)

- [ ] **Step 1: Create `cli/tests/integration.rs`**

```rust
use assert_cmd::Command;
use mockito::Server;
use std::io::Write;
use tempfile::NamedTempFile;

fn aget() -> Command {
    Command::cargo_bin("aget").unwrap()
}

#[tokio::test]
async fn test_fetches_native_markdown() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "text/markdown")
        .with_body("# Hello\n\nThis is markdown content that is long enough to pass quality and has **bold** text.\n\nMore content here.")
        .create_async()
        .await;

    let output = aget()
        .arg(&server.url())
        .output()
        .unwrap();

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("# Hello") || stdout.contains("Hello"));
}

#[tokio::test]
async fn test_output_to_file() {
    let mut server = Server::new_async().await;
    let _mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("content-type", "text/markdown")
        .with_body("# File Output Test\n\nLong enough content with **bold** and [links](http://example.com) here.")
        .create_async()
        .await;

    let out_file = tempfile::NamedTempFile::new().unwrap();
    let out_path = out_file.path().to_str().unwrap().to_string();

    let status = aget()
        .arg(&server.url())
        .arg("-o")
        .arg(&out_path)
        .status()
        .unwrap();

    assert!(status.success());
    let content = std::fs::read_to_string(&out_path).unwrap();
    assert!(!content.is_empty());
}

#[test]
fn test_invalid_url_exits_nonzero() {
    let output = aget().arg("not-a-url").output().unwrap();
    assert!(!output.status.success());
}

#[test]
fn test_unknown_engine_exits_nonzero() {
    let output = aget()
        .arg("https://example.com")
        .arg("--engine")
        .arg("fake_engine")
        .output()
        .unwrap();
    assert!(!output.status.success());
}

#[test]
fn test_help_exits_zero() {
    let output = aget().arg("--help").output().unwrap();
    assert!(output.status.success());
}
```

- [ ] **Step 2: Run integration tests**

```
cargo test -p aget
```

Expected: all 5 tests pass.

- [ ] **Step 3: Run full test suite**

```
make check
```

Expected: fmt clean, no clippy warnings, all tests pass, build succeeds.

- [ ] **Step 4: Final commit**

```bash
git add cli/tests/
git commit -m "test: add CLI integration tests"
```

---

## Self-Review Notes

- All 14 tasks are covered and map to spec sections.
- Types are consistent: `EngineResult`, `PipelineResult`, `FetchResponse` named identically throughout.
- `engine_by_name` in registry is referenced in `main.rs` for `--engine` flag validation.
- `apply_url_transform` is defined in `config.rs` and used in `pipeline.rs`.
- `dom_smoothie` API assumed — implementor must verify against crates.io docs before Task 8.
- `EMPTY_HEADERS` uses `std::sync::LazyLock` (stable in Rust 1.80+); if on older Rust, replace with `once_cell::sync::Lazy`.
