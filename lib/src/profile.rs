/// A documentation-framework profile: how to detect it and where its content lives.
pub struct Profile {
    pub key: &'static str,
    /// Substring matched against the value of <meta name="generator"> (case-insensitive).
    pub generator_pattern: Option<&'static str>,
    /// If any of these substrings appear in the HTML, treat the page as a match.
    pub needles: &'static [&'static str],
    /// CSS selectors (in priority order) used to locate the content root for extraction.
    pub content_selectors: &'static [&'static str],
}

pub static VITEPRESS: Profile = Profile {
    key: "vitepress",
    generator_pattern: Some("vitepress"),
    needles: &["id=\"VPContent\"", "class=\"VPDoc", "class=\"vp-doc"],
    content_selectors: &["#VPContent", ".VPDoc", ".vp-doc"],
};

pub static PROFILES: &[&Profile] = &[&VITEPRESS];

/// Detect which (if any) profile best matches the given HTML. First match wins.
pub fn detect_profile(html: &str) -> Option<&'static Profile> {
    let generator = extract_generator_meta(html);
    for profile in PROFILES {
        if let (Some(pattern), Some(value)) = (profile.generator_pattern, generator.as_deref()) {
            if value.to_lowercase().contains(&pattern.to_lowercase()) {
                return Some(*profile);
            }
        }
        for needle in profile.needles {
            if html.contains(needle) {
                return Some(*profile);
            }
        }
    }
    None
}

fn extract_generator_meta(html: &str) -> Option<String> {
    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(
            r#"(?is)<meta\s+[^>]*name\s*=\s*["']generator["'][^>]*content\s*=\s*["']([^"']+)["']"#,
        )
        .expect("static regex must compile")
    });
    re.captures(html).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
}

/// Try to extract markdown from `html` using `profile`'s content selectors.
/// Returns `None` if no selector matches or the result is empty.
pub fn extract_with_profile(html: &str, profile: &Profile, url: &url::Url) -> Option<String> {
    let _ = (html, profile, url);
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_profile_returns_none_for_empty_html() {
        assert!(detect_profile("").is_none());
    }

    #[test]
    fn test_detect_profile_returns_none_when_no_profile_matches() {
        assert!(detect_profile("<html><body>plain</body></html>").is_none());
    }

    #[test]
    fn test_detect_vitepress_via_needle() {
        let html = r#"<html><body><div id="VPContent">hi</div></body></html>"#;
        let p = detect_profile(html).expect("vitepress should match");
        assert_eq!(p.key, "vitepress");
    }

    #[test]
    fn test_detect_vitepress_via_generator_meta() {
        let html = r#"<html><head><meta name="generator" content="VitePress 1.0.0"></head><body></body></html>"#;
        let p = detect_profile(html).expect("vitepress generator should match");
        assert_eq!(p.key, "vitepress");
    }

    #[test]
    fn test_detect_generator_match_is_case_insensitive() {
        let html = r#"<meta name="generator" content="VITEPRESS">"#;
        assert!(detect_profile(html).is_some());
    }
}
