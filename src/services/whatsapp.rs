use async_trait::async_trait;
use anyhow::Result;
use log::{info, error};
use std::sync::Arc;
use tokio::sync::mpsc;
use whatsapp_rust::{
    bot::Bot,
    store::SqliteStore,
};
use wacore::types::events::Event as WaEvent;
use whatsapp_rust::proto_helpers::MessageExt;
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
use whatsapp_rust_ureq_http_client::UreqHttpClient;
use waproto::whatsapp as wa;
use waproto::whatsapp::MessageKey;
use wacore_binary::jid::Jid;
use std::str::FromStr;

use crate::services::{Service, ServiceMessage, ServiceEvent};
use crate::config::WhatsAppServiceConfig;

pub struct WhatsAppService {
    name: String,
    config: WhatsAppServiceConfig,
    client: Option<Arc<whatsapp_rust::Client>>,
}

impl WhatsAppService {
    pub fn new(name: String, config: WhatsAppServiceConfig) -> Self {
        Self {
            name,
            config,
            client: None,
        }
    }
}

#[async_trait]
impl Service for WhatsAppService {
    async fn connect(&mut self) -> Result<()> {
        let database_path = self.config.session_path.clone().unwrap_or_else(|| "whatsapp.db".to_string());
        
        // Ensure directory exists
        if let Some(parent) = std::path::Path::new(&database_path).parent() {
            if !parent.as_os_str().is_empty() {
                let _ = std::fs::create_dir_all(parent);
            }
        }
        
        let backend = SqliteStore::new(&database_path).await?;
        let backend_arc = Arc::new(backend); // Keep Arc for bot builder
        let transport = TokioWebSocketTransportFactory::new();
        let http = UreqHttpClient::new();

        let bot = Bot::builder()
            .with_backend(backend_arc.clone()) // Use the Arc'd backend
            .with_transport_factory(transport)
            .with_http_client(http)
            .build()
            .await?;
        
        let client = bot.client();
        self.client = Some(client);
        
        info!("[WhatsApp:{}] Client created", self.name);
        Ok(())
    }

