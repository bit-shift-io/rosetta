use pulldown_cmark;

/// Trait for formatting messages for different chat protocols
pub trait MessageFormatter {
    /// Format a text message with sender and content
    fn format_text(&self, sender: &str, content: &str, is_own: bool) -> String;

    /// Format a caption for attachments/media
    fn format_caption(&self, sender: &str, content: &str, is_own: bool) -> String;

    /// Convert markdown to HTML (default: no-op)
    fn markdown_to_html(&self, markdown: &str) -> String {
        markdown.to_string()
    }
}

/// Helper to format sender and content with custom prefix/suffix for the sender
/// (e.g., "**" for Discord, "*" for WhatsApp, "**" for Matrix)
pub fn format_sender_content(
    sender: &str,
    content: &str,
    sender_prefix: &str,
    sender_suffix: &str,
) -> String {
    match (sender.is_empty(), content.is_empty()) {
        (true, true) => "".to_string(),
        (true, false) => content.to_string(),
        (false, true) => format!("{}{}{}", sender_prefix, sender, sender_suffix),
        (false, false) => {
            format!("{}{}{}: {}", sender_prefix, sender, sender_suffix, content)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_sender_content_both_present() {
        let result = format_sender_content("Alice", "Hello", "**", "**");
        assert_eq!(result, "**Alice**: Hello");
    }

    #[test]
    fn format_sender_content_empty_sender() {
        let result = format_sender_content("", "Hello", "**", "**");
        assert_eq!(result, "Hello");
    }

    #[test]
    fn format_sender_content_empty_content() {
        let result = format_sender_content("Alice", "", "**", "**");
        assert_eq!(result, "**Alice**");
    }

    #[test]
    fn format_sender_content_both_empty() {
        let result = format_sender_content("", "", "**", "**");
        assert_eq!(result, "");
    }

    #[test]
    fn format_sender_content_whatsapp_style() {
        let result = format_sender_content("Alice", "Hello", "*", "*");
        assert_eq!(result, "*Alice*: Hello");
    }
}
