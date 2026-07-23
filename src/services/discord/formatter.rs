use crate::bridge::formatter::{MessageFormatter, format_sender_content};

pub struct DiscordFormatter;

impl DiscordFormatter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DiscordFormatter {
    fn default() -> Self {
        Self::new()
    }
}

impl MessageFormatter for DiscordFormatter {
    fn format_text(&self, sender: &str, content: &str, _is_own: bool) -> String {
        format_sender_content(sender, content, "**", "**")
    }

    fn format_caption(&self, sender: &str, content: &str, _is_own: bool) -> String {
        match (sender.is_empty(), content.is_empty()) {
            (true, true) => "".to_string(),
            (true, false) => content.to_string(),
            (false, true) => format!("**{}**", sender),
            (false, false) => format!("**{}**: {}", sender, content),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_text_sender_and_content() {
        let formatter = DiscordFormatter::new();
        let result = formatter.format_text("Alice", "Hello", false);
        assert_eq!(result, "**Alice**: Hello");
    }

    #[test]
    fn format_text_empty_sender() {
        let formatter = DiscordFormatter::new();
        let result = formatter.format_text("", "Hello", false);
        assert_eq!(result, "Hello");
    }

    #[test]
    fn format_text_empty_content() {
        let formatter = DiscordFormatter::new();
        let result = formatter.format_text("Alice", "", false);
        assert_eq!(result, "**Alice**");
    }

    #[test]
    fn format_caption_sender_and_content() {
        let formatter = DiscordFormatter::new();
        let result = formatter.format_caption("Alice", "Hello", false);
        assert_eq!(result, "**Alice**: Hello");
    }

    #[test]
    fn format_caption_empty_both() {
        let formatter = DiscordFormatter::new();
        let result = formatter.format_caption("", "", false);
        assert_eq!(result, "");
    }
}
