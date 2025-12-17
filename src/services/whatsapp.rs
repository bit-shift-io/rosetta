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
use wacore_binary::jid::Jid;
use std::str::FromStr;

use crate::services::{Service, ServiceMessage};
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
        let backend = SqliteStore::new(&self.config.database_path).await?;
        let backend = Arc::new(backend);
        let transport = TokioWebSocketTransportFactory::new();
        let http = UreqHttpClient::new();

        let bot = Bot::builder()
            .with_backend(backend)
            .with_transport_factory(transport)
            .with_http_client(http)
            .build()
            .await?;
        
        let client = bot.client();
        self.client = Some(client);
        
        info!("[WhatsApp:{}] Client created", self.name);
        Ok(())
    }

    async fn start(&mut self, tx: mpsc::Sender<ServiceMessage>) -> Result<()> {
        let backend = SqliteStore::new(&self.config.database_path).await?;
        let backend = Arc::new(backend);
        let transport = TokioWebSocketTransportFactory::new();
        let http = UreqHttpClient::new();
        
        let service_name = self.name.clone();
        let debug = self.config.debug;
        
        let mut bot = Bot::builder()
            .with_backend(backend)
            .with_transport_factory(transport)
            .with_http_client(http)
            .on_event(move |event, _client| {
                let tx = tx.clone();
                let service_name = service_name.clone();
                
                async move {
                    match event {
                        WaEvent::Message(msg, info) => {
                            let sender_jid = info.source.sender.to_string();
                            let chat_jid = info.source.chat.to_string();
                            
                            if debug {
                                let text_preview = msg.text_content().unwrap_or("<no text>");
                                info!("[WhatsApp DEBUG] Msg in Chat: {} From: {} (from_me={}) Content: {}", 
                                    chat_jid, sender_jid, info.source.is_from_me, text_preview);
                            } else {
                                info!("[WhatsApp:{}] Received message in chat: {}", service_name, chat_jid);
                            }
                            
                            if let Some(text) = msg.text_content() {
                                // Determine display name: push_name or sender JID
                                let display_name = if !info.push_name.is_empty() {
                                    info.push_name.clone()
                                } else {
                                    sender_jid.clone()
                                };
                                
                                let msg = ServiceMessage {
                                    sender: display_name,
                                    sender_id: sender_jid,
                                    content: text.to_string(),
                                    source_service: service_name.clone(),
                                    source_channel: chat_jid,
                                };
                                
                                if let Err(e) = tx.send(msg).await {
                                    error!("Failed to send internal message: {}", e);
                                }
                            } else if debug {
                                info!("[WhatsApp DEBUG] Message has no text content, skipping.");
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

    async fn send_message(&self, channel: &str, message: &ServiceMessage) -> Result<()> {
        let client = self.client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("WhatsApp client not connected"))?;
        
        let wa_message = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some(format!("{}: {}", message.sender, message.content)),
                ..Default::default()
            })),
            ..Default::default()
        };
        
        let jid = Jid::from_str(channel)?;
        
        client.send_message(jid, wa_message).await?;
        
        if self.config.debug {
            info!("[WhatsApp DEBUG] Successfully sent to {}", channel);
        }
        
        Ok(())
    }

    fn service_name(&self) -> &str {
        &self.name
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("[WhatsApp:{}] Disconnecting", self.name);
        // WhatsApp client cleanup handled by drop
        Ok(())
    }
}
