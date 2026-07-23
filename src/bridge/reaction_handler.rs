use crate::config::Config;
use crate::persistence::MessageStore;
use crate::services::ServiceReaction;
use crate::services::traits::MandatoryService;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct ReactionHandler;

impl ReactionHandler {
    pub fn new() -> Self {
        Self
    }

    pub async fn handle(
        &self,
        reaction: ServiceReaction,
        services: &HashMap<String, Arc<Mutex<Box<dyn MandatoryService>>>>,
        config: &Config,
        store: &MessageStore,
    ) -> Result<()> {
        log::info!(
            "Processing reaction from {}:{}:{}",
            reaction.source_service,
            reaction.source_channel,
            reaction.source_message_id
        );

        // Find which bridge(s) this reaction belongs to and check if we should bridge it
        for (_bridge_name, channels) in config.bridges.iter() {
            let source_channel = channels.iter().find(|ch| {
                ch.service == reaction.source_service && ch.channel == reaction.source_channel
            });

            if let Some(source_config) = source_channel {
                if reaction.is_own && !source_config.bridge_own_messages {
                    log::info!(
                        "[ReactionHandler] Skipping own reaction from {}:{} as bridging is disabled.",
                        reaction.source_service,
                        reaction.source_channel
                    );
                    return Ok(());
                }
            }
        }

        // Find all related messages for this source/dest
        match store.get_associated_mappings(
            &reaction.source_service,
            &reaction.source_channel,
            &reaction.source_message_id,
        ) {
            Ok(mappings) => {
                for (dest_service, dest_channel, dest_id) in mappings {
                    if let Some(service_lock) = services.get(&dest_service) {
                        let service = service_lock.lock().await;
                        log::info!(
                            "[ReactionHandler] Forwarding reaction '{}' to {}:{}",
                            reaction.emoji,
                            dest_service,
                            dest_channel
                        );

                        if let Err(e) = service
                            .react_to_message(&dest_channel, &dest_id, &reaction.emoji)
                            .await
                        {
                            log::warn!(
                                "[ReactionHandler] Failed to forward reaction to {}: {}",
                                dest_service,
                                e
                            );
                        }
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    "[ReactionHandler] Reaction ignored (message lookup failed): {}",
                    e
                );
            }
        }

        Ok(())
    }
}
