# HTTP Caching Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Introduce a `Fetch` trait + `CachingFetcher` that eliminates redundant within-run HTTP requests and caches responses on disk using HTTP cache semantics (ETag, Cache-Control, Expires).

**Architecture:** A new `Fetch` trait replaces `&Fetcher` in the Engine interface. `CachingFetcher` wraps `Fetcher + Cache` and implements `Fetch`, storing responses in SQLite at `~/.aget/cache.db`. `Pipeline` holds `Box<dyn Fetch>` and constructs `CachingFetcher` by default, falling back to bare `Fetcher` on DB open failure or when `--no-cache` is passed.

**Tech Stack:** Rust, `rusqlite` (bundled), `sha2` (cache key hashing), `httpdate` (Expires header parsing), `async-trait`, `mockito` (tests).

---

## File Map

| File | Action | Responsibility |
|---|---|---|
| `lib/Cargo.toml` | Modify | Add rusqlite, sha2, httpdate dependencies |
| `lib/src/fetch.rs` | Create | `Fetch` trait definition |
| `lib/src/fetcher.rs` | Modify | `FetchResponse` gains caching headers; `Fetcher` implements `Fetch` |
| `lib/src/error.rs` | Modify | Add `Cache(rusqlite::Error)` variant |
| `lib/src/cache.rs` | Create | `Cache` struct, SQLite schema, freshness logic, store/retrieve |
| `lib/src/caching_fetcher.rs` | Create | `CachingFetcher` wraps `Fetcher + Cache`, implements `Fetch` |
| `lib/src/engine/mod.rs` | Modify | `Engine::fetch()` takes `&dyn Fetch` instead of `&Fetcher` |
| `lib/src/engine/accept_md.rs` | Modify | Update trait signature only |
| `lib/src/engine/dot_md.rs` | Modify | Update trait signature only |
| `lib/src/engine/html_extract.rs` | Modify | Update trait signature only |
| `lib/src/pipeline.rs` | Modify | Hold `Box<dyn Fetch>`, accept `no_cache: bool` |
| `lib/src/lib.rs` | Modify | Export new modules |
| `cli/src/cli.rs` | Modify | Add `--no-cache` flag |
| `cli/src/main.rs` | Modify | Pass `cli.no_cache` to `Pipeline::new()` |

---

## Task 1: Add dependencies

**Files:**
- Modify: `lib/Cargo.toml`

- [ ] **Step 1: Add rusqlite, sha2, and httpdate to lib/Cargo.toml**

Replace the `[dependencies]` section of `lib/Cargo.toml` with:

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
rusqlite = { version = "0.32", features = ["bundled"] }
sha2 = "0.10"
httpdate = "1"

[dev-dependencies]
tokio.workspace = true
mockito = "1"
tempfile = "3"
```

- [ ] **Step 2: Verify it compiles**

Run:
```
cargo check -p aget-lib
```
Expected: no errors (just unused dependency warnings at most).

- [ ] **Step 3: Commit**

```
git add lib/Cargo.toml
git commit -m "chore: add rusqlite, sha2, httpdate dependencies"
```

---

## Task 2: Create `Fetch` trait

**Files:**
- Create: `lib/src/fetch.rs`
- Modify: `lib/src/lib.rs`

- [ ] **Step 1: Create `lib/src/fetch.rs`**

```rust
use crate::error::Result;
use crate::fetcher::FetchResponse;
use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;

#[async_trait]
pub trait Fetch: Send + Sync {
    async fn get(&self, url: &Url, headers: &HashMap<String, String>) -> Result<FetchResponse>;
}
```

- [ ] **Step 2: Export from `lib/src/lib.rs`**

Add `pub mod fetch;` so the file becomes:

```rust
pub mod cache;
pub mod caching_fetcher;
pub mod config;
pub mod engine;
pub mod error;
pub mod fetch;
pub mod fetcher;
pub mod pipeline;
pub mod quality;

pub use config::Config;
pub use error::{AgetError, Result};
pub use pipeline::{Pipeline, PipelineResult};
```

(`cache` and `caching_fetcher` are added here too — the files don't exist yet, but adding them now avoids a second edit. If the compiler errors on the missing files, temporarily omit those two lines and add them back in Tasks 4 and 5.)

- [ ] **Step 3: Verify it compiles**

Run:
```
cargo check -p aget-lib
```
Expected: error about missing `cache` and `caching_fetcher` modules (if you added them). If so, temporarily remove those two lines and add them back later.

- [ ] **Step 4: Commit**

```
git add lib/src/fetch.rs lib/src/lib.rs
git commit -m "feat: add Fetch trait"
```

---

## Task 3: Extend `FetchResponse` and implement `Fetch` for `Fetcher`

**Files:**
- Modify: `lib/src/fetcher.rs`

- [ ] **Step 1: Write failing tests for new caching header fields**

Add these tests inside the existing `#[cfg(test)]` block in `lib/src/fetcher.rs`:

