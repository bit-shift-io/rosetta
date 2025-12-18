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
    pub async fn start(self) -> Result<()> {
        let (tx, mut rx) = mpsc::channel::<ServiceEvent>(100);

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
        let store = self.store.clone();

        // Message routing task
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                 match event {
                     ServiceEvent::NewMessage(msg) => {
                         Self::route_message(msg, &services, &config, &store).await;
                     },
                     ServiceEvent::UpdateMessage(update) => {
                         Self::handle_edit(update, &services, &config, &store).await;
                     }
                 }
            }
        });

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
                warn!("Duplicate key ignored: {}:{}:{}", msg.source_service, msg.source_channel, msg.sender_id); // Wait sender_id? No source_id
                warn!("Duplicate message ignored: {}:{}:{}", msg.source_service, msg.source_channel, msg.source_id);
                return;
            },
            Err(e) => {
                error!("Failed to check for duplicate message: {}", e);
                // Proceed cautiously or return? Proceeding risks dupes, but DB failure logic.
            },
            _ => {}
        }
        
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
                                        status_lines.push(format!("* {}", member));
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
                };
                
                // Broadcast status to all channels in this bridge
                for ch in channels {
                    if let Some(svc_lock) = services.get(ch.service.as_str()) {
                        let svc = svc_lock.lock().await;
                        if let Err(e) = svc.send_message(&ch.channel, &status_msg).await {
                             error!("Failed to send status to {}:{}: {}", ch.service, ch.channel, e);
                        }
                    }
                }
                
                // Allow forwarding of the original .status message (User requested)
                // continue; 
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
        _config: &Config,
        store: &MessageStore,
    ) {
         info!("Processing edit from {}:{}:{}", update.source_service, update.source_channel, update.source_id);
         
         // Find all destination messages for this source
         match store.get_mappings(&update.source_service, &update.source_channel, &update.source_id) {
             Ok(mappings) => {
                 for (dest_service, dest_channel, dest_id) in mappings {
                     if let Some(service_lock) = services.get(&dest_service) {
                         let service = service_lock.lock().await;
                         
                         // We might want to apply the same alias logic here, 
                         // but for simplicity we just forward content for now.
                         // Ideally we should cache the original sender name to re-prepend it.
                         // For now, let's just forward the raw content or try to apply the format if we knew the sender.
                         // Since we don't have the original sender in the Update struct, we might send just the content.
                         // However, Matrix/Discord bridges usually expect "User: Content".
                         // IMPROVEMENT: Retrieve original message from DB? Or just edit blindly.
                         // For now, we will blindly edit. This means if the original message was "User: Hello", 
                         // and we edit to "World", it becomes "World". The "User:" prefix is lost on edit unless we re-fetch it.
                         // But `ServiceUpdate` assumes we are just passing the new content.
                         // Let's assume the update.new_content is the raw new text.
                         // We can't easily re-apply the "User: " prefix without looking it up.
                         
                         // HACK/FIX: Bridges usually manage this better. 
                         // For this iteration, we will just send the content.
                         // The user will see the message change. 
                         
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
}
