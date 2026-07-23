use crate::bridge::dispatcher::MessageDispatcher;
use crate::bridge::matcher::BridgeMatcher;
use crate::bridge::alias_resolver::AliasResolver;
use crate::bridge::media::MediaHandler;
use crate::bridge::formatter::MessageFormatter;
use crate::config::{ChannelConfig, Config};
use crate::services::{Attachment, ServiceMessage, ServiceEvent};
use crate::services::traits::{MessageSender, Connectable, ServiceInfo};
use crate::persistence::MessageStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

// Mock trait objects for testing
struct MockService {
    name: String,
    messages_sent: Mutex<Vec<(String, ServiceMessage)>>,
}

impl MockService {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            messages_sent: Mutex::new(Vec::new()),
        }
    }
}

#[async_trait::async_trait]
impl Connectable for MockService {
    async fn connect(&mut self) -> anyhow::Result<()> { Ok(()) }
    async fn start(&mut self, _tx: tokio::sync::mpsc::Sender<ServiceEvent>) -> anyhow::Result<()> { Ok(()) }
    fn is_connected(&self) -> bool { true }
    async fn wait_until_ready(&self) -> anyhow::Result<()> { Ok(()) }
    async fn disconnect(&mut self) -> anyhow::Result<()> { Ok(()) }
}

#[async_trait::async_trait]
impl MessageSender for MockService {
    async fn send_message(&self, channel: &str, message: &ServiceMessage) -> anyhow::Result<String> {
        let mut sent = self.messages_sent.lock().await;
        sent.push((channel.to_string(), message.clone()));
        Ok("msg123".to_string())
    }
}

impl ServiceInfo for MockService {
    fn service_name(&self) -> &str { &self.name }
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
}

// Simple formatter for testing
struct TestFormatter;

impl MessageFormatter for TestFormatter {
    fn format_text(&self, sender: &str, content: &str, _is_own: bool) -> String {
        if sender.is_empty() { content.to_string() } else { format!("{}: {}", sender, content) }
    }
    fn format_caption(&self, sender: &str, content: &str, _is_own: bool) -> String {
        if sender.is_empty() { content.to_string() } else { format!("{}: {}", sender, content) }
    }
}

fn make_test_config() -> Config {
    let mut bridges = HashMap::new();
    bridges.insert("bridge1".to_string(), vec![
        ChannelConfig {
            service: "matrix".to_string(),
            channel: "!room1:matrix.org".to_string(),
            display_names: true,
            enable_media: true,
            bridge_own_messages: false,
            aliases: HashMap::new(),
        },
        ChannelConfig {
            service: "discord".to_string(),
            channel: "123456".to_string(),
            display_names: true,
            enable_media: true,
            bridge_own_messages: false,
            aliases: HashMap::new(),
        },
    ]);
    Config {
        services: HashMap::new(),
        bridges,
        media: None,
    }
}

fn make_test_services() -> HashMap<String, Arc<Mutex<Box<dyn Connectable + MessageSender + ServiceInfo + Send + Sync>>>> {
    let mut services = HashMap::new();
    services.insert("matrix".to_string(), Arc::new(Mutex::new(Box::new(MockService::new("matrix")))));
    services.insert("discord".to_string(), Arc::new(Mutex::new(Box::new(MockService::new("discord")))));
    services
}

fn make_message(source_service: &str, source_channel: &str, content: &str) -> ServiceMessage {
    ServiceMessage {
        sender: "Alice".to_string(),
        sender_id: "@alice:matrix.org".to_string(),
        content: content.to_string(),
        attachments: vec![],
        source_service: source_service.to_string(),
        source_channel: source_channel.to_string(),
        source_id: "msg1".to_string(),
        is_own: false,
    }
}

#[tokio::test]
async fn dispatch_forwards_to_all_target_channels() {
    let config = make_test_config();
    let services = make_test_services();
    let matcher = BridgeMatcher;
    let alias_resolver = AliasResolver;
    let media_handler = MediaHandler;
    let formatters: HashMap<String, Box<dyn MessageFormatter + Send + Sync>> = HashMap::new();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());

    let dispatcher = MessageDispatcher::new(
        matcher, alias_resolver, media_handler, formatters, store
    );

    let msg = make_message("matrix", "!room1:matrix.org", "Hello!");
    
    dispatcher.dispatch(msg, &services, &config).await.unwrap();

    // Verify messages were sent to discord
    let discord_svc = services.get("discord").unwrap();
    let sent = discord_svc.lock().await.messages_sent.lock().await;
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, "123456"); // discord channel
    assert_eq!(sent[0].1.content, "Hello!");
}

