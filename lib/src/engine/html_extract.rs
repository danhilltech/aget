use crate::engine::{Engine, EngineResult};
use crate::error::{AgetError, Result};
use crate::fetch::Fetch;
use async_trait::async_trait;
use std::collections::HashMap;
use url::Url;

pub struct HtmlExtractEngine;

fn html_to_markdown(html: &str, url: &Url) -> Result<String> {
    // 1) Try framework-profile-based extraction first.
    if let Some(profile) = crate::profile::detect_profile(html) {
        if let Some(md) = crate::profile::extract_with_profile(html, profile, url) {
            return Ok(md);
        }
    }

    // 2) Fall back to readability + htmd.
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetcher::Fetcher;

    const SIMPLE_HTML: &str = r#"<!DOCTYPE html>
<html>
<head><title>Test Page</title></head>
<body>
  <article>
    <h1>Hello World</h1>
    <p>This is a paragraph with some <strong>bold</strong> text and enough content to be readable.</p>
    <p>Another paragraph to meet minimum length requirements for readability extraction.</p>
  </article>
</body>
</html>"#;

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
        let result = HtmlExtractEngine
            .fetch(&url, &fetcher, &HashMap::new())
            .await
            .unwrap();

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
        let result = HtmlExtractEngine
            .fetch(&url, &fetcher, &HashMap::new())
            .await
            .unwrap();

        assert!(matches!(result, EngineResult::Success(_)));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_html_extract_uses_profile_when_detected() {
        let mut server = mockito::Server::new_async().await;
        let _mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(
                r#"<!doctype html><html><head><meta name="generator" content="VitePress"></head>
            <body>
              <nav>SHOULD_NOT_APPEAR</nav>
              <div id="VPContent">
                <h1>From Profile</h1>
                <p>This came via the VitePress profile path.</p>
              </div>
            </body></html>"#,
            )
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&server.url()).unwrap();
        let result = HtmlExtractEngine
            .fetch(&url, &fetcher, &HashMap::new())
            .await
            .unwrap();

        let content = match result {
            EngineResult::Success(s) => s,
            _ => panic!("expected Success"),
        };
        assert!(content.contains("From Profile"));
        assert!(!content.contains("SHOULD_NOT_APPEAR"));
    }

    #[tokio::test]
    async fn test_html_extract_falls_back_to_readability_when_no_profile_matches() {
        let mut server = mockito::Server::new_async().await;
        // No generator meta, no profile needles — generic article
        let _mock = server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body(SIMPLE_HTML)
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&server.url()).unwrap();
        let result = HtmlExtractEngine
            .fetch(&url, &fetcher, &HashMap::new())
            .await
            .unwrap();

        assert!(matches!(result, EngineResult::Success(_)));
        if let EngineResult::Success(content) = result {
            assert!(content.contains("Hello"), "fallback should still extract content");
        }
    }
}
