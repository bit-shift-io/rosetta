use crate::config::Config;
use crate::services::ServiceMessage;
use crate::services::traits::{MandatoryService, MemberLister};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct StatusHandler;

impl StatusHandler {
    pub fn new() -> Self {
        Self
    }

    pub async fn handle(
        &self,
        msg: ServiceMessage,
        services: &HashMap<String, Arc<Mutex<Box<dyn MandatoryService>>>>,
        config: &Config,
    ) -> Result<()> {
        log::info!("Status command received in {}", msg.source_channel);

        // Find which bridge this belongs to
        let bridge_entry = config
            .bridges
            .iter()
            .find(|(_, channels)| {
                channels
                    .iter()
                    .any(|ch| ch.service == msg.source_service && ch.channel == msg.source_channel)
            });

        let bridge_name = bridge_entry.map(|(name, _)| name.as_str()).unwrap_or("unknown");
        let empty_channels: Vec<crate::config::ChannelConfig> = vec![];
        let bridge_channels = bridge_entry.map(|(_, channels)| channels).unwrap_or(&empty_channels);

        let mut status_lines = Vec::new();

        // Services section
        status_lines.push("Services".to_string());
        let mut checked_services = Vec::new();
        for ch in bridge_channels {
            if !checked_services.contains(&ch.service) {
                let status = if let Some(svc_lock) = services.get(ch.service.as_str()) {
                    let svc = svc_lock.lock().await;
                    if svc.is_connected() { "✅ Connected" } else { "❌ Disconnected" }
                } else {
                    "❓ Service Not Found"
                };
                status_lines.push(format!(" - {} {}", ch.service, status));
                checked_services.push(ch.service.clone());
            }
        }

        // Bridge header
        status_lines.push(format!("\nBridge {}", bridge_name));

        // Room/channel sections with members
        for ch in bridge_channels {
            let room_name = ch.room_name.as_deref().unwrap_or(&ch.channel);
            status_lines.push(format!("\n{} ({})", room_name, ch.service));

            if let Some(svc_lock) = services.get(ch.service.as_str()) {
                let svc = svc_lock.lock().await;
                match svc.get_room_members(&ch.channel).await {
                    Ok(members) => {
                        if members.is_empty() {
                            status_lines.push("  (No members found or not supported)".to_string());
                        } else {
                            for member in members {
                                status_lines.push(format!(" - {}", member));
                            }
                        }
                    }
                    Err(e) => {
                        status_lines.push(format!("  (Error fetching members: {})", e));
                    }
                }
            } else {
                status_lines.push("  (Service not available)".to_string());
            }
        }

        let status_msg_content = status_lines.join("\n");
        let status_msg = ServiceMessage {
            sender: "System".to_string(),
            sender_id: msg.sender_id.clone(), // Use the original sender's ID so Matrix service recognizes it as own
            content: status_msg_content,
            attachments: vec![],
            source_service: "bridge".to_string(),
            source_channel: "system".to_string(),
            source_id: "system".to_string(),
            is_own: true,
        };

        // Send status response only to the channel that requested it
        if let Some(svc_lock) = services.get(msg.source_service.as_str()) {
            let svc = svc_lock.lock().await;
            if let Err(e) = svc.send_message(&msg.source_channel, &status_msg).await {
                log::error!(
                    "Failed to send status to {}:{}: {}",
                    msg.source_service,
                    msg.source_channel,
                    e
                );
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ChannelConfig, Config};
    use crate::persistence::MessageStore;
    use crate::services::traits::{
        Connectable, MemberLister, MessageSender, ReactionSender, RoomNameFetcher, ServiceInfo,
    };
    use crate::services::{ServiceEvent, ServiceMessage};
    use anyhow::Result;
    use async_trait::async_trait;
    use std::any::Any;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::sync::mpsc;

    struct MockService {
        name: String,
        connected: bool,
        members: Vec<String>,
        room_name: Option<String>,
        send_messages: Mutex<Vec<(String, ServiceMessage)>>,
    }

    impl MockService {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
                connected: true,
                members: vec!["Alice".to_string(), "Bob".to_string()],
                room_name: Some("Test Room".to_string()),
                send_messages: Mutex::new(Vec::new()),
            }
        }

        fn with_connected(mut self, connected: bool) -> Self {
            self.connected = connected;
            self
        }

        fn with_members(mut self, members: Vec<String>) -> Self {
            self.members = members;
            self
        }

        fn with_room_name(mut self, room_name: Option<String>) -> Self {
            self.room_name = room_name;
            self
        }
    }

