use crate::config::Config;
use crate::services::{Service, ServiceMessage};
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct StatusHandler;

impl StatusHandler {
    pub fn new() -> Self {
        Self
    }

    pub async fn handle(
        &self,
        msg: ServiceMessage,
        services: &HashMap<String, Arc<Mutex<Box<dyn Service>>>>,
        config: &Config,
    ) -> Result<()> {
        log::info!("Status command received in {}", msg.source_channel);

        // Find which bridge this belongs to
        let bridge_name = config
            .bridges
            .iter()
            .find(|(_, channels)| {
                channels
                    .iter()
                    .any(|ch| ch.service == msg.source_service && ch.channel == msg.source_channel)
            })
            .map(|(name, _)| name.as_str())
            .unwrap_or("unknown");

        let mut status_lines = Vec::new();
        status_lines.push(format!("**Bridge Status: {}**", bridge_name));

        // 1. Service Health Section
        status_lines.push("\n**Services**".to_string());
        let mut checked_services = Vec::new();
        for (_, channels) in &config.bridges {
            for ch in channels {
                if !checked_services.contains(&ch.service) {
                    if let Some(svc_lock) = services.get(ch.service.as_str()) {
                        let svc = svc_lock.lock().await;
                        let status = if svc.is_connected() {
                            "✅ Connected"
                        } else {
                            "❌ Disconnected"
                        };
                        status_lines.push(format!("* **{}**: {}", ch.service, status));
                    } else {
                        status_lines.push(format!("* **{}**: ❓ Service Not Found", ch.service));
                    }
                    checked_services.push(ch.service.clone());
                }
            }
        }

        // 2. Room Members Section
        status_lines.push("\n**Rooms**".to_string());
        for (_, channels) in &config.bridges {
            for ch in channels {
                if let Some(svc_lock) = services.get(ch.service.as_str()) {
                    let svc = svc_lock.lock().await;
                    status_lines.push(format!("\n**{}:{}**", ch.service, ch.channel));

                    match svc.get_room_members(&ch.channel).await {
                        Ok(members) => {
                            if members.is_empty() {
                                status_lines
                                    .push("* *(No members found or not supported)*".to_string());
                            } else {
                                for member in members {
                                    status_lines.push(format!("* **{}**", member));
                                }
                            }
                        }
                        Err(e) => {
                            status_lines.push(format!("* *Error fetching members: {}*", e));
                        }
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
                log::error!(
                    "Failed to send status to {}:{}: {}",
                    msg.source_service,
                    msg.source_channel,
                    e
                );
            }
        }

        Ok(())
    }
}