```rust
#[tokio::test]
async fn test_fetcher_captures_etag() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("etag", "\"abc123\"")
        .with_body("content")
        .create_async()
        .await;

    let fetcher = Fetcher::new().unwrap();
    let url = Url::parse(&server.url()).unwrap();
    let resp = fetcher.get(&url, &HashMap::new()).await.unwrap();

    assert_eq!(resp.etag.as_deref(), Some("\"abc123\""));
    mock.assert_async().await;
}

#[tokio::test]
async fn test_fetcher_captures_cache_control() {
    let mut server = mockito::Server::new_async().await;
    let mock = server
        .mock("GET", "/")
        .with_status(200)
        .with_header("cache-control", "max-age=3600")
        .with_body("content")
        .create_async()
        .await;

    let fetcher = Fetcher::new().unwrap();
    let url = Url::parse(&server.url()).unwrap();
    let resp = fetcher.get(&url, &HashMap::new()).await.unwrap();

    assert_eq!(resp.cache_control.as_deref(), Some("max-age=3600"));
    mock.assert_async().await;
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:
```
cargo test -p aget-lib fetcher
```
Expected: compile error — `FetchResponse` has no `etag` or `cache_control` field.

- [ ] **Step 3: Update `FetchResponse` struct**

Replace the current `FetchResponse` struct definition:

```rust
pub struct FetchResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub cache_control: Option<String>,
    pub expires: Option<String>,
}
```

- [ ] **Step 4: Update `Fetcher::get()` to parse caching headers**

Replace the `get()` method body (keep the signature, replace from `let mut header_map` to the end):

```rust
pub async fn get(&self, url: &Url, headers: &HashMap<String, String>) -> Result<FetchResponse> {
    let mut header_map = HeaderMap::new();
    for (k, v) in headers {
        if let (Ok(name), Ok(value)) = (HeaderName::from_str(k), HeaderValue::from_str(v)) {
            header_map.insert(name, value);
        }
    }

    let response = self
        .client
        .get(url.as_str())
        .headers(header_map)
        .send()
        .await?;

    let status = response.status().as_u16();

    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.split(';').next().unwrap_or(s).trim().to_string());

    let etag = response
        .headers()
        .get(reqwest::header::ETAG)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let last_modified = response
        .headers()
        .get(reqwest::header::LAST_MODIFIED)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let cache_control = response
        .headers()
        .get(reqwest::header::CACHE_CONTROL)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let expires = response
        .headers()
        .get(reqwest::header::EXPIRES)
        .and_then(|v| v.to_str().ok())
        .map(String::from);

    let body = response.text().await?;

    Ok(FetchResponse {
        status,
        content_type,
        body,
        etag,
        last_modified,
        cache_control,
        expires,
    })
}
```

- [ ] **Step 5: Add `impl Fetch for Fetcher`**

Add this block after the `impl Fetcher` block (requires adding the import at the top of the file):

Add to imports at the top:
```rust
use crate::fetch::Fetch;
```

Add after the `impl Fetcher` block:
```rust
#[async_trait::async_trait]
impl Fetch for Fetcher {
    async fn get(&self, url: &Url, headers: &HashMap<String, String>) -> Result<FetchResponse> {
        self.get(url, headers).await
    }
}
```

- [ ] **Step 6: Run tests to verify they pass**

Run:
```
cargo test -p aget-lib fetcher
```
Expected: all tests pass including the two new ones.

- [ ] **Step 7: Commit**

```
git add lib/src/fetcher.rs
git commit -m "feat: extend FetchResponse with caching headers, impl Fetch for Fetcher"
```

---

## Task 4: Add `Cache` error variant and create `Cache` struct

**Files:**
- Modify: `lib/src/error.rs`
- Create: `lib/src/cache.rs`

- [ ] **Step 1: Add `Cache` variant to `AgetError`**

Replace `lib/src/error.rs` with:

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

    #[error("Cache error: {0}")]
    Cache(#[from] rusqlite::Error),
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

- [ ] **Step 2: Write failing tests for `Cache`**

Create `lib/src/cache.rs` with the tests only:

```rust
use crate::error::Result;
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

const DEFAULT_TTL: i64 = 3600;

pub struct Cache {
    conn: Mutex<Connection>,
}

pub struct CacheEntry {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub max_age_secs: Option<i64>,
    pub cached_at: i64,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cache() -> Cache {
        Cache::open_in_memory().unwrap()
    }

    #[test]
    fn test_miss_returns_none() {
        let cache = make_cache();
        assert!(cache.get("https://example.com/", "hash").unwrap().is_none());
    }

