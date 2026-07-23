use crate::bridge::formatter::{MessageFormatter, format_sender_content};

pub struct MatrixFormatter;

impl MatrixFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MatrixFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageFormatter for MatrixFormatter {
    fn format_text(&self, sender: &str, content: &str, _is_own: bool) -> String {
        format_sender_content(sender, content, "**", "**")
    }

    fn format_caption(&self, sender: &str, content: &str, _is_own: bool) -> String {
        match (sender.is_empty(), content.is_empty()) {
            (true, true) => "".to_string(),
            (true, false) => content.to_string(),
            (false, true) => format!("Sent by {}", sender),
            (false, false) => format!("{}: {}", sender, content),
        }
    }

    fn markdown_to_html(&self, markdown: &str) -> String {
        let mut options = pulldown_cmark::Options::empty();
        options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);

        let parser = pulldown_cmark::Parser::new_ext(markdown, options);
        let mut html_body = String::new();
        pulldown_cmark::html::push_html(&mut html_body, parser);
        html_body
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_text_sender_and_content() {
        let formatter = MatrixFormatter::new();
        let result = formatter.format_text("Alice", "Hello", false);
        assert_eq!(result, "**Alice**: Hello");
    }

    #[test]
    fn format_text_empty_sender() {
        let formatter = MatrixFormatter::new();
        let result = formatter.format_text("", "Hello", false);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn format_text_empty_content() {
        let formatter = MatrixFormatter::new();
        let result = formatter.format_text("Alice", "", false);
        assert_eq!(result, "**Alice**");
    }

    #[test]
    fn format_caption_sender_and_content() {
        let formatter = MatrixFormatter::new();
        let result = formatter.format_caption("Alice", "Hello", false);
        assert_eq!(result, "Alice: Hello");
    }

    #[test]
    fn format_caption_empty_both() {
        let formatter = MatrixFormatter::new();
        let result = formatter.format_caption("", "", false);
        assert_eq!(result, "");
    }

    #[test]
    fn format_caption_only_content() {
        let formatter = MatrixFormatter::new();
        let result = formatter.format_caption("", "Just content", false);
        assert_eq!(result, "Just content");
    }

    #[test]
    fn format_caption_only_sender() {
        let formatter = MatrixFormatter::new();
        let result = formatter.format_caption("Alice", "", false);
        assert_eq!(result, "Sent by Alice");
    }

    #[test]
    fn markdown_to_html_basic() {
        let formatter = MatrixFormatter::new();
        let result = formatter.markdown_to_html("**bold**");
        assert!(result.contains("<strong>bold</strong>"));
    }

    #[test]
    fn markdown_to_html_strikethrough() {
        let formatter = MatrixFormatter::new();
        let result = formatter.markdown_to_html("~~strike~~");
        assert!(result.contains("<del>strike</del>"));
    }
}
