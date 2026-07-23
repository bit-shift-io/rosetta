use crate::bridge::edit_handler::EditHandler;
use crate::config::{ChannelConfig, Config};
use crate::persistence::MessageStore;
use crate::services::{Service, ServiceUpdate, ServiceEvent};
use crate::services::traits::{MessageEditor, Connectable, MessageSender, ServiceInfo};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use anyhow::Result;

struct MockEditorService {
    name: String,
    edits: Mutex<Vec<(String, String, String)>>, // channel, message_id, new_content
}

impl MockEditorService {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            edits: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl Connectable for MockEditorService {
    async fn connect(&mut self) -> Result<()> { Ok(()) }
    async fn start(&mut self, _tx: tokio::sync::mpsc::Sender<ServiceEvent>) -> Result<()> { Ok(()) }
    fn is_connected(&self) -> bool { true }
    async fn wait_until_ready(&self) -> Result<()> { Ok(()) }
    async fn disconnect(&mut self) -> Result<()> { Ok(()) }
}

#[async_trait::async_trait]
impl MessageSender for MockEditorService {
    async fn send_message(&self, _channel: &str, _message: &crate::services::ServiceMessage) -> Result<String> {
        Ok("msg123".to_string())
    }
}

impl ServiceInfo for MockEditorService {
    fn service_name(&self) -> &str { &self.name }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

#[async_trait::async_trait]
impl MessageEditor for MockEditorService {
    async fn edit_message(&self, channel: &str, message_id: &str, new_content: &str) -> Result<()> {
        let mut edits = self.edits.lock().await;
        edits.push((channel.to_string(), message_id.to_string(), new_content.to_string()));
        Ok(())
    }
}

// Mock service without MessageEditor
struct MockNoEditorService {
    name: String,
}

impl MockNoEditorService {
    fn new(name: &str) -> Self {
        Self { name: name.to_string() }
    }
}

#[async_trait::async_trait]
impl Connectable for MockNoEditorService {
    async fn connect(&mut self) -> Result<()> { Ok(()) }
    async fn start(&mut self, _tx: tokio::sync::mpsc::Sender<ServiceEvent>) -> Result<()> { Ok(()) }
    fn is_connected(&self) -> bool { true }
    async fn wait_until_ready(&self) -> Result<()> { Ok(()) }
    async fn disconnect(&mut self) -> Result<()> { Ok(()) }
}

#[async_trait::async_trait]
impl MessageSender for MockNoEditorService {
    async fn send_message(&self, _channel: &str, _message: &crate::services::ServiceMessage) -> Result<String> {
        Ok("msg123".to_string())
    }
}

impl ServiceInfo for MockNoEditorService {
    fn service_name(&self) -> &str { &self.name }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

fn make_test_config() -> Config {
    let mut bridges = HashMap::new();
    bridges.insert("bridge1".to_string(), vec![
        ChannelConfig {
            service: "matrix".to_string(),
            channel: "!room1:matrix.org".to_string(),
            display_names: true,
            enable_media: true,
            bridge_own_messages: true,
            aliases: HashMap::new(),
        },
        ChannelConfig {
            service: "discord".to_string(),
            channel: "123456".to_string(),
            display_names: true,
            enable_media: true,
            bridge_own_messages: true,
            aliases: HashMap::new(),
        },
    ]);
    Config {
        services: HashMap::new(),
        bridges,
        media: None,
    }
}

fn make_services() -> HashMap<String, Arc<Mutex<Box<dyn Connectable + MessageSender + MessageEditor + ServiceInfo + Send + Sync>>>> {
    let mut services = HashMap::new();
    services.insert("matrix".to_string(), Arc::new(Mutex::new(Box::new(MockEditorService::new("matrix")))));
    services.insert("discord".to_string(), Arc::new(Mutex::new(Box::new(MockEditorService::new("discord")))));
    services
}

fn make_services_with_no_editor() -> HashMap<String, Arc<Mutex<Box<dyn Connectable + MessageSender + ServiceInfo + Send + Sync>>>> {
    let mut services = HashMap::new();
    services.insert("matrix".to_string(), Arc::new(Mutex::new(Box::new(MockEditorService::new("matrix")))));
    services.insert("discord".to_string(), Arc::new(Mutex::new(Box::new(MockNoEditorService::new("discord")))));
    services
}

#[tokio::test]
async fn edit_handler_processes_edit_across_bridges() {
    let config = make_test_config();
    let services = make_services();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());

    // Create mappings: matrix msg1 -> discord msg123
    store.save_mapping("matrix", "!room1:matrix.org", "msg1", "discord", "123456", "msg123").unwrap();

    let handler = EditHandler::new();

    let update = ServiceUpdate {
        source_service: "matrix".to_string(),
        source_channel: "!room1:matrix.org".to_string(),
        source_id: "msg1".to_string(),
        new_content: "Updated content".to_string(),
        is_own: false,
    };

    handler.handle(update, &services, &config, &store).await.unwrap();