    #[test]
    fn test_store_and_retrieve() {
        let cache = make_cache();
        let entry = CacheEntry {
            status: 200,
            content_type: Some("text/markdown".to_string()),
            body: "# Hello".to_string(),
            etag: Some("\"abc123\"".to_string()),
            last_modified: None,
            max_age_secs: Some(3600),
            cached_at: unix_now(),
        };
        cache.store("https://example.com/", "hash123", &entry).unwrap();
        let retrieved = cache.get("https://example.com/", "hash123").unwrap().unwrap();
        assert_eq!(retrieved.body, "# Hello");
        assert_eq!(retrieved.etag.as_deref(), Some("\"abc123\""));
        assert_eq!(retrieved.status, 200);
    }

    #[test]
    fn test_different_hash_misses() {
        let cache = make_cache();
        let entry = CacheEntry {
            status: 200,
            content_type: None,
            body: "body".to_string(),
            etag: None,
            last_modified: None,
            max_age_secs: Some(3600),
            cached_at: unix_now(),
        };
        cache.store("https://example.com/", "hash-a", &entry).unwrap();
        assert!(cache.get("https://example.com/", "hash-b").unwrap().is_none());
    }

    #[test]
    fn test_refresh_cached_at() {
        let cache = make_cache();
        let entry = CacheEntry {
            status: 200,
            content_type: None,
            body: "hello".to_string(),
            etag: None,
            last_modified: None,
            max_age_secs: Some(0),
            cached_at: 1000,
        };
        cache.store("https://example.com/", "hash", &entry).unwrap();
        cache.refresh_cached_at("https://example.com/", "hash").unwrap();
        let retrieved = cache.get("https://example.com/", "hash").unwrap().unwrap();
        assert!(retrieved.cached_at > 1000);
    }

    #[test]
    fn test_is_no_store_true() {
        assert!(is_no_store("no-store"));
        assert!(is_no_store("no-cache, no-store"));
        assert!(is_no_store("no-store, max-age=0"));
    }

    #[test]
    fn test_is_no_store_false() {
        assert!(!is_no_store("max-age=3600"));
        assert!(!is_no_store("no-cache"));
        assert!(!is_no_store(""));
    }

    #[test]
    fn test_compute_max_age_from_max_age_directive() {
        assert_eq!(compute_max_age_secs(Some("max-age=600"), None), Some(600));
        assert_eq!(compute_max_age_secs(Some("max-age=0"), None), Some(0));
        assert_eq!(compute_max_age_secs(Some("public, max-age=3600"), None), Some(3600));
    }

    #[test]
    fn test_compute_max_age_no_cache_is_zero() {
        assert_eq!(compute_max_age_secs(Some("no-cache"), None), Some(0));
    }

