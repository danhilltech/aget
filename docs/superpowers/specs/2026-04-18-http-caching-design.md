# HTTP Caching Design

**Date:** 2026-04-18
**Status:** Approved

## Problem

When multiple engines are tried in sequence, engines that fetch the same URL make independent HTTP requests. For example, when a server returns HTML (ignoring the `Accept: text/markdown` header), both `accept_md` and `html_extract` hit the same URL. Additionally, repeated `aget` invocations for the same URL always hit the network with no cross-run caching.

## Goals

1. Eliminate redundant within-run HTTP requests via a unified SQLite cache.
2. Support HTTP cache semantics across runs (ETag, Last-Modified, Cache-Control, Expires).
3. Keep engines unaware of caching — only the fetch interface changes.
4. Cache is on by default; opt-out via `--no-cache`.

## Architecture

Three new units, two changed units:

```
lib/src/
├── fetch.rs              NEW  — Fetch trait
├── fetcher.rs            MOD  — implements Fetch, FetchResponse gains caching headers
├── cache.rs              NEW  — Cache struct, SQLite schema, freshness + conditional logic
├── caching_fetcher.rs    NEW  — CachingFetcher wraps Fetcher + Cache, implements Fetch
├── engine/mod.rs         MOD  — Engine::fetch() takes &dyn Fetch instead of &Fetcher
└── pipeline.rs           MOD  — holds Box<dyn Fetch>, constructs based on no_cache flag
```

`Pipeline` constructs either a `CachingFetcher` (default) or bare `Fetcher` (`--no-cache`). Engines call `fetcher.get()` as before and are unaware of caching.

## Fetch Trait

```rust
#[async_trait]
pub trait Fetch: Send + Sync {
    async fn get(
        &self,
        url: &Url,
        headers: &HashMap<String, String>,
    ) -> Result<FetchResponse>;
}
```

`Fetcher` implements `Fetch` (no logic change, just trait impl added).

## Data Model

### FetchResponse

```rust
pub struct FetchResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: String,
    // caching headers — populated from every network response
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub cache_control: Option<String>,
    pub expires: Option<String>,
}
```

### SQLite Schema

Location: `~/.aget/cache.db`

```sql
CREATE TABLE IF NOT EXISTS entries (
    url                  TEXT NOT NULL,
    request_headers_hash TEXT NOT NULL,
    status               INTEGER NOT NULL,
    content_type         TEXT,
    body                 TEXT NOT NULL,
    etag                 TEXT,
    last_modified        TEXT,
    max_age_secs         INTEGER,        -- NULL means use default TTL
    cached_at            INTEGER NOT NULL,
    PRIMARY KEY (url, request_headers_hash)
);
```

**Cache key:** `(url, sha256(sorted "key=value" request header pairs))`

This ensures engines with different `Accept` headers (e.g. `accept_md` vs `html_extract`) produce separate cache entries, correctly handling servers that implement content negotiation.

## Cache Logic

`CachingFetcher::get()` flow:

```
1. Compute cache key = (url, sha256(sorted headers))
2. Query DB
   ├─ No entry        → full GET → store → return
   ├─ Fresh entry     → return cached body (no network)
   └─ Stale entry
       ├─ Has etag          → add If-None-Match header
       ├─ Has last_modified → add If-Modified-Since header
       └─ Conditional GET
           ├─ 304 → update cached_at → return cached body
           └─ 200 → replace entry   → return new body
```

### Freshness Rules (priority order)

| Condition | Behaviour |
|---|---|
| `Cache-Control: no-store` | Skip cache entirely — don't read or write |
| `Cache-Control: no-cache` | Always revalidate — store with `max_age_secs = 0` |
| `Cache-Control: max-age=N` | Store with `max_age_secs = N` |
| `Expires` header present | Convert to `max_age_secs` relative to response time |
| No caching headers | Store with `max_age_secs = NULL` → use default TTL |

**Default TTL:** 3600 seconds (constant in `cache.rs`).

## Engine Trait Change

```rust
// Before
async fn fetch(&self, url: &Url, fetcher: &Fetcher, domain_headers: &HashMap<String, String>) -> Result<EngineResult>;

// After
async fn fetch(&self, url: &Url, fetcher: &dyn Fetch, domain_headers: &HashMap<String, String>) -> Result<EngineResult>;
```

All engine implementations change only this signature line. Bodies are untouched.

## Pipeline

If `CachingFetcher::new()` fails to open the cache DB (e.g. permissions error), it logs a warning to stderr and falls back to a bare `Fetcher`. The command does not fail.

## CLI

One new flag added to `cli.rs`:

```
--no-cache    Disable HTTP response caching
```

`main.rs` passes `cli.no_cache` to `Pipeline::new()`.

## Dependencies

Add to `lib/Cargo.toml`:
- `rusqlite` with `bundled` feature (for SQLite)
- `sha2` (for SHA-256 cache key hashing)

## Testing

- `fetch.rs`: no tests (trait definition only)
- `fetcher.rs`: existing tests unchanged; add tests for new caching header fields
- `cache.rs`: unit tests for freshness logic, conditional request header generation, schema creation, store/retrieve round-trip
- `caching_fetcher.rs`: integration tests using `mockito` covering: cache miss → store, cache hit (fresh), stale + 304, stale + 200, `no-store` bypass
- `engine/*.rs`: existing tests unchanged (engines take `&dyn Fetch`; mockito server already satisfies this)
- `pipeline.rs`: existing tests updated to use `Box<dyn Fetch>`