    // Verify discord service received the edit
    let discord_svc = services.get("discord").unwrap();
    let edits = discord_svc.lock().await.edits.lock().await;
    assert_eq!(edits.len(), 1);
    assert_eq!(edits[0].0, "123456"); // channel
    assert_eq!(edits[0].1, "msg123"); // message_id
    assert_eq!(edits[0].2, "Updated content"); // new_content
}

#[tokio::test]
async fn edit_handler_respects_bridge_own_messages() {
    let mut config = make_test_config();
    // Disable bridge_own_messages for matrix
    if let Some(channels) = config.bridges.get_mut("bridge1") {
        for ch in channels {
            if ch.service == "matrix" {
                ch.bridge_own_messages = false;
            }
        }
    }

    let services = make_services();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());

    let handler = EditHandler::new();

    let update = ServiceUpdate {
        source_service: "matrix".to_string(),
        source_channel: "!room1:matrix.org".to_string(),
        source_id: "msg1".to_string(),
        new_content: "Updated content".to_string(),
        is_own: true, // This is our own message
    };

    handler.handle(update, &services, &config, &store).await.unwrap();

    // Should not have called edit on any service
    let matrix_svc = services.get("matrix").unwrap();
    let edits = matrix_svc.lock().await.edits.lock().await;
    assert_eq!(edits.len(), 0);
}

#[tokio::test]
async fn edit_handler_allows_own_when_bridge_own_messages_true() {
    let config = make_test_config(); // bridge_own_messages is true by default in make_test_config
    let services = make_services();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());

    // Create mappings
    store.save_mapping("matrix", "!room1:matrix.org", "msg1", "discord", "123456", "msg123").unwrap();

    let handler = EditHandler::new();

    let update = ServiceUpdate {
        source_service: "matrix".to_string(),
        source_channel: "!room1:matrix.org".to_string(),
        source_id: "msg1".to_string(),
        new_content: "Updated content".to_string(),
        is_own: true,
    };

    handler.handle(update, &services, &config, &store).await.unwrap();

    // Should have called edit on discord
    let discord_svc = services.get("discord").unwrap();
    let edits = discord_svc.lock().await.edits.lock().await;
    assert_eq!(edits.len(), 1);
}

#[tokio::test]
async fn edit_handler_skips_services_without_message_editor() {
    let config = make_test_config();
    let services = make_services_with_no_editor();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());

    // Create mappings
    store.save_mapping("matrix", "!room1:matrix.org", "msg1", "discord", "123456", "msg123").unwrap();

    let handler = EditHandler::new();

    let update = ServiceUpdate {
        source_service: "matrix".to_string(),
        source_channel: "!room1:matrix.org".to_string(),
        source_id: "msg1".to_string(),
        new_content: "Updated content".to_string(),
        is_own: false,
    };

    // Should not panic, should just skip discord
    handler.handle(update, &services, &config, &store).await.unwrap();
}

#[tokio::test]
async fn edit_handler_handles_multiple_destinations() {
    let mut config = make_test_config();
    config.bridges.insert("bridge1".to_string(), vec![
        ChannelConfig {
            service: "matrix".to_string(),
            channel: "!room1:matrix.org".to_string(),
            display_names: true,
            enable_media: true,
            bridge_own_messages: true,
            aliases: HashMap::new(),
        },
        ChannelConfig {
            service: "discord".to_string(),
            channel: "123456".to_string(),
            display_names: true,
            enable_media: true,
            bridge_own_messages: true,
            aliases: HashMap::new(),
        },
        ChannelConfig {
            service: "whatsapp".to_string(),
            channel: "group1".to_string(),
            display_names: true,
            enable_media: true,
            bridge_own_messages: true,
            aliases: HashMap::new(),
        },
    ]);

    let mut services = HashMap::new();
    services.insert("matrix".to_string(), Arc::new(Mutex::new(Box::new(MockEditorService::new("matrix")))));
    services.insert("discord".to_string(), Arc::new(Mutex::new(Box::new(MockEditorService::new("discord")))));
    services.insert("whatsapp".to_string(), Arc::new(Mutex::new(Box::new(MockEditorService::new("whatsapp")))));

    let store = Arc::new(MessageStore::new(":memory:").unwrap());
    store.save_mapping("matrix", "!room1:matrix.org", "msg1", "discord", "123456", "msg123").unwrap();
    store.save_mapping("matrix", "!room1:matrix.org", "msg1", "whatsapp", "group1", "msg456").unwrap();

    let handler = EditHandler::new();

    let update = ServiceUpdate {
        source_service: "matrix".to_string(),
        source_channel: "!room1:matrix.org".to_string(),
        source_id: "msg1".to_string(),
        new_content: "Updated content".to_string(),
        is_own: false,
    };

    handler.handle(update, &services, &config, &store).await.unwrap();

    // Both discord and whatsapp should have received the edit
    let discord_svc = services.get("discord").unwrap();
    let discord_edits = discord_svc.lock().await.edits.lock().await;
    assert_eq!(discord_edits.len(), 1);

    let whatsapp_svc = services.get("whatsapp").unwrap();
    let whatsapp_edits = whatsapp_svc.lock().await.edits.lock().await;
    assert_eq!(whatsapp_edits.len(), 1);
}