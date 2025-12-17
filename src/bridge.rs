use anyhow::Result;
use log::{info, error, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::services::{Service, ServiceMessage};

/// Manages multiple bridges and routes messages between services
pub struct BridgeCoordinator {
    config: Config,
    services: HashMap<String, Arc<tokio::sync::Mutex<Box<dyn Service>>>>,
}

impl BridgeCoordinator {
    pub fn new(
        config: Config,
        services: HashMap<String, Arc<tokio::sync::Mutex<Box<dyn Service>>>>,
    ) -> Self {
        Self { config, services }
    }

    /// Start all bridges and begin routing messages
    pub async fn start(self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel::<ServiceMessage>(100);

        // Start all services
        for (service_name, service) in &self.services {
            let mut svc = service.lock().await;
            match svc.start(tx.clone()).await {
                Ok(_) => info!("Started service: {}", service_name),
                Err(e) => error!("Failed to start service {}: {}", service_name, e),
            }
        }

        let services = self.services.clone();
        let config = self.config.clone();

        // Message routing task
        tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                Self::route_message(msg, &services, &config).await;
            }
        });

        Ok(())
    }

    /// Route a message from one service to all other services in the same bridge
    async fn route_message(
        msg: ServiceMessage,
        services: &HashMap<String, Arc<tokio::sync::Mutex<Box<dyn Service>>>>,
        config: &Config,
    ) {
        info!(
            "Processing message from {}:{} (sender: {})",
            msg.source_service, msg.source_channel, msg.sender
        );

        let mut matched_any_bridge = false;

        // Find which bridge(s) this message belongs to
        for (bridge_name, channels) in &config.bridges {
            // Find the source channel config
            let source_channel = channels.iter().find(|ch| {
                // Debug log matching attempts (info level for now to debug)
                // info!("Checking bridge '{}': {}/{} vs msg {}/{}", 
                //    bridge_name, ch.service, ch.channel, msg.source_service, msg.source_channel);
                ch.service == msg.source_service && ch.channel == msg.source_channel
            });

            if source_channel.is_none() {
                // Determine if we should log this rejection (debug only)
                // We don't have access to global debug flag here easily without config, 
                // but unlikely to spam if it's not matching any bridge.
                // For now, let's log valuable debug info if we can't find a bridge.
                // Note: This matches EVERY bridge loop, so it would spam if we log "not found in bridge X".
                continue;
            }
            
            matched_any_bridge = true;
            let source_config = source_channel.unwrap();

            // Check if we should bridge own messages
            // We need to look up the service instance to check its config
            let should_bridge_own = if let Some(service) = services.get(msg.source_service.as_str()) {
                 let svc = service.lock().await;
                 svc.should_bridge_own_messages()
            } else {
                false
            };

            if msg.sender_id.contains(&msg.source_service) && !should_bridge_own {
                continue;
            }

            // Apply alias if configured
            let display_name = if source_config.display_names {
                source_config
                    .aliases
                    .get(&msg.sender_id)
                    .cloned()
                    .unwrap_or_else(|| msg.sender.clone())
            } else {
                msg.sender.clone()
            };

            // Create modified message with alias applied
            let modified_msg = ServiceMessage {
                sender: display_name,
                ..msg.clone()
            };

            // Forward to all other channels in this bridge
            for target_channel in channels {
                // Skip the source channel
                let same_service = target_channel.service == msg.source_service;
                let same_channel = target_channel.channel == msg.source_channel;
                
                if same_service && same_channel {
                    continue;
                }

                // Get the target service
                if let Some(service_lock) = services.get(target_channel.service.as_str()) {
                    let service = service_lock.lock().await;
                    
                    // Create outgoing message, optionally stripping media
                    let mut outgoing_msg = modified_msg.clone();
                    if !target_channel.enable_media {
                        outgoing_msg.attachments.clear();
                    }
                    
                    info!("[Bridge:{}] Forwarded message from {}:{} to {}:{}", 
                        bridge_name, 
                        msg.source_service, msg.source_channel,
                        target_channel.service, target_channel.channel
                    );

                    // Send the message
                    if let Err(e) = service.send_message(&target_channel.channel, &outgoing_msg).await {
                        error!("[Bridge:{}] Failed to forward to {}:{}: {}", 
                            bridge_name, target_channel.service, target_channel.channel, e);
                    }
                } else {
                    warn!(
                        "[Bridge:{}] Service '{}' not found",
                        bridge_name, target_channel.service
                    );
                }
            }
        }
        
        if !matched_any_bridge {
            warn!("[Bridge] Message dropped: No bridge found for Service: '{}', Channel: '{}' (Sender: {})", 
                msg.source_service, msg.source_channel, msg.sender);
        }
    }
}