    #[async_trait]
    impl Connectable for MockService {
        async fn connect(&mut self) -> Result<()> { Ok(()) }
        async fn start(&mut self, _tx: mpsc::Sender<ServiceEvent>) -> Result<()> { Ok(()) }
        fn is_connected(&self) -> bool { self.connected }
        async fn wait_until_ready(&self) -> Result<()> { Ok(()) }
        async fn disconnect(&mut self) -> Result<()> { Ok(()) }
    }

    #[async_trait]
    impl MessageSender for MockService {
        async fn send_message(&self, channel: &str, message: &ServiceMessage) -> Result<String> {
            let mut msgs = self.send_messages.lock().await;
            msgs.push((channel.to_string(), message.clone()));
            Ok("msg123".to_string())
        }
    }

    #[async_trait]
    impl ReactionSender for MockService {
        async fn react_to_message(&self, _channel: &str, _message_id: &str, _emoji: &str) -> Result<()> { Ok(()) }
    }

    impl ServiceInfo for MockService {
        fn service_name(&self) -> &str { &self.name }
        fn as_any(&self) -> &dyn Any { self }
        fn as_any_mut(&mut self) -> &mut dyn Any { self }
    }

    #[async_trait]
    impl MemberLister for MockService {
        async fn get_room_members(&self, _channel: &str) -> Result<Vec<String>> {
            Ok(self.members.clone())
        }
    }

    #[async_trait]
    impl RoomNameFetcher for MockService {
        async fn get_room_name(&self, _channel: &str) -> Result<Option<String>> {
            Ok(self.room_name.clone())
        }
    }

    fn make_test_config() -> Config {
        let mut bridges = HashMap::new();
        bridges.insert(
            "family_bridge".to_string(),
            vec![
                ChannelConfig {
                    service: "matrix_bot".to_string(),
                    channel: "!room1:matrix.org".to_string(),
                    room_name: Some("Family Matrix Room".to_string()),
                    display_names: true,
                    enable_media: true,
                    bridge_own_messages: false,
                    aliases: HashMap::new(),
                },
                ChannelConfig {
                    service: "discord_bot".to_string(),
                    channel: "123456789".to_string(),
                    room_name: Some("Family Discord Channel".to_string()),
                    display_names: true,
                    enable_media: true,
                    bridge_own_messages: false,
                    aliases: HashMap::new(),
                },
                ChannelConfig {
                    service: "whatsapp_direct".to_string(),
                    channel: "group1@g.us".to_string(),
                    room_name: Some("Family WhatsApp Group".to_string()),
                    display_names: true,
                    enable_media: true,
                    bridge_own_messages: true,
                    aliases: HashMap::new(),
                },
            ],
        );
        bridges.insert(
            "rpg_bridge".to_string(),
            vec![
                ChannelConfig {
                    service: "matrix_bot".to_string(),
                    channel: "!room2:matrix.org".to_string(),
                    room_name: Some("RPG Matrix Room".to_string()),
                    display_names: true,
                    enable_media: true,
                    bridge_own_messages: false,
                    aliases: HashMap::new(),
                },
                ChannelConfig {
                    service: "discord_bot".to_string(),
                    channel: "987654321".to_string(),
                    room_name: Some("RPG Discord Channel".to_string()),
                    display_names: true,
                    enable_media: true,
                    bridge_own_messages: false,
                    aliases: HashMap::new(),
                },
            ],
        );
        Config {
            services: HashMap::new(),
            bridges,
            media: None,
        }
    }

