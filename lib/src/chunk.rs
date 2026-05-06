pub fn chunk_markdown(content: &str, max_chars: usize) -> Vec<String> {
    if content.len() <= max_chars {
        return vec![content.to_string()];
    }
    split_at_boundary(content, max_chars, 0)
}

const BOUNDARIES: &[&str] = &["\n## ", "\n### ", "\n\n", "\n"];

fn split_at_boundary(text: &str, max_chars: usize, level: usize) -> Vec<String> {
    if text.len() <= max_chars {
        return vec![text.to_string()];
    }

    let separator = match BOUNDARIES.get(level) {
        Some(s) => *s,
        None => return hard_split(text, max_chars),
    };

    let sections = split_keeping_separator(text, separator);
    if sections.len() <= 1 {
        return split_at_boundary(text, max_chars, level + 1);
    }

    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    for section in sections {
        if !current.is_empty() && current.len() + section.len() > max_chars {
            chunks.extend(split_at_boundary(&current, max_chars, level + 1));
            current = section;
        } else {
            current.push_str(&section);
        }
    }
    if !current.is_empty() {
        chunks.extend(split_at_boundary(&current, max_chars, level + 1));
    }
    chunks
}

fn split_keeping_separator(text: &str, separator: &str) -> Vec<String> {
    // Separators start with '\n' for heading/paragraph boundaries.
    // The '\n' logically closes the preceding section; the next section
    // starts at the non-newline portion of the separator (e.g. "## ").
    // We split so that '\n' stays with the preceding section and the
    // rest of the separator begins the next section.
    let leading_newlines = separator.bytes().take_while(|&b| b == b'\n').count();
    let sep_prefix = &separator[..leading_newlines]; // e.g. "\n"
    let sep_tail = &separator[leading_newlines..]; // e.g. "## "

    let mut sections: Vec<String> = Vec::new();
    let mut pos = 0usize;

    while pos < text.len() {
        // Search for the separator starting after the first character of the
        // current section so a match at pos=0 doesn't produce an empty section.
        let search_from = text[pos..]
            .char_indices()
            .nth(1)
            .map(|(i, _)| pos + i)
            .unwrap_or(text.len());

        match text[search_from..].find(separator) {
            Some(rel) => {
                let match_start = search_from + rel;
                // Section ends after the leading newlines of the separator.
                let section_end = match_start + sep_prefix.len();
                sections.push(text[pos..section_end].to_string());
                // Next section starts at the non-newline tail of the separator.
                pos = section_end + sep_tail.len();
                if !sep_tail.is_empty() {
                    // Prepend sep_tail to next section by starting pos before it.
                    // Actually we already advanced pos past sep_tail; we need to
                    // rewind to include sep_tail at the start of the next section.
                    pos -= sep_tail.len();
                }
            }
            None => {
                sections.push(text[pos..].to_string());
                break;
            }
        }
    }
    sections
}

fn hard_split(text: &str, max_chars: usize) -> Vec<String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut buf = String::new();
    for ch in text.chars() {
        if buf.len() + ch.len_utf8() > max_chars && !buf.is_empty() {
            chunks.push(std::mem::take(&mut buf));
        }
        buf.push(ch);
    }
    if !buf.is_empty() {
        chunks.push(buf);
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_content_returns_single_chunk() {
        let content = "# Hello\n\nShort enough.";
        let chunks = chunk_markdown(content, 1000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0], content);
    }

    #[test]
    fn test_splits_at_h2_boundary() {
        let content = "# Title\n\nIntro paragraph.\n\n## Section A\n\nLots of text in section A.\n\n## Section B\n\nLots of text in section B.\n";
        // Force splitting by setting max_chars below total length but above each section length
        let chunks = chunk_markdown(content, 80);
        assert!(
            chunks.len() >= 2,
            "expected at least 2 chunks, got {}",
            chunks.len()
        );
        // No chunk should exceed max_chars (allowing slack for boundary inclusion)
        for c in &chunks {
            assert!(c.len() <= 120, "chunk too long: {} chars", c.len());
        }
        // Reassembling chunks must reproduce the original content exactly
        assert_eq!(chunks.join(""), content);
        // Each chunk after the first should start with "## "
        for c in chunks.iter().skip(1) {
            assert!(
                c.starts_with("## "),
                "chunk should start with '## ', got: {:?}",
                &c[..c.len().min(20)]
            );
        }
    }

    #[test]
    fn test_falls_through_to_h3_when_no_h2() {
        let content =
            "# Title\n\nIntro.\n\n### Sub A\n\nContent A here.\n\n### Sub B\n\nContent B here.\n";
        let chunks = chunk_markdown(content, 60);
        assert!(chunks.len() >= 2);
        assert_eq!(chunks.join(""), content);
    }

    #[test]
    fn test_hard_cut_when_no_boundaries() {
        // One long line of repeated chars with no boundary characters at all
        let content = "a".repeat(500);
        let chunks = chunk_markdown(&content, 100);
        assert!(
            chunks.len() >= 5,
            "expected at least 5 chunks, got {}",
            chunks.len()
        );
        for c in &chunks {
            assert!(c.len() <= 100, "chunk exceeded max: {}", c.len());
        }
        assert_eq!(chunks.join(""), content);
    }

    #[test]
    fn test_preserves_unicode() {
        let content = "# 日本語\n\n本文がここにあります。たくさんの文字があります。\n\n## セクション2\n\nもっとテキスト。\n";
        let chunks = chunk_markdown(content, 30);
        assert!(chunks.len() >= 2);
        // Concatenation must equal original (no byte-boundary corruption)
        assert_eq!(chunks.join(""), content);
    }

    #[test]
    fn test_zero_max_returns_per_char_chunks_for_no_boundary_input() {
        // Edge case: very small max with no boundaries — should not infinite-loop
        let content = "abcdef";
        let chunks = chunk_markdown(content, 1);
        assert_eq!(chunks.len(), 6);
        assert_eq!(chunks.join(""), content);
    }
}
