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

pub static PROFILES: &[&Profile] = &[];

/// Detect which (if any) profile best matches the given HTML. First match wins.
pub fn detect_profile(html: &str) -> Option<&'static Profile> {
    let _ = html;
    None
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
}
