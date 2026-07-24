use crate::bridge::alias_resolver::AliasResolver;
use crate::bridge::formatter::MessageFormatter;
use crate::bridge::matcher::BridgeMatcher;
use crate::bridge::media::MediaHandler;
use crate::config::Config;
use crate::persistence::MessageStore;
use crate::services::ServiceMessage;
use crate::services::traits::MandatoryService;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct MessageDispatcher {
    matcher: BridgeMatcher,
    alias_resolver: AliasResolver,
    media_handler: MediaHandler,
    formatters: HashMap<String, Box<dyn MessageFormatter + Send + Sync>>,
    store: Arc<MessageStore>,
}

impl MessageDispatcher {
    pub fn new(
        matcher: BridgeMatcher,
        alias_resolver: AliasResolver,
        media_handler: MediaHandler,
        formatters: HashMap<String, Box<dyn MessageFormatter + Send + Sync>>,
        store: Arc<MessageStore>,
    ) -> Self {
        Self {
            matcher,
            alias_resolver,
            media_handler,
            formatters,
            store,
        }
    }

    pub async fn dispatch(
        &self,
        msg: ServiceMessage,
        services: &HashMap<String, Arc<Mutex<Box<dyn MandatoryService>>>>,
        config: &Config,
    ) -> Result<()> {
        // Deduplication: Check if we've already processed this message
        match self
            .store
            .exists(&msg.source_service, &msg.source_channel, &msg.source_id)
        {
            Ok(true) => {
                log::debug!(
                    "[Dispatcher] Duplicate message ignored: {}:{}:{}",
                    msg.source_service,
                    msg.source_channel,
                    msg.source_id
                );
                return Ok(());
            }
            Err(e) => {
                log::error!("Failed to check for duplicate message: {}", e);
                // Proceed cautiously
            }
            _ => {}
        }

        // Find target channels using BridgeMatcher
        let target_channels =
            self.matcher
                .find_targets(&msg.source_service, &msg.source_channel, config);

        if target_channels.is_empty() {
            log::warn!(
                "[Dispatcher] Message dropped: No bridge found for Service: '{}', Channel: '{}' (Sender: {})",
                msg.source_service,
                msg.source_channel,
                msg.sender
            );
            return Ok(());
        }

        // Find the source channel config for bridge_own_messages check
        let source_config = config
            .bridges
            .values()
            .flatten()
            .find(|ch| ch.service == msg.source_service && ch.channel == msg.source_channel);

        if let Some(source_config) = source_config {
            if msg.is_own && !source_config.bridge_own_messages {
                log::info!(
                    "[Dispatcher] Skipping own message from {}:{} as bridging is disabled for this channel.",
                    msg.source_service,
                    msg.source_channel
                );
                return Ok(());
            }
        }

        // Resolve sender name (applying aliases if configured) using AliasResolver
        let source_config = config
            .bridges
            .values()
            .flatten()
            .find(|ch| ch.service == msg.source_service && ch.channel == msg.source_channel);

        let display_name = if let Some(source_config) = source_config {
            self.alias_resolver
                .resolve(&msg.sender_id, source_config, &msg.sender)
        } else {
            msg.sender.clone()
        };

        // Create modified message with alias applied (or suppressed)
        let modified_msg = ServiceMessage {
            sender: display_name,
            ..msg.clone()
        };

        // Forward to all target channels
        for target_channel in target_channels {
            // Get the target service
            if let Some(service_lock) = services.get(target_channel.service.as_str()) {
                let service = service_lock.lock().await;

                // Create outgoing message, applying media policy and display names via MediaHandler
                let mut outgoing_msg = modified_msg.clone();
                self.media_handler
                    .process(&mut outgoing_msg, target_channel);

                // Apply target service's formatter if available
                if let Some(formatter) = self.formatters.get(&target_channel.service) {
                    // Format text message
                    if outgoing_msg.content.is_empty() && outgoing_msg.attachments.is_empty() {
                        // Empty message, keep as is
                    } else {
                        let formatted_text = formatter.format_text(
                            &outgoing_msg.sender,
                            &outgoing_msg.content,
                            outgoing_msg.is_own,
                        );
                        outgoing_msg.content = formatted_text;
                    }
                }

                log::info!(
                    "[Dispatcher] Forwarded message from {}:{} to {}:{}",
                    msg.source_service,
                    msg.source_channel,
                    target_channel.service,
                    target_channel.channel
                );

                // Send the message
                match service
                    .send_message(&target_channel.channel, &outgoing_msg)
                    .await
                {
                    Ok(dest_id) => {
                        // Save mapping for future edits
                        if let Err(e) = self.store.save_mapping(
                            &msg.source_service,
                            &msg.source_channel,
                            &msg.source_id,
                            &target_channel.service,
                            &target_channel.channel,
                            &dest_id,
                        ) {
                            log::error!("Failed to save message mapping: {}", e);
                        }
                    }
                    Err(e) => {
                        log::error!(
                            "[Dispatcher] Failed to forward to {}:{}: {}",
                            target_channel.service,
                            target_channel.channel,
                            e
                        );
                    }
                }
            } else {
                log::warn!(
                    "[Dispatcher] Service '{}' not found",
                    target_channel.service
                );
            }
        }

        Ok(())
    }
}
