pub fn chunk_markdown(content: &str, max_chars: usize) -> Vec<String> {
    if content.len() <= max_chars {
        return vec![content.to_string()];
    }
    todo!("implement boundary-based chunking")
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
}
