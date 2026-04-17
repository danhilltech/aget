use crate::error::Result;
use crate::fetcher::Fetcher;
use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;

use super::EngineResult;

pub struct HtmlExtractEngine;

#[async_trait]
impl super::Engine for HtmlExtractEngine {
    fn name(&self) -> &'static str {
        "html_extract"
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
