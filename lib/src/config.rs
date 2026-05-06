use crate::error::{AgetError, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub domains: HashMap<String, DomainRule>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct DomainRule {
    pub url_transform: Option<String>,
    pub engine: Option<String>,
    pub engines: Option<Vec<String>>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub path_pattern: Option<String>,
}

impl Config {
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path).map_err(AgetError::Io)?;
        toml::from_str(&content).map_err(AgetError::TomlParse)
    }

    pub fn load_default() -> Result<Self> {
        match Self::default_path() {
            Some(path) if path.exists() => Self::load(&path),
            _ => Ok(Self::default()),
        }
    }

    pub fn default_path() -> Option<PathBuf> {
        dirs::home_dir().map(|h| h.join(".aget").join("config.toml"))
    }
}

pub fn apply_url_transform(url: &url::Url, template: &str) -> Result<url::Url> {
    let segments: Vec<&str> = url
        .path_segments()
        .map(|segs| segs.filter(|s| !s.is_empty()).collect())
        .unwrap_or_default();

    let mut result = String::new();
    let mut seg_idx = 0;
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '{' {
            let mut _var_name = String::new();
            for c2 in chars.by_ref() {
                if c2 == '}' {
                    break;
                }
                _var_name.push(c2);
            }
            result.push_str(segments.get(seg_idx).copied().unwrap_or(""));
            seg_idx += 1;
        } else {
            result.push(c);
        }
    }

    url::Url::parse(&result).map_err(AgetError::UrlParse)
}

pub fn domain_rule_matches(rule: &DomainRule, url: &url::Url) -> bool {
    let pattern = match &rule.path_pattern {
        Some(p) => p,
        None => return true,
    };
    match regex::Regex::new(pattern) {
        Ok(re) => re.is_match(url.path()),
        Err(_) => true, // fail open on bad regex (logged elsewhere if needed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_config_parses_domain_rule() {
        let toml = r#"
[domains."github.com"]
url_transform = "https://raw.githubusercontent.com/{owner}/{repo}/refs/heads/main/readme.md"
engine = "direct"

[domains."docs.example.com"]
engines = ["accept_md", "dot_md"]

[domains."docs.example.com".headers]
Authorization = "Bearer token123"
"#;
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(toml.as_bytes()).unwrap();

        let config = Config::load(f.path()).unwrap();

        let gh = config.domains.get("github.com").unwrap();
        assert_eq!(
            gh.url_transform.as_deref(),
            Some("https://raw.githubusercontent.com/{owner}/{repo}/refs/heads/main/readme.md")
        );
        assert_eq!(gh.engine.as_deref(), Some("direct"));

        let docs = config.domains.get("docs.example.com").unwrap();
        assert_eq!(docs.engines.as_ref().unwrap(), &["accept_md", "dot_md"]);
        assert_eq!(
            docs.headers.get("Authorization").map(String::as_str),
            Some("Bearer token123")
        );
    }

    #[test]
    fn test_config_empty_file() {
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(b"").unwrap();
        let config = Config::load(f.path()).unwrap();
        assert!(config.domains.is_empty());
    }

    #[test]
    fn test_apply_url_transform_github() {
        let url = url::Url::parse("https://github.com/danhilltech/goyolov5").unwrap();
        let template = "https://raw.githubusercontent.com/{owner}/{repo}/refs/heads/main/readme.md";
        let result = apply_url_transform(&url, template).unwrap();
        assert_eq!(
            result.as_str(),
            "https://raw.githubusercontent.com/danhilltech/goyolov5/refs/heads/main/readme.md"
        );
    }

    #[test]
    fn test_apply_url_transform_no_placeholders() {
        let url = url::Url::parse("https://example.com/page").unwrap();
        let template = "https://other.com/fixed";
        let result = apply_url_transform(&url, template).unwrap();
        assert_eq!(result.as_str(), "https://other.com/fixed");
    }

    #[test]
    fn test_config_parses_path_pattern() {
        let toml = r#"
[domains."github.com"]
url_transform = "https://raw.githubusercontent.com/{owner}/{repo}/HEAD/README.md"
engine = "direct"
path_pattern = "^/[^/]+/[^/]+/?$"
"#;
        let mut f = NamedTempFile::new().unwrap();
        f.write_all(toml.as_bytes()).unwrap();
        let config = Config::load(f.path()).unwrap();
        let gh = config.domains.get("github.com").unwrap();
        assert_eq!(gh.path_pattern.as_deref(), Some("^/[^/]+/[^/]+/?$"));
    }

    #[test]
    fn test_domain_rule_matches_when_no_path_pattern() {
        let rule = DomainRule::default();
        let url = url::Url::parse("https://example.com/anything/here").unwrap();
        assert!(domain_rule_matches(&rule, &url));
    }

    #[test]
    fn test_domain_rule_matches_only_when_pattern_matches() {
        let rule = DomainRule {
            path_pattern: Some(r"^/[^/]+/[^/]+/?$".to_string()),
            ..Default::default()
        };
        let ok = url::Url::parse("https://github.com/wevm/curl.md").unwrap();
        let bad = url::Url::parse("https://github.com/wevm/curl.md/blob/main/README.md").unwrap();
        assert!(domain_rule_matches(&rule, &ok));
        assert!(!domain_rule_matches(&rule, &bad));
    }

    #[test]
    fn test_domain_rule_invalid_regex_does_not_panic() {
        let rule = DomainRule {
            path_pattern: Some("[unclosed".to_string()),
            ..Default::default()
        };
        let url = url::Url::parse("https://example.com/").unwrap();
        // Bad regex should NOT panic and SHOULD fall back to "matches" so user mistakes don't silently break fetches
        assert!(domain_rule_matches(&rule, &url));
    }
}