#[tokio::test]
async fn dispatch_skips_source_channel() {
    let config = make_test_config();
    let services = make_test_services();
    let matcher = BridgeMatcher;
    let alias_resolver = AliasResolver;
    let media_handler = MediaHandler;
    let formatters: HashMap<String, Box<dyn MessageFormatter + Send + Sync>> = HashMap::new();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());

    let dispatcher = MessageDispatcher::new(
        matcher, alias_resolver, media_handler, formatters, store
    );

    // Send from discord channel
    let msg = make_message("discord", "123456", "Hello from Discord!");
    
    dispatcher.dispatch(msg, &services, &config).await.unwrap();

    // Only matrix should receive it
    let matrix_svc = services.get("matrix").unwrap();
    let matrix_sent = matrix_svc.lock().await.messages_sent.lock().await;
    assert_eq!(matrix_sent.len(), 1);
    assert_eq!(matrix_sent[0].0, "!room1:matrix.org");

    // Discord should not receive its own message
    let discord_svc = services.get("discord").unwrap();
    let discord_sent = discord_svc.lock().await.messages_sent.lock().await;
    assert_eq!(discord_sent.len(), 0);
}

#[tokio::test]
async fn dispatch_applies_media_policy() {
    let mut config = make_test_config();
    // Disable media for discord
    if let Some(channels) = config.bridges.get_mut("bridge1") {
        for ch in channels {
            if ch.service == "discord" {
                ch.enable_media = false;
            }
        }
    }

    let services = make_test_services();
    let matcher = BridgeMatcher;
    let alias_resolver = AliasResolver;
    let media_handler = MediaHandler;
    let formatters: HashMap<String, Box<dyn MessageFormatter + Send + Sync>> = HashMap::new();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());

    let dispatcher = MessageDispatcher::new(
        matcher, alias_resolver, media_handler, formatters, store
    );

    let mut msg = make_message("matrix", "!room1:matrix.org", "Check this image!");
    msg.attachments.push(Attachment {
        filename: "image.png".to_string(),
        mime_type: "image/png".to_string(),
        data: vec![1,2,3],
    });

    dispatcher.dispatch(msg, &services, &config).await.unwrap();

    let discord_svc = services.get("discord").unwrap();
    let sent = discord_svc.lock().await.messages_sent.lock().await;
    assert_eq!(sent.len(), 1);
    // Attachments should be stripped
    assert!(sent[0].1.attachments.is_empty());
}

#[tokio::test]
async fn dispatch_applies_display_names_policy() {
    let mut config = make_test_config();
    // Disable display names for discord
    if let Some(channels) = config.bridges.get_mut("bridge1") {
        for ch in channels {
            if ch.service == "discord" {
                ch.display_names = false;
            }
        }
    }

    let services = make_test_services();
    let matcher = BridgeMatcher;
    let alias_resolver = AliasResolver;
    let media_handler = MediaHandler;
    let formatters: HashMap<String, Box<dyn MessageFormatter + Send + Sync>> = HashMap::new();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());

    let dispatcher = MessageDispatcher::new(
        matcher, alias_resolver, media_handler, formatters, store
    );

    let msg = make_message("matrix", "!room1:matrix.org", "Hello!");
    
    dispatcher.dispatch(msg, &services, &config).await.unwrap();

    let discord_svc = services.get("discord").unwrap();
    let sent = discord_svc.lock().await.messages_sent.lock().await;
    assert_eq!(sent.len(), 1);
    // Sender should be empty when display_names is false
    assert_eq!(sent[0].1.sender, "");
}

#[tokio::test]
async fn dispatch_resolves_aliases() {
    let mut config = make_test_config();
    // Add alias for matrix channel
    if let Some(channels) = config.bridges.get_mut("bridge1") {
        for ch in channels {
            if ch.service == "matrix" {
                ch.aliases.insert("@alice:matrix.org".to_string(), "Alice Smith".to_string());
            }
        }
    }

    let services = make_test_services();
    let matcher = BridgeMatcher;
    let alias_resolver = AliasResolver;
    let media_handler = MediaHandler;
    let formatters: HashMap<String, Box<dyn MessageFormatter + Send + Sync>> = HashMap::new();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());

    let dispatcher = MessageDispatcher::new(
        matcher, alias_resolver, media_handler, formatters, store
    );

    let msg = make_message("matrix", "!room1:matrix.org", "Hello!");
    // Note: original sender_id is @alice:matrix.org

    dispatcher.dispatch(msg, &services, &config).await.unwrap();

    let discord_svc = services.get("discord").unwrap();
    let sent = discord_svc.lock().await.messages_sent.lock().await;
    assert_eq!(sent.len(), 1);
    // Sender should be resolved to alias
    assert_eq!(sent[0].1.sender, "Alice Smith");
}

#[tokio::test]
async fn dispatch_saves_message_mapping() {
    let config = make_test_config();
    let services = make_test_services();
    let matcher = BridgeMatcher;
    let alias_resolver = AliasResolver;
    let media_handler = MediaHandler;
    let formatters: HashMap<String, Box<dyn MessageFormatter + Send + Sync>> = HashMap::new();
    let store = Arc::new(MessageStore::new(":memory:").unwrap());

    let dispatcher = MessageDispatcher::new(
        matcher, alias_resolver, media_handler, formatters, store.clone()
    );

    let msg = make_message("matrix", "!room1:matrix.org", "Hello!");
    
    dispatcher.dispatch(msg, &services, &config).await.unwrap();

    // Verify mapping was saved
    let mappings = store.get_associated_mappings("matrix", "!room1:matrix.org", "msg1").unwrap();
    assert!(!mappings.is_empty());
    // Should have mapping to discord
    assert!(mappings.iter().any(|(svc, _, _)| svc == "discord"));
}