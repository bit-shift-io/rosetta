use crate::config::ChannelConfig;
use crate::services::ServiceMessage;

pub struct MediaHandler;

impl MediaHandler {
    pub fn new() -> Self {
        Self
    }

    /// Process a message according to the target channel's media policy.
    /// If enable_media is false, strips attachments from the message.
    /// If display_names is false, clears the sender name.
    pub fn process(&self, msg: &mut ServiceMessage, target_config: &ChannelConfig) {
        if !target_config.enable_media {
            msg.attachments.clear();
        }
        if !target_config.display_names {
            msg.sender.clear();
        }
    }
}

impl Default for MediaHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ChannelConfig;
    use crate::services::{Attachment, ServiceMessage};
    use std::collections::HashMap;

    fn make_message() -> ServiceMessage {
        ServiceMessage {
            sender: "Alice".to_string(),
            sender_id: "@alice:matrix.org".to_string(),
            content: "Hello!".to_string(),
            attachments: vec![Attachment {
                filename: "image.png".to_string(),
                mime_type: "image/png".to_string(),
                data: vec![1, 2, 3],
            }],
            source_service: "matrix".to_string(),
            source_channel: "room1".to_string(),
            source_id: "msg1".to_string(),
            is_own: false,
        }
    }

    fn make_channel_config(enable_media: bool, display_names: bool) -> ChannelConfig {
        ChannelConfig {
            service: "discord".to_string(),
            channel: "channel1".to_string(),
            room_name: None,
            display_names,
            enable_media,
            bridge_own_messages: false,
            aliases: HashMap::new(),
        }
    }

    #[test]
    fn process_keeps_attachments_when_enable_media_true() {
        let mut msg = make_message();
        let config = make_channel_config(true, true);
        let handler = MediaHandler::new();

        handler.process(&mut msg, &config);

        assert_eq!(msg.attachments.len(), 1);
        assert_eq!(msg.attachments[0].filename, "image.png");
    }

    #[test]
    fn process_strips_attachments_when_enable_media_false() {
        let mut msg = make_message();
        let config = make_channel_config(false, true);
        let handler = MediaHandler::new();

        handler.process(&mut msg, &config);

        assert!(msg.attachments.is_empty());
    }

    #[test]
    fn process_keeps_sender_when_display_names_true() {
        let mut msg = make_message();
        let config = make_channel_config(true, true);
        let handler = MediaHandler::new();

        handler.process(&mut msg, &config);

        assert_eq!(msg.sender, "Alice");
    }

    #[test]
    fn process_clears_sender_when_display_names_false() {
        let mut msg = make_message();
        let config = make_channel_config(true, false);
        let handler = MediaHandler::new();

        handler.process(&mut msg, &config);

        assert!(msg.sender.is_empty());
    }

    #[test]
    fn process_both_media_and_names_disabled() {
        let mut msg = make_message();
        let config = make_channel_config(false, false);
        let handler = MediaHandler::new();

        handler.process(&mut msg, &config);

        assert!(msg.attachments.is_empty());
        assert!(msg.sender.is_empty());
    }
}
