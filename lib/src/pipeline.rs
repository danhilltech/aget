use crate::caching_fetcher::CachingFetcher;
use crate::config::{apply_url_transform, DomainRule};
use crate::engine::{registry, EngineResult};
use crate::error::Result;
use crate::fetch::Fetch;
use crate::fetcher::Fetcher;
use crate::quality::passes_quality;
use std::collections::HashMap;
use std::sync::LazyLock;
use url::Url;

pub struct PipelineResult {
    pub content: String,
    pub engine_used: String,
    pub quality_passed: bool,
}

pub struct Pipeline {
    fetcher: Box<dyn Fetch>,
}

static EMPTY_HEADERS: LazyLock<HashMap<String, String>> = LazyLock::new(HashMap::new);

impl Pipeline {
    pub fn new(no_cache: bool) -> Result<Self> {
        let fetcher: Box<dyn Fetch> = if no_cache {
            Box::new(Fetcher::new()?)
        } else {
            match CachingFetcher::new() {
                Ok(cf) => Box::new(cf),
                Err(e) => {
                    eprintln!("[aget] warning: could not open cache ({e}), running without cache");
                    Box::new(Fetcher::new()?)
                }
            }
        };
        Ok(Self { fetcher })
    }

    pub async fn run(
        &self,
        raw_url: &Url,
        rule: Option<&DomainRule>,
        verbose: bool,
    ) -> Result<PipelineResult> {
        // Apply URL transform if configured
        let url = match rule.and_then(|r| r.url_transform.as_ref()) {
            Some(template) => apply_url_transform(raw_url, template)?,
            None => raw_url.clone(),
        };

        let domain_headers: &HashMap<String, String> =
            rule.map(|r| &r.headers).unwrap_or(&EMPTY_HEADERS);

        // "direct" mode — skip engine chain, fetch transformed URL as-is
        if rule.and_then(|r| r.engine.as_deref()) == Some("direct") {
            if verbose {
                eprintln!("[aget] direct fetch: {}", url);
            }
            let resp = self.fetcher.get(&url, domain_headers).await?;
            return Ok(PipelineResult {
                content: resp.body,
                engine_used: "direct".to_string(),
                quality_passed: true,
            });
        }

        // Run engine chain
        let engines = registry::build_chain(rule);
        let mut best_effort: Option<(String, String)> = None;

        for engine in &engines {
            if verbose {
                eprintln!("[aget] trying engine: {}", engine.name());
            }

            match engine
                .fetch(&url, self.fetcher.as_ref(), domain_headers)
                .await?
            {
                EngineResult::Skip(reason) => {
                    if verbose {
                        eprintln!("[aget] {}: skipped ({})", engine.name(), reason);
                    }
                }
                EngineResult::Success(content) => {
                    if passes_quality(&content) {
                        if verbose {
                            eprintln!(
                                "[aget] {}: success (quality check passed, {} chars)",
                                engine.name(),
                                content.len()
                            );
                        }
                        return Ok(PipelineResult {
                            content,
                            engine_used: engine.name().to_string(),
                            quality_passed: true,
                        });
                    }
                    if verbose {
                        eprintln!(
                            "[aget] {}: quality check failed ({} chars), keeping as fallback",
                            engine.name(),
                            content.len()
                        );
                    }
                    best_effort = Some((engine.name().to_string(), content));
                }
            }
        }

        // Best-effort fallback
        let (engine_used, content) =
            best_effort.unwrap_or_else(|| ("none".to_string(), String::new()));

        Ok(PipelineResult {
            content,
            engine_used,
            quality_passed: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::DomainRule;

    const GOOD_MD: &str = "# Hello World\n\nThis is some content that is long enough to pass the quality check and contains markdown markers like **bold** text.\n\nAnother paragraph to ensure we are above 100 characters.";
    const BAD_MD: &str = "nothing here";

    #[tokio::test]
    async fn test_pipeline_uses_first_quality_engine() {
        let mut server = mockito::Server::new_async().await;
        server
            .mock("GET", "/")
            .match_header(
                "Accept",
                mockito::Matcher::Regex("text/markdown".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "text/markdown")
            .with_body(GOOD_MD)
            .create_async()
            .await;

        let pipeline = Pipeline::new(true).unwrap(); // no_cache=true avoids DB in tests
        let url = Url::parse(&server.url()).unwrap();
        let result = pipeline.run(&url, None, false).await.unwrap();

        assert_eq!(result.engine_used, "accept_md");
        assert!(result.quality_passed);
    }

    #[tokio::test]
    async fn test_pipeline_direct_mode_skips_engine_chain() {
        let mut server = mockito::Server::new_async().await;
        let mock = server
            .mock("GET", "/readme.md")
            .with_status(200)
            .with_body("# Direct readme content")
            .create_async()
            .await;

        let base = server.url();
        let pipeline = Pipeline::new(true).unwrap(); // no_cache=true avoids DB in tests
        let url = Url::parse(&format!("{}/original", base)).unwrap();
        let rule = DomainRule {
            url_transform: Some(format!("{}/readme.md", base)),
            engine: Some("direct".to_string()),
            ..Default::default()
        };
        let result = pipeline.run(&url, Some(&rule), false).await.unwrap();

        assert_eq!(result.engine_used, "direct");
        assert!(result.content.contains("Direct readme content"));
        mock.assert_async().await;
    }

    #[tokio::test]
    async fn test_pipeline_best_effort_fallback() {
        let mut server = mockito::Server::new_async().await;
        // accept_md: returns short content that fails quality
        server
            .mock("GET", "/")
            .match_header(
                "Accept",
                mockito::Matcher::Regex("text/markdown".to_string()),
            )
            .with_status(200)
            .with_header("content-type", "text/markdown")
            .with_body(BAD_MD)
            .create_async()
            .await;
        // dot_md: 404
        server
            .mock("GET", "/.md")
            .with_status(404)
            .create_async()
            .await;
        // html_extract fetch: return something
        server
            .mock("GET", "/")
            .with_status(200)
            .with_header("content-type", "text/html")
            .with_body("<html><body><p>some content</p></body></html>")
            .create_async()
            .await;

        let pipeline = Pipeline::new(true).unwrap(); // no_cache=true avoids DB in tests
        let url = Url::parse(&server.url()).unwrap();
        let result = pipeline.run(&url, None, false).await.unwrap();

        // Should get some content from best-effort
        assert!(!result.content.is_empty());
    }
}