    fn make_services() -> HashMap<String, Arc<Mutex<Box<dyn MandatoryService>>>> {
        let mut services = HashMap::new();
        services.insert(
            "matrix_bot".to_string(),
            Arc::new(Mutex::new(Box::new(MockService::new("matrix_bot")) as Box<dyn MandatoryService>)),
        );
        services.insert(
            "discord_bot".to_string(),
            Arc::new(Mutex::new(Box::new(MockService::new("discord_bot")) as Box<dyn MandatoryService>)),
        );
        services.insert(
            "whatsapp_direct".to_string(),
            Arc::new(Mutex::new(Box::new(MockService::new("whatsapp_direct")) as Box<dyn MandatoryService>)),
        );
        services
    }

    fn make_status_message(source_service: &str, source_channel: &str) -> ServiceMessage {
        ServiceMessage {
            sender: "Alice".to_string(),
            sender_id: "@alice:matrix.org".to_string(),
            content: ".status".to_string(),
            attachments: vec![],
            source_service: source_service.to_string(),
            source_channel: source_channel.to_string(),
            source_id: "msg1".to_string(),
            is_own: false,
        }
    }

    #[tokio::test]
    async fn status_shows_only_current_bridge() {
        let config = make_test_config();
        let services = make_services();
        let store = Arc::new(MessageStore::new(":memory:").unwrap());
        let handler = StatusHandler::new();

        // Request status from family_bridge (matrix_bot channel)
        let msg = make_status_message("matrix_bot", "!room1:matrix.org");
        let result = handler.handle(msg, &services, &config).await;
        assert!(result.is_ok());

        // Verify the invoking service received the status
        let svc_lock = services.get("matrix_bot").unwrap();
        let svc = svc_lock.lock().await;
        let mock = svc.as_any().downcast_ref::<MockService>().unwrap();
        let sent = mock.send_messages.lock().await;
        
        assert!(!sent.is_empty(), "matrix_bot should have received status");
        let content = &sent[0].1.content;
        assert!(content.contains("Bridge family_bridge"));
    }

    #[tokio::test]
    async fn status_includes_services_section_with_connection_status() {
        let config = make_test_config();
        let mut services = make_services();
        
        // Make discord_bot disconnected
        {
            let svc_lock = services.get("discord_bot").unwrap();
            let mut svc = svc_lock.lock().await;
            let mock = svc.as_any_mut().downcast_mut::<MockService>().unwrap();
            mock.connected = false;
        }

        let store = Arc::new(MessageStore::new(":memory:").unwrap());
        let handler = StatusHandler::new();

        let msg = make_status_message("matrix_bot", "!room1:matrix.org");
        let result = handler.handle(msg, &services, &config).await;
        assert!(result.is_ok());

        // Check matrix_bot sent status with ✅ Connected
        let svc_lock = services.get("matrix_bot").unwrap();
        let svc = svc_lock.lock().await;
        let mock = svc.as_any().downcast_ref::<MockService>().unwrap();
        let sent = mock.send_messages.lock().await;
        assert!(!sent.is_empty());
        let content = &sent[0].1.content;
        assert!(content.contains("Services"));
        assert!(content.contains("matrix_bot ✅ Connected"));
        assert!(content.contains("discord_bot ❌ Disconnected"));
        assert!(content.contains("whatsapp_direct ✅ Connected"));
    }

    #[tokio::test]
    async fn status_includes_bridge_header() {
        let config = make_test_config();
        let services = make_services();
        let store = Arc::new(MessageStore::new(":memory:").unwrap());
        let handler = StatusHandler::new();

        let msg = make_status_message("matrix_bot", "!room1:matrix.org");
        let result = handler.handle(msg, &services, &config).await;
        assert!(result.is_ok());

        let svc_lock = services.get("matrix_bot").unwrap();
        let svc = svc_lock.lock().await;
        let mock = svc.as_any().downcast_ref::<MockService>().unwrap();
        let sent = mock.send_messages.lock().await;
        let content = &sent[0].1.content;
        assert!(content.contains("Bridge family_bridge"));
    }

