use crate::error::Result;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::collections::HashMap;
use std::str::FromStr;
use url::Url;

pub struct Fetcher {
    client: reqwest::Client,
}

pub struct FetchResponse {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: String,
}

impl FetchResponse {
    pub fn is_success(&self) -> bool {
        self.status >= 200 && self.status < 300
    }

    pub fn content_type_is(&self, prefix: &str) -> bool {
        self.content_type
            .as_deref()
            .map(|ct| ct.starts_with(prefix))
            .unwrap_or(false)
    }
}

impl Fetcher {
    pub fn new() -> Result<Self> {
        let client = reqwest::Client::builder()
            .user_agent(concat!("aget/", env!("CARGO_PKG_VERSION")))
            .build()?;
        Ok(Self { client })
    }

    pub async fn get(&self, url: &Url, headers: &HashMap<String, String>) -> Result<FetchResponse> {
        let mut header_map = HeaderMap::new();
        for (k, v) in headers {
            if let (Ok(name), Ok(value)) = (HeaderName::from_str(k), HeaderValue::from_str(v)) {
                header_map.insert(name, value);
            }
        }

        let response = self
            .client
            .get(url.as_str())
            .headers(header_map)
            .send()
            .await?;

        let status = response.status().as_u16();
        let content_type = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.split(';').next().unwrap_or(s).trim().to_string());

        let body = response.text().await?;
        Ok(FetchResponse {
            status,
            content_type,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetcher_get_success() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/page")
            .with_status(200)
            .with_header("content-type", "text/markdown; charset=utf-8")
            .with_body("# Hello")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&format!("{}/page", server.url())).unwrap();
        let resp = fetcher.get(&url, &HashMap::new()).await.unwrap();

        assert!(resp.is_success());
        assert_eq!(resp.content_type.as_deref(), Some("text/markdown"));
        assert_eq!(resp.body, "# Hello");
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_fetcher_passes_custom_headers() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/")
            .match_header("X-Api-Key", "secret")
            .with_status(200)
            .with_body("ok")
            .create_async()
            .await;

        let fetcher = Fetcher::new().unwrap();
        let url = Url::parse(&server.url()).unwrap();
        let mut headers = HashMap::new();
        headers.insert("X-Api-Key".to_string(), "secret".to_string());
        let resp = fetcher.get(&url, &headers).await.unwrap();

        assert!(resp.is_success());
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_content_type_is() {
        let resp = FetchResponse {
            status: 200,
            content_type: Some("text/html".to_string()),
            body: String::new(),
        };
        assert!(resp.content_type_is("text/html"));
        assert!(!resp.content_type_is("text/markdown"));
    }
}
