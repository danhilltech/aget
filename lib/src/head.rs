use serde::Serialize;
use crate::config::DomainRule;
use crate::error::Result;
use crate::pipeline::Pipeline;
use url::Url;

#[derive(Debug, Serialize)]
pub struct HeadResult {
    pub url: String,
    pub engine_used: String,
    pub size_bytes: usize,
    pub size_kb: f64,
    pub token_count: usize,
    pub title: Option<String>,
    pub description: Option<String>,
}

impl HeadResult {
    pub fn to_plain_text(&self) -> String {
        let title = self.title.as_deref().unwrap_or("-");
        let description = self.description.as_deref().unwrap_or("-");
        format!(
            "URL:         {}\nEngine:      {}\nSize:        {:.1} KB ({} bytes)\nTokens:      {}\nTitle:       {}\nDescription: {}",
            self.url,
            self.engine_used,
            self.size_kb,
            self.size_bytes,
            self.token_count,
            title,
            description,
        )
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_string())
    }
}

pub fn extract_title(content: &str) -> Option<String> {
    for line in content.lines() {
        if let Some(stripped) = line.strip_prefix("# ") {
            return Some(stripped.trim().to_string());
        }
    }
    None
}

pub fn extract_description(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if trimmed.chars().count() <= 200 {
            return Some(trimmed.to_string());
        }
        let truncated: String = trimmed.chars().take(200).collect();
        return Some(format!("{}…", truncated));
    }
    None
}

pub fn compute_size_kb(size_bytes: usize) -> f64 {
    (size_bytes as f64 / 1024.0 * 10.0).round() / 10.0
}

pub fn count_tokens(content: &str) -> usize {
    tiktoken_rs::cl100k_base()
        .map(|bpe| bpe.encode_with_special_tokens(content).len())
        .unwrap_or_else(|_| content.len() / 4) // ~4 bytes per token heuristic
}

