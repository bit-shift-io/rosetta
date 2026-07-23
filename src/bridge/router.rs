use crate::bridge::dispatcher::MessageDispatcher;
use crate::bridge::edit_handler::EditHandler;
use crate::bridge::reaction_handler::ReactionHandler;
use crate::bridge::status_handler::StatusHandler;
use crate::config::Config;
use crate::persistence::MessageStore;
use crate::services::ServiceEvent;
use crate::services::traits::MandatoryService;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Central event router that receives ServiceEvents and delegates to appropriate handlers
pub struct EventRouter {
    dispatcher: MessageDispatcher,
    edit_handler: EditHandler,
    reaction_handler: ReactionHandler,
    status_handler: StatusHandler,
}

impl EventRouter {
    pub fn new(
        dispatcher: MessageDispatcher,
        edit_handler: EditHandler,
        reaction_handler: ReactionHandler,
        status_handler: StatusHandler,
    ) -> Self {
        Self {
            dispatcher,
            edit_handler,
            reaction_handler,
            status_handler,
        }
    }

    /// Route an incoming event to the appropriate handler
    pub async fn route(
        &self,
        event: ServiceEvent,
        services: &HashMap<String, Arc<Mutex<Box<dyn MandatoryService>>>>,
        config: &Config,
        store: &MessageStore,
    ) -> Result<()> {
        match event {
            ServiceEvent::NewMessage(msg) => {
                // Check for .status command first
                if msg.content.trim() == ".status" {
                    self.status_handler.handle(msg, services, config).await?;
                } else {
                    self.dispatcher.dispatch(msg, services, config).await?;
                }
            }
            ServiceEvent::UpdateMessage(update) => {
                self.edit_handler
                    .handle(update, services, config, store)
                    .await?;
            }
            ServiceEvent::NewReaction(reaction) => {
                self.reaction_handler
                    .handle(reaction, services, config, store)
                    .await?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bridge::alias_resolver::AliasResolver;
    use crate::bridge::dispatcher::MessageDispatcher;
    use crate::bridge::edit_handler::EditHandler;
    use crate::bridge::formatter::MessageFormatter;
    use crate::bridge::matcher::BridgeMatcher;
    use crate::bridge::media::MediaHandler;
    use crate::bridge::reaction_handler::ReactionHandler;
    use crate::bridge::status_handler::StatusHandler;
    use crate::config::{ChannelConfig, Config};
    use crate::persistence::MessageStore;
    use crate::services::traits::{
        Connectable, MandatoryService, MemberLister, MessageEditor, MessageSender, ReactionSender,
        ServiceInfo,
    };
    use crate::services::{ServiceEvent, ServiceMessage, ServiceReaction, ServiceUpdate};
    use anyhow::Result;
    use async_trait::async_trait;
    use std::any::Any;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tokio::sync::mpsc;

    struct MockService {
        name: String,
    }

    impl MockService {
        fn new(name: &str) -> Self {
            Self {
                name: name.to_string(),
            }
        }
    }

    #[async_trait]
    impl Connectable for MockService {
        async fn connect(&mut self) -> Result<()> {
            Ok(())
        }
        async fn start(&mut self, _tx: mpsc::Sender<ServiceEvent>) -> Result<()> {
            Ok(())
        }
        fn is_connected(&self) -> bool {
            true
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
        async fn react_to_message(
            &self,
            _channel: &str,
            _message_id: &str,
            _emoji: &str,
        ) -> Result<()> {
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
            Ok(vec![])
        }
    }

    #[async_trait]
    impl MessageEditor for MockService {
        async fn edit_message(
            &self,
            _channel: &str,
            _message_id: &str,
            _new_content: &str,
        ) -> Result<()> {
            Ok(())
        }
    }

    fn make_test_config() -> Config {
        let mut bridges = HashMap::new();
        bridges.insert(
            "bridge1".to_string(),
            vec![
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
            "matrix".to_string(),
            Arc::new(Mutex::new(
                Box::new(MockService::new("matrix")) as Box<dyn MandatoryService>
            )),
        );
        services.insert(
            "discord".to_string(),
            Arc::new(Mutex::new(
                Box::new(MockService::new("discord")) as Box<dyn MandatoryService>
            )),
        );
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

    fn make_router() -> EventRouter {
        let formatters: HashMap<String, Box<dyn MessageFormatter + Send + Sync>> = HashMap::new();
        let store = Arc::new(MessageStore::new(":memory:").unwrap());

        let dispatcher = MessageDispatcher::new(
            BridgeMatcher,
            AliasResolver,
            MediaHandler,
            formatters,
            store.clone(),
        );
        let edit_handler = EditHandler::new();
        let reaction_handler = ReactionHandler::new();
        let status_handler = StatusHandler::new();

        EventRouter::new(dispatcher, edit_handler, reaction_handler, status_handler)
    }

    #[tokio::test]
    async fn route_new_message_delegates_to_dispatcher() {
        let config = make_test_config();
        let services = make_services();
        let store = Arc::new(MessageStore::new(":memory:").unwrap());
        let router = make_router();

        let msg = make_message("matrix", "!room1:matrix.org", "Hello!");

        let result = router
            .route(ServiceEvent::NewMessage(msg), &services, &config, &store)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn route_status_command_delegates_to_status_handler() {
        let config = make_test_config();
        let services = make_services();
        let store = Arc::new(MessageStore::new(":memory:").unwrap());
        let router = make_router();

        let msg = make_message("matrix", "!room1:matrix.org", ".status");

        let result = router
            .route(ServiceEvent::NewMessage(msg), &services, &config, &store)
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn route_update_message_delegates_to_edit_handler() {
        let config = make_test_config();
        let services = make_services();
        let store = Arc::new(MessageStore::new(":memory:").unwrap());

        // Create mapping for edit to work
        store
            .save_mapping(
                "matrix",
                "!room1:matrix.org",
                "msg1",
                "discord",
                "123456",
                "msg123",
            )
            .unwrap();

        let router = make_router();

        let update = ServiceUpdate {
            source_service: "matrix".to_string(),
            source_channel: "!room1:matrix.org".to_string(),
            source_id: "msg1".to_string(),
            new_content: "Updated content".to_string(),
            is_own: false,
        };

        let result = router
            .route(
                ServiceEvent::UpdateMessage(update),
                &services,
                &config,
                &store,
            )
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn route_reaction_delegates_to_reaction_handler() {
        let config = make_test_config();
        let services = make_services();
        let store = Arc::new(MessageStore::new(":memory:").unwrap());

        // Create mapping for reaction to work
        store
            .save_mapping(
                "matrix",
                "!room1:matrix.org",
                "msg1",
                "discord",
                "123456",
                "msg123",
            )
            .unwrap();

        let router = make_router();

        let reaction = ServiceReaction {
            source_service: "matrix".to_string(),
            source_channel: "!room1:matrix.org".to_string(),
            source_message_id: "msg1".to_string(),
            _sender: "Alice".to_string(),
            emoji: "👍".to_string(),
            is_own: false,
        };

        let result = router
            .route(
                ServiceEvent::NewReaction(reaction),
                &services,
                &config,
                &store,
            )
            .await;

        assert!(result.is_ok());
    }
}
