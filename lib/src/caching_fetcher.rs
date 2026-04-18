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

        // Second call: stale -> conditional GET -> 304
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
