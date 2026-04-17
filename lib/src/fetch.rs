use crate::error::Result;
use crate::fetcher::FetchResponse;
use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;

#[async_trait]
pub trait Fetch: Send + Sync {
    async fn get(&self, url: &Url, headers: &HashMap<String, String>) -> Result<FetchResponse>;
}