    async fn start(&mut self, tx: mpsc::Sender<ServiceEvent>) -> Result<()> {
        let transport = TokioWebSocketTransportFactory::new();
        let http = UreqHttpClient::new();

        // Client initialization is handled by Bot::builder below

        
        let database_path = self.config.session_path.clone().unwrap_or_else(|| "whatsapp.db".to_string());
        
        // Ensure directory exists
        if let Some(parent) = std::path::Path::new(&database_path).parent() {
            if !parent.as_os_str().is_empty() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    error!("Failed to create data directory for WhatsApp: {}", e);
                }
            }
        }

        let backend = SqliteStore::new(&database_path).await?;
        let backend = Arc::new(backend);
        
        let service_name = self.name.clone();
        let debug = self.config.debug;

        let mut bot = Bot::builder()
            .with_backend(backend)
            .with_transport_factory(transport)
            .with_http_client(http)
            .on_event(move |event, client| {
                let tx = tx.clone();
                let service_name = service_name.clone();
                
                async move {
                    match event {
                        WaEvent::Message(msg, info) => {
                            let is_own = info.source.is_from_me;
                            let sender_jid = info.source.sender.to_string();
                            let chat_jid = info.source.chat.to_string();
                            
                            // Determine display name
                            let display_name = if !info.push_name.is_empty() {
                                info.push_name.clone()
                            } else {
                                sender_jid.clone()
                            };

                            let text = msg.text_content().unwrap_or("").to_string();

                            if debug {
                                info!("[WhatsApp DEBUG] Msg in Chat: {} From: {} Content: {}", 
                                    chat_jid, sender_jid, text);
                            } else {
                                info!("[WhatsApp:{}] Received message in chat: {}", service_name, chat_jid);
                            }
                            
                            let mut attachments = Vec::new();
                                                        // Handle Image messages
                                if let Some(image_msg) = &msg.image_message {
                                    if debug { info!("[WhatsApp DEBUG] Received image message"); }
                                    
                                    // Dereference the Box to get the inner ImageMessage which implements Downloadable
                                    match client.download(&**image_msg).await {
                                        Ok(data) => {
                                            attachments.push(crate::services::Attachment {
                                                filename: "image.jpg".to_string(), 
                                                mime_type: image_msg.mimetype.clone().unwrap_or("image/jpeg".to_string()),
                                                data,
                                            });
                                        },
                                        Err(e) => error!("[WhatsApp] Failed to download media: {}", e),
                                    }
                                }
                                
                                // Handle Reactions
                                if let Some(reaction) = &msg.reaction_message {
                                    if let Some(key) = &reaction.key {
                                        if debug {
                                            info!("[WhatsApp:{}] Received reaction message: {:?}", service_name, reaction);
                                        }
                                        if let Some(target_id) = &key.id {
                                             let reaction_event = crate::services::ServiceReaction {
                                                 source_service: service_name.clone(),
                                                 source_channel: chat_jid.clone(),
                                                 source_message_id: target_id.clone(),
                                             _sender: display_name.clone(),
                                             emoji: reaction.text.clone().unwrap_or_default(),
                                             is_own,
                                         };
                                             
                                             if let Err(e) = tx.send(ServiceEvent::NewReaction(reaction_event)).await {
                                                 error!("Failed to send reaction: {}", e);
                                             }
                                             // Return early if it's just a reaction? 
                                             // WhatsApp messages usually contain one thing.
                                             return; 
                                        }
                                    }
                                }

                                // Only send if we have text OR attachments
                                if !text.is_empty() || !attachments.is_empty() {
                                    let service_msg = ServiceMessage {
                                        sender: display_name,
                                        sender_id: sender_jid,
                                        content: text,
                                        attachments,
                                        source_service: service_name.clone(),
                                        source_channel: chat_jid,
                                        source_id: info.id.to_string(),
                                        is_own,
                                    };
                                    
                                    if let Err(e) = tx.send(ServiceEvent::NewMessage(service_msg)).await {
                                        error!("Failed to send internal message: {}", e);
                                    }
                                }
                            }
                            WaEvent::PairingQrCode { code, .. } => {
                                info!("[WhatsApp:{}] Scan this QR code to link:", service_name);
                                qr2term::print_qr(&code).unwrap();
                            }
                            WaEvent::Connected(_) => info!("[WhatsApp:{}] Connected!", service_name),
                            _ => {}
                        }
                    }
                })
                .build()
                .await?;
            
            let client = bot.client();
            self.client = Some(client);
            
            // Run the bot in the background
            tokio::spawn(async move {
                if let Ok(handle) = bot.run().await {
                    let _ = handle.await;
                }
            });
    
            Ok(())
        }
    
        async fn send_message(&self, channel: &str, message: &ServiceMessage) -> Result<String> {
            let client = self.client.as_ref()
                .ok_or_else(|| anyhow::anyhow!("WhatsApp client not connected"))?;
            
            let jid = Jid::from_str(channel)?;
            let mut last_id = "unknown".to_string();

            // 1. Send text if present
            // 1. Send text if present AND no attachments (otherwise text goes in caption)
            if !message.content.is_empty() && message.attachments.is_empty() {
                let text = if message.sender.is_empty() {
                    message.content.clone()
                } else {
                    format!("*{}*: {}", message.sender, message.content)
                };
                let wa_message = wa::Message {
                    extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                        text: Some(text),
                        ..Default::default()
                    })),
                    ..Default::default()
                };
                let resp = client.send_message(jid.clone(), wa_message).await?;
                last_id = resp; 
            }
            
            // 2. Send attachments
            for attachment in &message.attachments {
                // Determine type (image or generic document)
                let is_image = attachment.mime_type.starts_with("image/");
                
                // Upload media
                // Fix: client.upload takes data explicitly (Move) and MediaType enum
                // Using wacore::download::MediaType based on compiler suggestion
                let media_type = if is_image { wacore::download::MediaType::Image } else { wacore::download::MediaType::Document };
                
                let upload = client.upload(
                    attachment.data.clone(), 
                    media_type
                ).await?;
                
                let wa_message = if is_image {
                    wa::Message {
                        image_message: Some(Box::new(wa::message::ImageMessage {
                            url: Some(upload.url),
                            direct_path: Some(upload.direct_path),
                            media_key: Some(upload.media_key),
                            mimetype: Some(attachment.mime_type.clone()),
                            file_enc_sha256: Some(upload.file_enc_sha256),
                            file_sha256: Some(upload.file_sha256),
                            file_length: Some(attachment.data.len() as u64),
                            caption: Some(match (message.sender.is_empty(), message.content.is_empty()) {
                                (true, true) => "".to_string(),
                                (true, false) => message.content.clone(),
                                (false, true) => format!("*{}*", message.sender),
                                (false, false) => format!("*{}*: {}", message.sender, message.content),
                            }),
                            ..Default::default()
                        })),
                        ..Default::default()
                    }
                } else {
                    wa::Message {
                        document_message: Some(Box::new(wa::message::DocumentMessage {
                            url: Some(upload.url),
                            direct_path: Some(upload.direct_path),
                            media_key: Some(upload.media_key),
                            mimetype: Some(attachment.mime_type.clone()),
                            file_enc_sha256: Some(upload.file_enc_sha256),
                            file_sha256: Some(upload.file_sha256),
                            file_length: Some(attachment.data.len() as u64),
                            title: Some(attachment.filename.clone()),
                            file_name: Some(attachment.filename.clone()),
                            ..Default::default()
                        })),
                        ..Default::default()
                    }
                };
                
                let resp = client.send_message(jid.clone(), wa_message).await?;
                last_id = resp;
            }
            
            if self.config.debug {
                info!("[WhatsApp DEBUG] Successfully sent message(s) to {}", channel);
            }
            
            Ok(last_id)
        }

    async fn edit_message(&self, _channel: &str, _message_id: &str, _new_content: &str) -> Result<()> {
        // WhatsApp editing not fully supported in this bridge version yet
        Ok(())
    }

    async fn react_to_message(&self, channel: &str, message_id: &str, emoji: &str) -> Result<()> {
         let client = self.client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WhatsApp client not connected"))?;
            
         let jid = Jid::from_str(channel)?;
         
         let wa_message = wa::Message {
             reaction_message: Some(wa::message::ReactionMessage {
                 key: Some(MessageKey {
                     remote_jid: Some(channel.to_string()),
                     from_me: Some(true), 
                     id: Some(message_id.to_string()),
                     ..Default::default()
                 }),
                 text: Some(emoji.to_string()),
                 sender_timestamp_ms: Some(std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH)?.as_millis() as i64),
                 ..Default::default()
             }),
             ..Default::default()
         };
         
         client.send_message(jid, wa_message).await?;
         Ok(())
    }

    fn service_name(&self) -> &str {
        &self.name
    }

    fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("[WhatsApp:{}] Disconnecting", self.name);
        // WhatsApp client cleanup handled by drop
        Ok(())
    }

    async fn wait_until_ready(&self) -> Result<()> {
        info!("[WhatsApp:{}] Waiting for connection to stabilize...", self.name);
        // WhatsApp doesn't have a simple "is synced" flag we can easily poll, 
        // but we can check if client exists and give it a moment.
        if self.client.is_some() {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            info!("[WhatsApp:{}] Ready!", self.name);
        }
        Ok(())
    }
}
