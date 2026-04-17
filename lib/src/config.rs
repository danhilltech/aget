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
}