pub async fn head(url: &Url, pipeline: &Pipeline, rule: Option<&DomainRule>) -> Result<HeadResult> {
    let result = pipeline.run(url, rule, false).await?;
    let content = &result.content;
    let size_bytes = content.len();
    Ok(HeadResult {
        url: url.to_string(),
        engine_used: result.engine_used,
        size_bytes,
        size_kb: compute_size_kb(size_bytes),
        token_count: count_tokens(content),
        title: extract_title(content),
        description: extract_description(content),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_title_from_h1() {
        let content = "# My Title\n\nSome paragraph.";
        assert_eq!(extract_title(content), Some("My Title".to_string()));
    }

    #[test]
    fn test_title_none_when_no_h1() {
        let content = "Some paragraph with no heading.";
        assert_eq!(extract_title(content), None);
    }

    #[test]
    fn test_title_ignores_h2() {
        // "## Sub" does NOT start with "# " (second char is '#', not ' ')
        let content = "## Subtitle\n\nSome text.";
        assert_eq!(extract_title(content), None);
    }

    #[test]
    fn test_description_first_paragraph() {
        let content = "# Title\n\nFirst paragraph here.";
        assert_eq!(
            extract_description(content),
            Some("First paragraph here.".to_string())
        );
    }

    #[test]
    fn test_description_skips_headings() {
        let content = "# Title\n\n## Subtitle\n\nActual paragraph.";
        assert_eq!(
            extract_description(content),
            Some("Actual paragraph.".to_string())
        );
    }

    #[test]
    fn test_description_truncates_at_200() {
        let long_line = "a".repeat(250);
        let content = format!("# Title\n\n{}", long_line);
        let desc = extract_description(&content).unwrap();
        assert_eq!(desc.chars().count(), 201); // 200 chars + '…'
        assert!(desc.ends_with('…'));
    }

    #[test]
    fn test_description_no_content() {
        assert_eq!(extract_description(""), None);
        assert_eq!(extract_description("# Only a heading"), None);
    }

    #[test]
    fn test_size_kb_rounding() {
        assert_eq!(compute_size_kb(1024), 1.0);
        assert_eq!(compute_size_kb(1536), 1.5);
        assert_eq!(compute_size_kb(0), 0.0);
    }

    #[test]
    fn test_count_tokens_empty() {
        assert_eq!(count_tokens(""), 0);
    }

    #[test]
    fn test_count_tokens_nonzero_for_content() {
        assert!(count_tokens("Hello, world!") > 0);
    }

    #[test]
    fn test_count_tokens_longer_produces_more() {
        let short = count_tokens("hello");
        let long = count_tokens(
            "hello world this is a much longer piece of text with many words and sentences",
        );
        assert!(long > short);
    }

    #[test]
    fn test_to_plain_text_contains_all_fields() {
        let result = HeadResult {
            url: "https://example.com".to_string(),
            engine_used: "accept_md".to_string(),
            size_bytes: 1024,
            size_kb: 1.0,
            token_count: 200,
            title: Some("My Title".to_string()),
            description: Some("My description.".to_string()),
        };
        let text = result.to_plain_text();
        assert!(text.contains("URL:"));
        assert!(text.contains("https://example.com"));
        assert!(text.contains("Engine:"));
        assert!(text.contains("accept_md"));
        assert!(text.contains("Size:"));
        assert!(text.contains("1.0 KB"));
        assert!(text.contains("1024"));
        assert!(text.contains("Tokens:"));
        assert!(text.contains("200"));
        assert!(text.contains("Title:"));
        assert!(text.contains("My Title"));
        assert!(text.contains("Description:"));
        assert!(text.contains("My description."));
    }

    #[test]
    fn test_to_plain_text_none_fields_show_dash() {
        let result = HeadResult {
            url: "https://example.com".to_string(),
            engine_used: "none".to_string(),
            size_bytes: 0,
            size_kb: 0.0,
            token_count: 0,
            title: None,
            description: None,
        };
        let text = result.to_plain_text();
        // Title and Description lines should show "-"
        assert!(text.contains("Title:       -"));
        assert!(text.contains("Description: -"));
    }

    #[test]
    fn test_to_json_is_valid_json_with_required_keys() {
        let result = HeadResult {
            url: "https://example.com".to_string(),
            engine_used: "accept_md".to_string(),
            size_bytes: 1024,
            size_kb: 1.0,
            token_count: 200,
            title: Some("My Title".to_string()),
            description: None,
        };
        let json = result.to_json();
        assert!(json.contains("\"url\""));
        assert!(json.contains("\"engine_used\""));
        assert!(json.contains("\"size_bytes\""));
        assert!(json.contains("\"size_kb\""));
        assert!(json.contains("\"token_count\""));
        assert!(json.contains("\"title\""));
        assert!(json.contains("\"description\""));
        assert!(json.contains("null")); // description is None → null
    }

    #[tokio::test]
    async fn test_head_returns_metadata_from_pipeline() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/markdown")
            .with_body("# My Title\n\nFirst paragraph of content here with enough text to pass quality checks. This is a longer description that ensures we meet the minimum length requirements for content to be considered valid by the quality module.")
            .create_async()
            .await;

        let pipeline = crate::pipeline::Pipeline::new(true).unwrap();
        let url = url::Url::parse(&server.url()).unwrap();
        let result = head(&url, &pipeline, None).await.unwrap();

        assert_eq!(result.title.as_deref(), Some("My Title"));
        assert_eq!(
            result.description.as_deref(),
            Some("First paragraph of content here with enough text to pass quality checks. This is a longer description that ensures we meet the minimum length requirements for content to be considered valid by the qua…")
        );
        assert!(result.size_bytes > 0);
        assert!(result.token_count > 0);
        assert_eq!(result.url, url.to_string());
        assert_eq!(result.engine_used, "accept_md");
    }
}
