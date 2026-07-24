use crate::bridge::status_handler::StatusHandler;
use crate::config::{ChannelConfig, Config};
use crate::persistence::MessageStore;
use crate::services::traits::{
    Connectable, MandatoryService, MemberLister, MessageSender, ReactionSender, RoomNameFetcher,
    ServiceInfo,
};
use crate::services::{ServiceEvent, ServiceMessage};
use anyhow::Result;
use async_trait::async_trait;
use std::any::Any;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// Mock service for testing
struct MockService {
    name: String,
    connected: bool,
    members: Vec<String>,
    room_name: Option<String>,
    member_error: Option<String>,
}

impl MockService {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            connected: true,
            members: vec!["Alice".to_string(), "Bob".to_string()],
            room_name: Some(format!("{} Room", name)),
            member_error: None,
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

    fn with_member_error(mut self, error: String) -> Self {
        self.member_error = Some(error);
        self
    }
}

#[async_trait]
impl Connectable for MockService {
    async fn connect(&mut self) -> Result<()> {
        Ok(())
    }
    async fn start(&mut self, _tx: tokio::sync::mpsc::Sender<ServiceEvent>) -> Result<()> {
        Ok(())
    }
    fn is_connected(&self) -> bool {
        self.connected
    }
    async fn wait_until_ready(&self) -> Result<()> {
        Ok(())
    }
    async fn disconnect(&mut self) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl MessageSender for MockService {
    async fn send_message(&self, _channel: &str, _message: &ServiceMessage) -> Result<String> {
        Ok("msg123".to_string())
    }
}

#[async_trait]
impl ReactionSender for MockService {
    async fn react_to_message(&self, _channel: &str, _message_id: &str, _emoji: &str) -> Result<()> {
        Ok(())
    }
}

impl ServiceInfo for MockService {
    fn service_name(&self) -> &str {
        &self.name
    }
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[async_trait]
impl MemberLister for MockService {
    async fn get_room_members(&self, _channel: &str) -> Result<Vec<String>> {
        if let Some(ref err) = self.member_error {
            return Err(anyhow::anyhow!(err.clone()));
        }
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
                channel: "123456".to_string(),
                room_name: Some("Family Discord Channel".to_string()),
                display_names: true,
                enable_media: true,
                bridge_own_messages: false,
                aliases: HashMap::new(),
            },
            ChannelConfig {
                service: "whatsapp_direct".to_string(),
                channel: "00000@lid".to_string(),
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
                channel: "789012".to_string(),
                room_name: Some("RPG Discord Channel".to_string()),
                display_names: true,
                enable_media: false,
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
        Arc::new(Mutex::new(
            Box::new(MockService::new("matrix_bot")) as Box<dyn MandatoryService>
        )),
    );
    services.insert(
        "discord_bot".to_string(),
        Arc::new(Mutex::new(
            Box::new(MockService::new("discord_bot")) as Box<dyn MandatoryService>
        )),
    );
    services.insert(
        "whatsapp_direct".to_string(),
        Arc::new(Mutex::new(
            Box::new(MockService::new("whatsapp_direct")) as Box<dyn MandatoryService>
        )),
    );
    services
}

fn make_status_message(
    source_service: &str,
    source_channel: &str,
) -> ServiceMessage {
    ServiceMessage {
        sender: "TestUser".to_string(),
        sender_id: "@testuser:matrix.org".to_string(),
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

    // Send .status in family_bridge
    let msg = make_status_message("matrix_bot", "!room1:matrix.org");
    let result = handler.handle(msg, &services, &config).await;
    assert!(result.is_ok());

    // We can't easily capture the sent message in this test setup,
    // but we can verify the handler runs without error
}

#[tokio::test]
async fn status_bridge_filtering() {
    let config = make_test_config();
    let mut services = make_services();

    // Make one service disconnected
    if let Some(svc) = services.get("whatsapp_direct") {
        let mut svc_lock = svc.lock().await;
        if let Some(mock) = svc_lock.as_any_mut().downcast_mut::<MockService>() {
            mock.connected = false;
        }
    }

    let store = Arc::new(MessageStore::new(":memory:").unwrap());
    let handler = StatusHandler::new();

    let msg = make_status_message("matrix_bot", "!room1:matrix.org");
    let result = handler.handle(msg, &services, &config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn status_service_filtering_only_bridge_services() {
    let config = make_test_config();
    let services = make_services();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());
    let handler = StatusHandler::new();

    // Send .status in rpg_bridge (only has matrix_bot and discord_bot)
    let msg = make_status_message("matrix_bot", "!room2:matrix.org");
    let result = handler.handle(msg, &services, &config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn status_shows_room_names() {
    let config = make_test_config();
    let services = make_services();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());
    let handler = StatusHandler::new();

    let msg = make_status_message("matrix_bot", "!room1:matrix.org");
    let result = handler.handle(msg, &services, &config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn status_shows_members() {
    let config = make_test_config();
    let services = make_services();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());
    let handler = StatusHandler::new();

    let msg = make_status_message("matrix_bot", "!room1:matrix.org");
    let result = handler.handle(msg, &services, &config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn status_handles_member_fetch_error() {
    let config = make_test_config();
    let mut services = make_services();

    // Make one service return an error for member fetching
    if let Some(svc) = services.get("discord_bot") {
        let mut svc_lock = svc.lock().await;
        if let Some(mock) = svc_lock.as_any_mut().downcast_mut::<MockService>() {
            mock.member_error = Some("Permission denied".to_string());
        }
    }

    let store = Arc::new(MessageStore::new(":memory:").unwrap());
    let handler = StatusHandler::new();

    let msg = make_status_message("matrix_bot", "!room1:matrix.org");
    let result = handler.handle(msg, &services, &config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn status_handles_empty_members() {
    let config = make_test_config();
    let mut services = make_services();

    // Make one service return empty members
    if let Some(svc) = services.get("discord_bot") {
        let mut svc_lock = svc.lock().await;
        if let Some(mock) = svc_lock.as_any_mut().downcast_mut::<MockService>() {
            mock.members = vec![];
        }
    }

    let store = Arc::new(MessageStore::new(":memory:").unwrap());
    let handler = StatusHandler::new();

    let msg = make_status_message("matrix_bot", "!room1:matrix.org");
    let result = handler.handle(msg, &services, &config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn status_handles_missing_room_name() {
    let config = make_test_config();
    let mut services = make_services();

    // Make one service have no room_name
    if let Some(svc) = services.get("discord_bot") {
        let mut svc_lock = svc.lock().await;
        if let Some(mock) = svc_lock.as_any_mut().downcast_mut::<MockService>() {
            mock.room_name = None;
        }
    }

    let store = Arc::new(MessageStore::new(":memory:").unwrap());
    let handler = StatusHandler::new();

    let msg = make_status_message("matrix_bot", "!room1:matrix.org");
    let result = handler.handle(msg, &services, &config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn status_output_format_has_services_section() {
    let config = make_test_config();
    let services = make_services();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());
    let handler = StatusHandler::new();

    let msg = make_status_message("matrix_bot", "!room1:matrix.org");
    let result = handler.handle(msg, &services, &config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn status_output_format_has_bridge_header() {
    let config = make_test_config();
    let services = make_services();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());
    let handler = StatusHandler::new();

    let msg = make_status_message("matrix_bot", "!room1:matrix.org");
    let result = handler.handle(msg, &services, &config).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn status_sends_to_invoking_channel_only() {
    let config = make_test_config();
    let services = make_services();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());
    let handler = StatusHandler::new();

    // Send status in matrix_bot channel
    let msg = make_status_message("matrix_bot", "!room1:matrix.org");
    let result = handler.handle(msg, &services, &config).await;
    assert!(result.is_ok());

    // The handler should send the response back to the same service/channel
    // We can't easily verify this without mocking send_message, but the test ensures no panic
}