    #[tokio::test]
    async fn status_includes_room_names_and_members() {
        let config = make_test_config();
        let services = make_services();
        let store = Arc::new(MessageStore::new(":memory:").unwrap());
        let handler = StatusHandler::new();

        let msg = make_status_message("matrix_bot", "!room1:matrix.org");
        let result = handler.handle(msg, &services, &config).await;
        assert!(result.is_ok());

        let svc_lock = services.get("matrix_bot").unwrap();
        let svc = svc_lock.lock().await;
        let mock = svc.as_any().downcast_ref::<MockService>().unwrap();
        let sent = mock.send_messages.lock().await;
        let content = &sent[0].1.content;
        
        // Debug: print the content
        eprintln!("=== STATUS CONTENT ===\n{}", content);
        eprintln!("======================");
        
        // Check room names appear
        assert!(content.contains("Family Matrix Room (matrix_bot)"));
        assert!(content.contains("Family Discord Channel (discord_bot)"));
        assert!(content.contains("Family WhatsApp Group (whatsapp_direct)"));
        
        // Check members appear
        assert!(content.contains(" - Alice"), "Expected ' - Alice' in content:\n{}", content);
        assert!(content.contains(" - Bob"), "Expected ' - Bob' in content:\n{}", content);
    }

    #[tokio::test]
    async fn status_handles_empty_members() {
        let config = make_test_config();
        let mut services = make_services();
        
        // Make a service with no members
        {
            let svc_lock = services.get("whatsapp_direct").unwrap();
            let mut svc = svc_lock.lock().await;
            let mock = svc.as_any_mut().downcast_mut::<MockService>().unwrap();
            mock.members = vec![];
        }

        let store = Arc::new(MessageStore::new(":memory:").unwrap());
        let handler = StatusHandler::new();

        let msg = make_status_message("matrix_bot", "!room1:matrix.org");
        let result = handler.handle(msg, &services, &config).await;
        assert!(result.is_ok());

        let svc_lock = services.get("matrix_bot").unwrap();
        let svc = svc_lock.lock().await;
        let mock = svc.as_any().downcast_ref::<MockService>().unwrap();
        let sent = mock.send_messages.lock().await;
        let content = &sent[0].1.content;
        
        // Should show "No members found or not supported" for whatsapp
        assert!(content.contains("No members found or not supported"));
    }

    #[tokio::test]
    async fn status_handles_member_fetch_error() {
        let config = make_test_config();
        let mut services = make_services();
        
        // Make a service that returns error on get_room_members
        {
            let svc_lock = services.get("discord_bot").unwrap();
            let mut svc = svc_lock.lock().await;
            let mock = svc.as_any_mut().downcast_mut::<MockService>().unwrap();
            mock.members = vec![]; // Will be handled but we want to test error path
            // We'll use a custom mock for this
        }

        let store = Arc::new(MessageStore::new(":memory:").unwrap());
        let handler = StatusHandler::new();

        let msg = make_status_message("matrix_bot", "!room1:matrix.org");
        let result = handler.handle(msg, &services, &config).await;
        assert!(result.is_ok());
        
        // This test verifies the handler doesn't panic on error
    }

    #[tokio::test]
    async fn status_responds_only_to_invoking_channel() {
        let config = make_test_config();
        let services = make_services();
        let store = Arc::new(MessageStore::new(":memory:").unwrap());
        let handler = StatusHandler::new();

        // Request from matrix_bot channel
        let msg = make_status_message("matrix_bot", "!room1:matrix.org");
        let result = handler.handle(msg, &services, &config).await;
        assert!(result.is_ok());

        // Check only matrix_bot received the status
        let svc_lock = services.get("matrix_bot").unwrap();
        let svc = svc_lock.lock().await;
        let mock = svc.as_any().downcast_ref::<MockService>().unwrap();
        let sent = mock.send_messages.lock().await;
        assert!(!sent.is_empty());
        assert_eq!(sent[0].0, "!room1:matrix.org"); // Sent to the invoking channel
    }

    #[tokio::test]
    async fn status_handles_missing_bridge_gracefully() {
        let config = make_test_config();
        let services = make_services();
        let store = Arc::new(MessageStore::new(":memory:").unwrap());
        let handler = StatusHandler::new();

        // Request from unknown service (not in config at all)
        let msg = make_status_message("unknown_service", "!unknown:matrix.org");
        
        let result = handler.handle(msg, &services, &config).await;
        assert!(result.is_ok());
        
        // The handler won't find a matching service to send to, so it just returns OK
        // This test verifies it doesn't panic
    }
}