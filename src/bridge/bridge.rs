use anyhow::Result;
use log::{error, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::bridge::alias_resolver::AliasResolver;
use crate::bridge::dispatcher::MessageDispatcher;
use crate::bridge::edit_handler::EditHandler;
use crate::bridge::formatter::MessageFormatter;
use crate::bridge::matcher::BridgeMatcher;
use crate::bridge::media::MediaHandler;
use crate::bridge::reaction_handler::ReactionHandler;
use crate::bridge::router::EventRouter;
use crate::bridge::status_handler::StatusHandler;
use crate::config::Config;
use crate::persistence::MessageStore;
use crate::services::formatter::{DiscordFormatter, MatrixFormatter, WhatsAppFormatter};
use crate::services::{Service, ServiceEvent, ServiceMessage, ServiceUpdate};

/// Manages multiple bridges and routes messages between services
pub struct BridgeCoordinator {
    config: Config,
    services: HashMap<String, Arc<tokio::sync::Mutex<Box<dyn Service>>>>,
    store: Arc<MessageStore>,
    router: EventRouter,
}

impl BridgeCoordinator {
    pub fn new(
        config: Config,
        services: HashMap<String, Arc<tokio::sync::Mutex<Box<dyn Service>>>>,
    ) -> Result<Self> {
        let store = Arc::new(MessageStore::new("data/message_history.db")?);

        // Create formatters for each service
        let mut formatters: HashMap<String, Box<dyn MessageFormatter + Send + Sync>> =
            HashMap::new();
        formatters.insert("matrix".to_string(), Box::new(MatrixFormatter::new()));
        formatters.insert("discord".to_string(), Box::new(DiscordFormatter::new()));
        formatters.insert("whatsapp".to_string(), Box::new(WhatsAppFormatter::new()));

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

        let router = EventRouter::new(dispatcher, edit_handler, reaction_handler, status_handler);

        Ok(Self {
            config,
            services,
            store,
            router,
        })
    }

    /// Start all bridges and begin routing messages
    pub async fn start(&self) -> Result<()> {
        info!("Starting Bridge Coordinator...");
        // ... existing code ...
        let (tx, mut event_rx) = mpsc::channel::<ServiceEvent>(100);

        // Start all services
        for (service_name, service) in &self.services {
            let mut svc = service.lock().await;
            match svc.start(tx.clone()).await {
                Ok(_) => info!("Started service: {}", service_name),
                Err(e) => error!("Failed to start service {}: {}", service_name, e),
            }
        }

        // Wait for all services to be ready before starting the routing loop
        info!("Waiting for all services to be synchronized...");
        for (name, service) in &self.services {
            let svc = service.lock().await;
            if let Err(e) = svc.wait_until_ready().await {
                error!("Service {} failed to become ready: {}", name, e);
            }
        }
        info!("All services ready. Starting message routing.");

        let services = self.services.clone();
        let config = self.config.clone();
        let store = self.store.clone();

        // Message routing loop - blocks until cancelled or rx closed
        while let Some(event) = event_rx.recv().await {
            // Deduplication: Check if we've already processed this message
            if let ServiceEvent::NewMessage(ref msg) = event {
                match self
                    .store
                    .exists(&msg.source_service, &msg.source_channel, &msg.source_id)
                {
                    Ok(true) => {
                        warn!(
                            "Duplicate message ignored: {}:{}:{}",
                            msg.source_service, msg.source_channel, msg.source_id
                        );
                        continue;
                    }
                    Err(e) => {
                        error!("Failed to check for duplicate message: {}", e);
                        // Proceed cautiously
                    }
                    _ => {}
                }
            }

            if let Err(e) = self.router.route(event, &services, &config, &store).await {
                error!("Router error: {}", e);
            }
        }

        Ok(())
    }

    /// Route a message from one service to all other services in the same bridge
    /// Delegates to the event router which uses the dispatcher
    async fn route_message(&self, msg: ServiceMessage) {
        // Deduplication: Check if we've already processed this message
        match self
            .store
            .exists(&msg.source_service, &msg.source_channel, &msg.source_id)
        {
            Ok(true) => {
                warn!(
                    "Duplicate message ignored: {}:{}:{}",
                    msg.source_service, msg.source_channel, msg.source_id
                );
                return;
            }
            Err(e) => {
                error!("Failed to check for duplicate message: {}", e);
                // Proceed cautiously
            }
            _ => {}
        }

        // Use the event router to handle the message
        let services = self.services.clone();
        let config = self.config.clone();
        let store = self.store.clone();

        if let Err(e) = self
            .router
            .route(ServiceEvent::NewMessage(msg), &services, &config, &store)
            .await
        {
            error!("Router error: {}", e);
        }
    }

    /// Graceful shutdown
    pub async fn shutdown(&self) {
        info!("Shutting down bridge...");
    }
}
