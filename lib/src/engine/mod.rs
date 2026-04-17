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
