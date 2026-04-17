use crate::error::Result;
use crate::fetcher::Fetcher;
use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;

use super::EngineResult;

pub struct AcceptMdEngine;

#[async_trait]
impl super::Engine for AcceptMdEngine {
    fn name(&self) -> &'static str {
        "accept_md"
    }

    async fn fetch(
        &self,
        _url: &Url,
        _fetcher: &Fetcher,
        _domain_headers: &HashMap<String, String>,
    ) -> Result<EngineResult> {
        unimplemented!()
    }
}
