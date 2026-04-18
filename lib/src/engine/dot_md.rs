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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fetcher::Fetcher;

    #[test]
    fn test_append_md_extension() {
        let url = Url::parse("https://example.com/docs/page").unwrap();
        let result = append_md_extension(&url).unwrap();
        assert_eq!(result.path(), "/docs/page.md");
    }

    #[test]
    fn test_append_md_extension_already_md() {
        let url = Url::parse("https://example.com/docs/page.md").unwrap();
        assert!(append_md_extension(&url).is_none());
    }

    #[tokio::test]
    async fn test_dot_md_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/page.md")
            .with_status(200)
            .with_header("content-type", "text/plain")
            .with_body("# Page\n\nContent here with **markdown**.")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&format!("{}/page", server.url())).unwrap();
        let result = DotMdEngine
            .fetch(&url, &fetcher, &HashMap::new())
            .await
            .unwrap();

        assert!(matches!(result, EngineResult::Success(_)));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_dot_md_skips_404() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/page.md")
            .with_status(404)
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&format!("{}/page", server.url())).unwrap();
        let result = DotMdEngine
            .fetch(&url, &fetcher, &HashMap::new())
            .await
            .unwrap();

        assert!(matches!(result, EngineResult::Skip(_)));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_dot_md_skips_html_response() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/page.md")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body("<html>not markdown</html>")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&format!("{}/page", server.url())).unwrap();
        let result = DotMdEngine
            .fetch(&url, &fetcher, &HashMap::new())
            .await
            .unwrap();

        assert!(matches!(result, EngineResult::Skip(_)));
        mock.assert_async().await;
    }
}