    #[test]
    fn test_compute_max_age_none_when_no_headers() {
        assert_eq!(compute_max_age_secs(None, None), None);
        assert_eq!(compute_max_age_secs(Some(""), None), None);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run:
```
cargo test -p aget-lib cache
```
Expected: compile error — `Cache`, `CacheEntry`, `unix_now`, `is_no_store`, `compute_max_age_secs`, `open_in_memory`, `get`, `store`, `refresh_cached_at` are not defined.

- [ ] **Step 4: Implement `Cache`**

Replace the content of `lib/src/cache.rs` (keep the test block, add the implementation above it):

```rust
use crate::error::Result;
use rusqlite::{Connection, OptionalExtension, params};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

const DEFAULT_TTL: i64 = 3600;

pub struct Cache {
    conn: Mutex<Connection>,
}

pub struct CacheEntry {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: String,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub max_age_secs: Option<i64>,
    pub cached_at: i64,
}

impl Cache {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let cache = Self { conn: Mutex::new(conn) };
        cache.init_schema()?;
        Ok(cache)
    }

    pub fn default_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".aget").join("cache.db"))
    }

    #[cfg(test)]
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let cache = Self { conn: Mutex::new(conn) };
        cache.init_schema()?;
        Ok(cache)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.lock().unwrap().execute_batch(
            "CREATE TABLE IF NOT EXISTS entries (
                url                  TEXT NOT NULL,
                request_headers_hash TEXT NOT NULL,
                status               INTEGER NOT NULL,
                content_type         TEXT,
                body                 TEXT NOT NULL,
                etag                 TEXT,
                last_modified        TEXT,
                max_age_secs         INTEGER,
                cached_at            INTEGER NOT NULL,
                PRIMARY KEY (url, request_headers_hash)
            );",
        )?;
        Ok(())
    }

    pub fn get(&self, url: &str, headers_hash: &str) -> Result<Option<CacheEntry>> {
        let conn = self.conn.lock().unwrap();
        let entry = conn.query_row(
            "SELECT status, content_type, body, etag, last_modified, max_age_secs, cached_at
             FROM entries WHERE url = ?1 AND request_headers_hash = ?2",
            params![url, headers_hash],
            |row| {
                Ok(CacheEntry {
                    status: row.get::<_, i64>(0)? as u16,
                    content_type: row.get(1)?,
                    body: row.get(2)?,
                    etag: row.get(3)?,
                    last_modified: row.get(4)?,
                    max_age_secs: row.get(5)?,
                    cached_at: row.get(6)?,
                })
            },
        )
        .optional()?;
        Ok(entry)
    }

    pub fn store(&self, url: &str, headers_hash: &str, entry: &CacheEntry) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT OR REPLACE INTO entries
             (url, request_headers_hash, status, content_type, body, etag, last_modified, max_age_secs, cached_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                url,
                headers_hash,
                entry.status as i64,
                entry.content_type,
                entry.body,
                entry.etag,
                entry.last_modified,
                entry.max_age_secs,
                entry.cached_at,
            ],
        )?;
        Ok(())
    }

    pub fn refresh_cached_at(&self, url: &str, headers_hash: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE entries SET cached_at = ?1 WHERE url = ?2 AND request_headers_hash = ?3",
            params![unix_now(), url, headers_hash],
        )?;
        Ok(())
    }
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

/// Returns true if the response must not be stored.
pub fn is_no_store(cache_control: &str) -> bool {
    cache_control
        .split(',')
        .any(|d| d.trim() == "no-store")
}

/// Returns the effective max-age in seconds, or None to use DEFAULT_TTL.
/// Returns Some(0) for no-cache (always revalidate).
pub fn compute_max_age_secs(cache_control: Option<&str>, expires: Option<&str>) -> Option<i64> {
    let cc = cache_control.unwrap_or("");

    if cc.split(',').any(|d| d.trim() == "no-cache") {
        return Some(0);
    }

    for directive in cc.split(',') {
        if let Some(val) = directive.trim().strip_prefix("max-age=") {
            if let Ok(secs) = val.trim().parse::<i64>() {
                return Some(secs);
            }
        }
    }

    if let Some(exp) = expires {
        if let Ok(expires_time) = httpdate::parse_http_date(exp) {
            let remaining = expires_time
                .duration_since(SystemTime::now())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            return Some(remaining.max(0));
        }
    }

    None
}

pub fn effective_max_age(entry: &CacheEntry) -> i64 {
    entry.max_age_secs.unwrap_or(DEFAULT_TTL)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_cache() -> Cache {
        Cache::open_in_memory().unwrap()
    }

    #[test]
    fn test_miss_returns_none() {
        let cache = make_cache();
        assert!(cache.get("https://example.com/", "hash").unwrap().is_none());
    }

    #[test]
    fn test_store_and_retrieve() {
        let cache = make_cache();
        let entry = CacheEntry {
            status: 200,
            content_type: Some("text/markdown".to_string()),
            body: "# Hello".to_string(),
            etag: Some("\"abc123\"".to_string()),
            last_modified: None,
            max_age_secs: Some(3600),
            cached_at: unix_now(),
        };
        cache.store("https://example.com/", "hash123", &entry).unwrap();
        let retrieved = cache.get("https://example.com/", "hash123").unwrap().unwrap();
        assert_eq!(retrieved.body, "# Hello");
        assert_eq!(retrieved.etag.as_deref(), Some("\"abc123\""));
        assert_eq!(retrieved.status, 200);
    }

    #[test]
    fn test_different_hash_misses() {
        let cache = make_cache();
        let entry = CacheEntry {
            status: 200,
            content_type: None,
            body: "body".to_string(),
            etag: None,
            last_modified: None,
            max_age_secs: Some(3600),
            cached_at: unix_now(),
        };
        cache.store("https://example.com/", "hash-a", &entry).unwrap();
        assert!(cache.get("https://example.com/", "hash-b").unwrap().is_none());
    }

    #[test]
    fn test_refresh_cached_at() {
        let cache = make_cache();
        let entry = CacheEntry {
            status: 200,
            content_type: None,
            body: "hello".to_string(),
            etag: None,
            last_modified: None,
            max_age_secs: Some(0),
            cached_at: 1000,
        };
        cache.store("https://example.com/", "hash", &entry).unwrap();
        cache.refresh_cached_at("https://example.com/", "hash").unwrap();
        let retrieved = cache.get("https://example.com/", "hash").unwrap().unwrap();
        assert!(retrieved.cached_at > 1000);
    }

    #[test]
    fn test_is_no_store_true() {
        assert!(is_no_store("no-store"));
        assert!(is_no_store("no-cache, no-store"));
        assert!(is_no_store("no-store, max-age=0"));
    }

    #[test]
    fn test_is_no_store_false() {
        assert!(!is_no_store("max-age=3600"));
        assert!(!is_no_store("no-cache"));
        assert!(!is_no_store(""));
    }

    #[test]
    fn test_compute_max_age_from_max_age_directive() {
        assert_eq!(compute_max_age_secs(Some("max-age=600"), None), Some(600));
        assert_eq!(compute_max_age_secs(Some("max-age=0"), None), Some(0));
        assert_eq!(compute_max_age_secs(Some("public, max-age=3600"), None), Some(3600));
    }

    #[test]
    fn test_compute_max_age_no_cache_is_zero() {
        assert_eq!(compute_max_age_secs(Some("no-cache"), None), Some(0));
    }

    #[test]
    fn test_compute_max_age_none_when_no_headers() {
        assert_eq!(compute_max_age_secs(None, None), None);
        assert_eq!(compute_max_age_secs(Some(""), None), None);
    }
}
```

- [ ] **Step 5: Run tests to verify they pass**

Run:
```
cargo test -p aget-lib cache
```
Expected: all tests pass.

- [ ] **Step 6: Commit**

```
git add lib/src/error.rs lib/src/cache.rs
git commit -m "feat: add Cache struct with SQLite storage and freshness logic"
```

---

## Task 5: Create `CachingFetcher`

**Files:**
- Create: `lib/src/caching_fetcher.rs`
- Modify: `lib/src/lib.rs` (add `pub mod caching_fetcher;` if not already there)

- [ ] **Step 1: Write failing tests**

Create `lib/src/caching_fetcher.rs` with tests only:

```rust
use crate::cache::Cache;
use crate::error::Result;
use crate::fetch::Fetch;
use crate::fetcher::{FetchResponse, Fetcher};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use url::Url;

#[cfg(test)]
mod tests {
    use super::*;

    async fn make_cf() -> CachingFetcher {
        CachingFetcher::with_in_memory_cache().unwrap()
    }

    #[tokio::test]
    async fn test_cache_miss_fetches_and_stores() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/markdown")
            .with_header("cache-control", "max-age=3600")
            .with_body("# Hello")
            .expect(1)
            .create_async()
            .await;

        let cf = make_cf().await;
        let url = Url::parse(&server.url()).unwrap();
        let resp = cf.get(&url, &HashMap::new()).await.unwrap();

        assert_eq!(resp.body, "# Hello");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fresh_cache_hit_skips_network() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/markdown")
            .with_header("cache-control", "max-age=3600")
            .with_body("# Cached")
            .expect(1)
            .create_async()
            .await;

        let cf = make_cf().await;
        let url = Url::parse(&server.url()).unwrap();
        let headers = HashMap::new();

        let resp1 = cf.get(&url, &headers).await.unwrap();
        let resp2 = cf.get(&url, &headers).await.unwrap();

        assert_eq!(resp1.body, "# Cached");
        assert_eq!(resp2.body, "# Cached");
        mock.assert_async().await; // exactly 1 network call
    }

    #[tokio::test]
    async fn test_stale_304_returns_cached_body() {
        let mut server = mockito::Server::new_async().await;

        // Populate cache with immediately-stale entry + ETag
        let mock1 = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/markdown")
            .with_header("cache-control", "max-age=0")
            .with_header("etag", "\"v1\"")
            .with_body("# Original")
            .create_async()
            .await;

        let cf = make_cf().await;
        let url = Url::parse(&server.url()).unwrap();
        let headers = HashMap::new();

        cf.get(&url, &headers).await.unwrap();
        mock1.assert_async().await;

        // Second call: stale → conditional GET → 304
        let mock2 = server
            .mock("GET", "/")
            .match_header("if-none-match", "\"v1\"")
            .with_status(304)
            .create_async()
            .await;

        let resp = cf.get(&url, &headers).await.unwrap();
        assert_eq!(resp.body, "# Original");
        assert_eq!(resp.status, 200);
        mock2.assert_async().await;
    }

    #[tokio::test]
    async fn test_stale_200_replaces_cached_entry() {
        let mut server = mockito::Server::new_async().await;

        // Populate cache with immediately-stale entry + ETag
        server
            .mock("GET", "/")
            .with_status(200)
            .with_header("cache-control", "max-age=0")
            .with_header("etag", "\"v1\"")
            .with_body("# Old")
            .create_async()
            .await;

        let cf = make_cf().await;
        let url = Url::parse(&server.url()).unwrap();
        let headers = HashMap::new();

        cf.get(&url, &headers).await.unwrap();

        // Second call: server returns fresh 200 with new content
        server
            .mock("GET", "/")
            .match_header("if-none-match", "\"v1\"")
            .with_status(200)
            .with_header("cache-control", "max-age=3600")
            .with_header("etag", "\"v2\"")
            .with_body("# New")
            .create_async()
            .await;

        let resp = cf.get(&url, &headers).await.unwrap();
        assert_eq!(resp.body, "# New");

        // Third call should now be a fresh cache hit for the new content
        let mock3 = server
            .mock("GET", "/")
            .expect(0) // should NOT be called
            .create_async()
            .await;

        let resp3 = cf.get(&url, &headers).await.unwrap();
        assert_eq!(resp3.body, "# New");
        mock3.assert_async().await;
    }

    #[tokio::test]
    async fn test_no_store_bypasses_cache() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("cache-control", "no-store")
            .with_body("fresh")
            .expect(2)
            .create_async()
            .await;

        let cf = make_cf().await;
        let url = Url::parse(&server.url()).unwrap();
        let headers = HashMap::new();

        cf.get(&url, &headers).await.unwrap();
        cf.get(&url, &headers).await.unwrap();

        mock.assert_async().await; // 2 network calls, nothing cached
    }

    #[tokio::test]
    async fn test_different_headers_are_separate_cache_entries() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/")
            .match_header("accept", "text/markdown")
            .with_status(200)
            .with_header("cache-control", "max-age=3600")
            .with_body("# Markdown")
            .create_async()
            .await;
        server
            .mock("GET", "/")
            .with_status(200)
            .with_header("cache-control", "max-age=3600")
            .with_body("<html>HTML</html>")
            .create_async()
            .await;

        let cf = make_cf().await;
        let url = Url::parse(&server.url()).unwrap();

        let mut md_headers = HashMap::new();
        md_headers.insert("accept".to_string(), "text/markdown".to_string());

        let resp_md = cf.get(&url, &md_headers).await.unwrap();
        let resp_html = cf.get(&url, &HashMap::new()).await.unwrap();

        assert_eq!(resp_md.body, "# Markdown");
        assert_eq!(resp_html.body, "<html>HTML</html>");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:
```
cargo test -p aget-lib caching_fetcher
```
Expected: compile error — `CachingFetcher` is not defined.

- [ ] **Step 3: Implement `CachingFetcher`**

Add the implementation above the `#[cfg(test)]` block in `lib/src/caching_fetcher.rs`:

```rust
use crate::cache::{compute_max_age_secs, effective_max_age, is_no_store, unix_now, Cache, CacheEntry};
use crate::error::Result;
use crate::fetch::Fetch;
use crate::fetcher::{FetchResponse, Fetcher};
use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use url::Url;

pub struct CachingFetcher {
    inner: Fetcher,
    cache: Cache,
}

impl CachingFetcher {
    pub fn new() -> Result<Self> {
        let path = Cache::default_path()
            .ok_or_else(|| crate::error::AgetError::config("could not determine home directory"))?;
        Ok(Self {
            inner: Fetcher::new()?,
            cache: Cache::open(&path)?,
        })
    }

    #[cfg(test)]
    pub fn with_in_memory_cache() -> Result<Self> {
        Ok(Self {
            inner: Fetcher::new()?,
            cache: Cache::open_in_memory()?,
        })
    }

    fn store_response(&self, url: &str, headers_hash: &str, resp: &FetchResponse) -> Result<()> {
        let cc = resp.cache_control.as_deref();
        if cc.map(is_no_store).unwrap_or(false) {
            return Ok(());
        }
        let max_age_secs = compute_max_age_secs(cc, resp.expires.as_deref());
        self.cache.store(
            url,
            headers_hash,
            &CacheEntry {
                status: resp.status,
                content_type: resp.content_type.clone(),
                body: resp.body.clone(),
                etag: resp.etag.clone(),
                last_modified: resp.last_modified.clone(),
                max_age_secs,
                cached_at: unix_now(),
            },
        )
    }
}

fn hash_headers(headers: &HashMap<String, String>) -> String {
    let mut pairs: Vec<String> = headers
        .iter()
        .map(|(k, v)| format!("{}={}", k.to_lowercase(), v))
        .collect();
    pairs.sort();
    let mut hasher = Sha256::new();
    hasher.update(pairs.join("\n").as_bytes());
    format!("{:x}", hasher.finalize())
}

#[async_trait]
impl Fetch for CachingFetcher {
    async fn get(&self, url: &Url, headers: &HashMap<String, String>) -> Result<FetchResponse> {
        let url_str = url.as_str();
        let headers_hash = hash_headers(headers);

        if let Some(entry) = self.cache.get(url_str, &headers_hash)? {
            let now = unix_now();

            if entry.cached_at + effective_max_age(&entry) > now {
                // Fresh — return cached without network call
                return Ok(FetchResponse {
                    status: entry.status,
                    content_type: entry.content_type,
                    body: entry.body,
                    etag: entry.etag,
                    last_modified: entry.last_modified,
                    cache_control: None,
                    expires: None,
                });
            }

            // Stale — try conditional request
            let mut cond_headers = headers.clone();
            if let Some(etag) = &entry.etag {
                cond_headers.insert("If-None-Match".to_string(), etag.clone());
            }
            if let Some(lm) = &entry.last_modified {
                cond_headers.insert("If-Modified-Since".to_string(), lm.clone());
            }

            let resp = self.inner.get(url, &cond_headers).await?;

            if resp.status == 304 {
                self.cache.refresh_cached_at(url_str, &headers_hash)?;
                return Ok(FetchResponse {
                    status: entry.status,
                    content_type: entry.content_type,
                    body: entry.body,
                    etag: entry.etag,
                    last_modified: entry.last_modified,
                    cache_control: None,
                    expires: None,
                });
            }

            // New 200 response — replace cache entry
            self.store_response(url_str, &headers_hash, &resp)?;
            return Ok(resp);
        }

        // Cache miss — full request
        let resp = self.inner.get(url, headers).await?;
        self.store_response(url_str, &headers_hash, &resp)?;
        Ok(resp)
    }
}
```

- [ ] **Step 4: Ensure `pub mod caching_fetcher;` is in `lib/src/lib.rs`**

Verify `lib/src/lib.rs` contains:
```rust
pub mod cache;
pub mod caching_fetcher;
pub mod config;
pub mod engine;
pub mod error;
pub mod fetch;
pub mod fetcher;
pub mod pipeline;
pub mod quality;

pub use config::Config;
pub use error::{AgetError, Result};
pub use pipeline::{Pipeline, PipelineResult};
```

- [ ] **Step 5: Run tests to verify they pass**

Run:
```
cargo test -p aget-lib caching_fetcher
```
Expected: all 6 tests pass.

- [ ] **Step 6: Commit**

```
git add lib/src/caching_fetcher.rs lib/src/lib.rs
git commit -m "feat: add CachingFetcher with SQLite-backed HTTP cache"
```

---

## Task 6: Update Engine trait and all engine implementations

**Files:**
- Modify: `lib/src/engine/mod.rs`
- Modify: `lib/src/engine/accept_md.rs`
- Modify: `lib/src/engine/dot_md.rs`
- Modify: `lib/src/engine/html_extract.rs`

- [ ] **Step 1: Update Engine trait in `lib/src/engine/mod.rs`**

Replace the file:

```rust
pub mod accept_md;
pub mod dot_md;
pub mod html_extract;
pub mod registry;

use crate::error::Result;
use crate::fetch::Fetch;
use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;

#[async_trait]
pub trait Engine: Send + Sync {
    fn name(&self) -> &'static str;
    async fn fetch(
        &self,
        url: &Url,
        fetcher: &dyn Fetch,
        domain_headers: &HashMap<String, String>,
    ) -> Result<EngineResult>;
}

pub enum EngineResult {
    Success(String),
    Skip(String),
}
```

- [ ] **Step 2: Replace `accept_md.rs` impl block (imports + Engine impl)**

Replace everything in `lib/src/engine/accept_md.rs` above `#[cfg(test)]` with:

```rust
use crate::engine::{Engine, EngineResult};
use crate::error::Result;
use crate::fetch::Fetch;
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
        fetcher: &dyn Fetch,
        domain_headers: &HashMap<String, String>,
    ) -> Result<EngineResult> {
        let mut headers = domain_headers.clone();
        headers.insert(
            "Accept".to_string(),
            "text/markdown, text/plain;q=0.9".to_string(),
        );

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
```

Leave the `#[cfg(test)]` block untouched.

- [ ] **Step 3: Replace `dot_md.rs` impl block**

Replace everything in `lib/src/engine/dot_md.rs` above `#[cfg(test)]` with:

```rust
use crate::engine::{Engine, EngineResult};
use crate::error::Result;
use crate::fetch::Fetch;
use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;

pub struct DotMdEngine;

fn append_md_extension(url: &Url) -> Option<Url> {
    let mut new_url = url.clone();
    let path = url.path().to_string();
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
        fetcher: &dyn Fetch,
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
```

Leave the `#[cfg(test)]` block untouched.

- [ ] **Step 4: Replace `html_extract.rs` impl block**

Replace everything in `lib/src/engine/html_extract.rs` above `#[cfg(test)]` with:

```rust
use crate::engine::{Engine, EngineResult};
use crate::error::{AgetError, Result};
use crate::fetch::Fetch;
use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;

pub struct HtmlExtractEngine;

fn html_to_markdown(html: &str, url: &Url) -> Result<String> {
    let mut readability = dom_smoothie::Readability::new(html, Some(url.as_str()), None)
        .map_err(|e| AgetError::extraction(e.to_string()))?;

    let article = readability
        .parse()
        .map_err(|e| AgetError::extraction(e.to_string()))?;

    htmd::convert(&article.content)
        .map_err(|e| AgetError::extraction(e.to_string()))
        .or_else(|_| Ok(article.text_content.to_string()))
}

#[async_trait]
impl Engine for HtmlExtractEngine {
    fn name(&self) -> &'static str {
        "html_extract"
    }

    async fn fetch(
        &self,
        url: &Url,
        fetcher: &dyn Fetch,
        domain_headers: &HashMap<String, String>,
    ) -> Result<EngineResult> {
        let resp = fetcher.get(url, domain_headers).await?;
        let content = html_to_markdown(&resp.body, url).unwrap_or_else(|_| resp.body.clone());
        Ok(EngineResult::Success(content))
    }
}
```

Leave the `#[cfg(test)]` block untouched.

- [ ] **Step 5: Run all engine tests to verify they pass**

Run:
```
cargo test -p aget-lib engine
```
Expected: all existing engine tests pass (Rust will auto-coerce `&Fetcher` to `&dyn Fetch` at call sites in tests).

- [ ] **Step 6: Commit**

```
git add lib/src/engine/mod.rs lib/src/engine/accept_md.rs lib/src/engine/dot_md.rs lib/src/engine/html_extract.rs
git commit -m "refactor: Engine::fetch takes &dyn Fetch instead of &Fetcher"
```

---

## Task 7: Update Pipeline to use `Box<dyn Fetch>`

**Files:**
- Modify: `lib/src/pipeline.rs`

- [ ] **Step 1: Update `Pipeline` struct and constructor**

Replace the imports and `Pipeline` struct/impl in `lib/src/pipeline.rs`:

```rust
use crate::cache::Cache;
use crate::caching_fetcher::CachingFetcher;
use crate::config::{apply_url_transform, DomainRule};
use crate::engine::{registry, EngineResult};
use crate::error::Result;
use crate::fetch::Fetch;
use crate::fetcher::Fetcher;
use std::collections::HashMap;
use std::sync::LazyLock;
use url::Url;

pub struct PipelineResult {
    pub content: String,
    pub engine_used: String,
    pub quality_passed: bool,
}

pub struct Pipeline {
    fetcher: Box<dyn Fetch>,
}

static EMPTY_HEADERS: LazyLock<HashMap<String, String>> = LazyLock::new(HashMap::new);

impl Pipeline {
    pub fn new(no_cache: bool) -> Result<Self> {
        let fetcher: Box<dyn Fetch> = if no_cache {
            Box::new(Fetcher::new()?)
        } else {
            match CachingFetcher::new() {
                Ok(cf) => Box::new(cf),
                Err(e) => {
                    eprintln!("[aget] warning: could not open cache ({e}), running without cache");
                    Box::new(Fetcher::new()?)
                }
            }
        };
        Ok(Self { fetcher })
    }
}
```

- [ ] **Step 2: Update `Pipeline::run()` to use `self.fetcher`**

In `Pipeline::run()`, replace `self.fetcher.get(...)` calls — they already use `self.fetcher`, but the type is now `Box<dyn Fetch>`. The `run()` method passes `self.fetcher.as_ref()` to `engine.fetch()`. Change this line:

```rust
match engine.fetch(&url, &self.fetcher, domain_headers).await? {
```

to:

```rust
match engine.fetch(&url, self.fetcher.as_ref(), domain_headers).await? {
```

And for the direct mode, replace:

```rust
let resp = self.fetcher.get(&url, domain_headers).await?;
```

This line still works because `Box<dyn Fetch>` implements `Fetch` via deref coercion — no change needed here.

- [ ] **Step 3: Update existing pipeline tests**

In the `#[cfg(test)]` block of `pipeline.rs`, `Pipeline::new()` now takes a `bool`. Update all calls:

```rust
// Before
let pipeline = Pipeline::new().unwrap();

// After
let pipeline = Pipeline::new(true).unwrap();  // no_cache=true keeps tests fast (no DB)
```

Apply this to all three test functions: `test_pipeline_uses_first_quality_engine`, `test_pipeline_direct_mode_skips_engine_chain`, `test_pipeline_best_effort_fallback`.

- [ ] **Step 4: Run pipeline tests to verify they pass**

Run:
```
cargo test -p aget-lib pipeline
```
Expected: all existing pipeline tests pass.

- [ ] **Step 5: Run full test suite**

Run:
```
cargo test -p aget-lib
```
Expected: all tests pass.

- [ ] **Step 6: Commit**

```
git add lib/src/pipeline.rs
git commit -m "refactor: Pipeline holds Box<dyn Fetch>, accepts no_cache flag"
```

---

## Task 8: Add `--no-cache` CLI flag

**Files:**
- Modify: `cli/src/cli.rs`
- Modify: `cli/src/main.rs`

- [ ] **Step 1: Add `--no-cache` flag to `cli/src/cli.rs`**

Replace the file:

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

    /// Disable HTTP response caching
    #[arg(long = "no-cache")]
    pub no_cache: bool,
}
```

- [ ] **Step 2: Pass `no_cache` to `Pipeline::new()` in `cli/src/main.rs`**

Change the line:

```rust
let pipeline = Pipeline::new().context("failed to create pipeline")?;
```

to:

```rust
let pipeline = Pipeline::new(cli.no_cache).context("failed to create pipeline")?;
```

- [ ] **Step 3: Build and verify**

Run:
```
cargo build
```
Expected: clean build.

Run:
```
cargo test
```
Expected: all tests pass across both `aget-lib` and `aget`.

- [ ] **Step 4: Smoke test the CLI flag**

Run:
```
cargo run -- --help
```
Expected: `--no-cache` appears in the options list.

- [ ] **Step 5: Commit**

```
git add cli/src/cli.rs cli/src/main.rs
git commit -m "feat: add --no-cache flag to disable HTTP response caching"
```
