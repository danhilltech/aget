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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_accept_md_success_markdown() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .match_header(
                "Accept",
                mockito::Matcher::Regex("text/markdown".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "text/markdown")
            .with_body("# Hello\n\nWorld content here with **bold**.")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&server.url()).unwrap();
        let result = AcceptMdEngine
            .fetch(&url, &fetcher, &HashMap::new())
            .await
            .unwrap();

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
        let result = AcceptMdEngine
            .fetch(&url, &fetcher, &HashMap::new())
            .await
            .unwrap();

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
        let result = AcceptMdEngine
            .fetch(&url, &fetcher, &HashMap::new())
            .await
            .unwrap();

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
        let result = AcceptMdEngine
            .fetch(&url, &fetcher, &domain_headers)
            .await
            .unwrap();

        assert!(matches!(result, EngineResult::Success(_)));
        mock.assert_async().await;
    }
}
