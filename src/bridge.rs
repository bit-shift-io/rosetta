use anyhow::Result;
use log::{info, error, warn};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;

use crate::config::Config;
use crate::services::{Service, ServiceMessage, ServiceEvent, ServiceUpdate};
use crate::persistence::MessageStore;

/// Manages multiple bridges and routes messages between services
pub struct BridgeCoordinator {
    config: Config,
    services: HashMap<String, Arc<tokio::sync::Mutex<Box<dyn Service>>>>,
    store: Arc<MessageStore>,
}

impl BridgeCoordinator {
    pub fn new(
        config: Config,
        services: HashMap<String, Arc<tokio::sync::Mutex<Box<dyn Service>>>>,
    ) -> Result<Self> {
        let store = Arc::new(MessageStore::new("data/message_history.db")?);
        Ok(Self { config, services, store })
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
             match event {
                 ServiceEvent::NewMessage(msg) => {
                     Self::route_message(msg, &services, &config, &store).await;
                 },
                 ServiceEvent::UpdateMessage(update) => {
                     Self::handle_edit(update, &services, &config, &store).await;
                 },
                 ServiceEvent::NewReaction(reaction) => {
                     self.handle_reaction(reaction, &config, &store).await;
                 }
             }
        }
        
        Ok(())
    }

    /// Route a message from one service to all other services in the same bridge
    async fn route_message(
        msg: ServiceMessage,
        services: &HashMap<String, Arc<tokio::sync::Mutex<Box<dyn Service>>>>,
        config: &Config,
        store: &MessageStore,
    ) {
        info!(
            "Processing message from {}:{} (sender: {})",
            msg.source_service, msg.source_channel, msg.sender
        );

        // Deduplication: Check if we've already processed this message
        match store.exists(&msg.source_service, &msg.source_channel, &msg.source_id) {
            Ok(true) => {
                warn!("Duplicate message ignored: {}:{}:{}", msg.source_service, msg.source_channel, msg.source_id);
                return;
            },
            Err(e) => {
                error!("Failed to check for duplicate message: {}", e);
                // Proceed cautiously
            },
            _ => {}
        }
        
        let mut matched_any_bridge = false;

        // Find which bridge(s) this message belongs to
        for (bridge_name, channels) in &config.bridges {
            // Find the source channel config
            let source_channel = channels.iter().find(|ch| {
                ch.service == msg.source_service && ch.channel == msg.source_channel
            });

            if let Some(source_config) = source_channel {
                matched_any_bridge = true;

                if msg.is_own && !source_config.bridge_own_messages {
                    info!("[Bridge] Skipping own message from {}:{} in bridge '{}' as bridging is disabled for this channel.", 
                        msg.source_service, msg.source_channel, bridge_name);
                    continue;
                }
            } else {
                continue;
            }
            
            let source_config = source_channel.unwrap();

            // Handle .status command
            if msg.content.trim() == ".status" {
                info!("Status command received in bridge '{}'", bridge_name);
                
                let mut status_lines = Vec::new();
                status_lines.push(format!("**Bridge Status: {}**", bridge_name));
                
                // 1. Service Health Section
                status_lines.push("\n**Services**".to_string());
                let mut checked_services = Vec::new(); 
                for ch in channels {
                    if !checked_services.contains(&ch.service) {
                         if let Some(svc_lock) = services.get(ch.service.as_str()) {
                             let svc = svc_lock.lock().await;
                             let status = if svc.is_connected() { "✅ Connected" } else { "❌ Disconnected" };
                             status_lines.push(format!("* **{}**: {}", ch.service, status));
                         } else {
                             status_lines.push(format!("* **{}**: ❓ Service Not Found", ch.service));
                         }
                         checked_services.push(ch.service.clone());
                    }
                }
                
                // 2. Room Members Section
                status_lines.push("\n**Rooms**".to_string());
                for ch in channels {
                    if let Some(svc_lock) = services.get(ch.service.as_str()) {
                        let svc = svc_lock.lock().await;
                        status_lines.push(format!("\n**{}:{}**", ch.service, ch.channel));
                        
                        match svc.get_room_members(&ch.channel).await {
                            Ok(members) => {
                                if members.is_empty() {
                                    status_lines.push("* *(No members found or not supported)*".to_string());
                                } else {
                                    for member in members {
                                        status_lines.push(format!("* **{}**", member));
                                    }
                                }
                            },
                            Err(e) => {
                                status_lines.push(format!("* *Error fetching members: {}*", e));
                            }
                        }
                    }
                }
                
                let status_msg_content = status_lines.join("\n");
                let status_msg = ServiceMessage {
                    sender: "System".to_string(),
                    sender_id: "system".to_string(),
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
                         error!("Failed to send status to {}:{}: {}", msg.source_service, msg.source_channel, e);
                    }
                }
                
                // Skip further processing
                continue;
            }

            // Resolve sender name (applying aliases if configured)
            let display_name = source_config
                .aliases
                .get(&msg.sender_id)
                .cloned()
                .unwrap_or_else(|| msg.sender.clone());

            // Create modified message with alias applied (or suppressed)
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
                    
                    // Create outgoing message, optionally stripping media OR suppressing name
                    let mut outgoing_msg = modified_msg.clone();
                    if !target_channel.enable_media {
                        outgoing_msg.attachments.clear();
                    }
                    if !target_channel.display_names {
                        outgoing_msg.sender = "".to_string();
                    }
                    
                    info!("[Bridge:{}] Forwarded message from {}:{} to {}:{}", 
                        bridge_name, 
                        msg.source_service, msg.source_channel,
                        target_channel.service, target_channel.channel
                    );

                    // Send the message
                    match service.send_message(&target_channel.channel, &outgoing_msg).await {
                         Ok(dest_id) => {
                                // Save mapping for future edits
                                if let Err(e) = store.save_mapping(
                                    &msg.source_service,
                                    &msg.source_channel,
                                    &msg.source_id,
                                    &target_channel.service,
                                    &target_channel.channel,
                                    &dest_id
                                ) {
                                    error!("Failed to save message mapping: {}", e);
                                }
                         },
                         Err(e) => {
                            error!("[Bridge:{}] Failed to forward to {}:{}: {}", 
                                bridge_name, target_channel.service, target_channel.channel, e);
                         }
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

    
    async fn handle_edit(
        update: ServiceUpdate,
        services: &HashMap<String, Arc<tokio::sync::Mutex<Box<dyn Service>>>>,
        config: &Config,
        store: &MessageStore,
    ) {
         info!("Processing edit from {}:{}:{}", update.source_service, update.source_channel, update.source_id);
         
         // Check if we should bridge own messages/edits
         if update.is_own {
             let mut allow = false;
             // Find config definition
             for (bridge_name, channels) in &config.bridges {
                 if let Some(source_config) = channels.iter().find(|ch| 
                     ch.service == update.source_service && ch.channel == update.source_channel
                 ) {
                     if source_config.bridge_own_messages {
                         allow = true;
                         break;
                     } else {
                         info!("[Bridge] Skipping own edit from {}:{} in bridge '{}'", 
                             update.source_service, update.source_channel, bridge_name);
                     }
                 }
             }
             
             if !allow {
                 return;
             }
         }

         // Find all related messages for this source/dest
         match store.get_associated_mappings(&update.source_service, &update.source_channel, &update.source_id) {
             Ok(mappings) => {
                 for (dest_service, dest_channel, dest_id) in mappings {
                     if let Some(service_lock) = services.get(&dest_service) {
                         let service = service_lock.lock().await;
                         
                         if let Err(e) = service.edit_message(&dest_channel, &dest_id, &update.new_content).await {
                                error!("Failed to edit message in {}:{}: {}", dest_service, dest_channel, e);
                         } else {
                                info!("Edited message in {}:{}", dest_service, dest_channel);
                         }
                     }
                 }
             },
             Err(e) => {
                 error!("Failed to look up message mappings: {}", e);
             }
         }
    }

    pub async fn handle_reaction(
        &self,
        reaction: crate::services::ServiceReaction,
        config: &Config,
        store: &MessageStore,
    ) {
        // Find which bridge(s) this reaction belongs to and check if we should bridge it
        for (bridge_name, channels) in config.bridges.iter() {
            let source_channel = channels.iter().find(|ch| {
                ch.service == reaction.source_service && ch.channel == reaction.source_channel
            });

            if let Some(source_config) = source_channel {
                if reaction.is_own && !source_config.bridge_own_messages {
                    info!("[Bridge] Skipping own reaction from {}:{} in bridge '{}' as bridging is disabled.", 
                        reaction.source_service, reaction.source_channel, bridge_name);
                    return;
                }
            }
        }

        // Find all related messages for this source/dest
        match store.get_associated_mappings(&reaction.source_service, &reaction.source_channel, &reaction.source_message_id) {
            Ok(mappings) => {
                for (dest_service, dest_channel, dest_id) in mappings {
                    if let Some(service_lock) = self.services.get(&dest_service) {
                        let service = service_lock.lock().await;
                        info!("[Bridge] Forwarding reaction '{}' to {}:{}", reaction.emoji, dest_service, dest_channel);
                        
                        if let Err(e) = service.react_to_message(&dest_channel, &dest_id, &reaction.emoji).await {
                             warn!("[Bridge] Failed to forward reaction to {}: {}", dest_service, e);
                        }
                    }
                }
            },
            Err(e) => {
                warn!("[Bridge] Reaction ignored (message lookup failed): {}", e);
            }
        }
    }

    /// Graceful shutdown
    pub async fn shutdown(&self) {
        info!("Shutting down bridge...");
    }
}
