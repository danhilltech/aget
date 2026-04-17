const MIN_LENGTH: usize = 100;

const MARKDOWN_MARKERS: &[&str] = &["# ", "## ", "**", "```", "---", "- ", "* ", "["];

pub fn passes_quality(content: &str) -> bool {
    if content.len() < MIN_LENGTH {
        return false;
    }
    MARKDOWN_MARKERS
        .iter()
        .any(|marker| content.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_passes_with_heading() {
        let content = "# Hello World\n\n".to_string()
            + &"Some content that is long enough to pass the minimum length check. ".repeat(3);
        assert!(passes_quality(&content));
    }

    #[test]
    fn test_passes_with_bold() {
        let content = "**bold text** at the start\n\n".to_string()
            + &"Some content that is long enough to pass the minimum length check. ".repeat(3);
        assert!(passes_quality(&content));
    }

    #[test]
    fn test_passes_with_link() {
        let content = "[link text](https://example.com)\n\n".to_string()
            + &"Some content that is long enough to pass the minimum length check. ".repeat(3);
        assert!(passes_quality(&content));
    }

    #[test]
    fn test_fails_too_short() {
        let content = "# Short";
        assert!(!passes_quality(content));
    }

    #[test]
    fn test_fails_no_markers() {
        let content = "a".repeat(200);
        assert!(!passes_quality(&content));
    }

    #[test]
    fn test_fails_empty() {
        assert!(!passes_quality(""));
    }
}
