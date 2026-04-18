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

pub fn is_no_store(cache_control: &str) -> bool {
    cache_control
        .split(',')
        .any(|d| d.trim() == "no-store")
}

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
