use crate::config::Config;
use crate::persistence::MessageStore;
use crate::services::{Service, ServiceUpdate};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct EditHandler;

impl EditHandler {
    pub fn new() -> Self {
        Self
    }

    pub async fn handle(
        &self,
        update: ServiceUpdate,
        services: &HashMap<String, Arc<Mutex<Box<dyn Service>>>>,
        config: &Config,
        store: &MessageStore,
    ) -> Result<()> {
        log::info!(
            "Processing edit from {}:{}:{}",
            update.source_service,
            update.source_channel,
            update.source_id
        );

        // Check if we should bridge own messages/edits
        if update.is_own {
            let mut allow = false;
            for (_bridge_name, channels) in &config.bridges {
                if let Some(source_config) = channels.iter().find(|ch| {
                    ch.service == update.source_service && ch.channel == update.source_channel
                }) {
                    if source_config.bridge_own_messages {
                        allow = true;
                        break;
                    } else {
                        log::info!(
                            "[EditHandler] Skipping own edit from {}:{} as bridging is disabled.",
                            update.source_service,
                            update.source_channel
                        );
                    }
                }
            }

            if !allow {
                return Ok(());
            }
        }

        // Find all related messages for this source/dest
        match store.get_associated_mappings(
            &update.source_service,
            &update.source_channel,
            &update.source_id,
        ) {
            Ok(mappings) => {
                for (dest_service, dest_channel, dest_id) in mappings {
                    if let Some(service_lock) = services.get(&dest_service) {
                        let mut service = service_lock.lock().await;

                        log::info!(
                            "[EditHandler] Forwarding edit to {}:{}",
                            dest_service,
                            dest_channel
                        );

                        if let Err(e) = service
                            .edit_message(&dest_channel, &dest_id, &update.new_content)
                            .await
                        {
                            log::error!(
                                "Failed to edit message in {}:{}: {}",
                                dest_service,
                                dest_channel,
                                e
                            );
                        } else {
                            log::info!("Edited message in {}:{}", dest_service, dest_channel);
                        }
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to look up message mappings: {}", e);
            }
        }

        Ok(())
    }
}
