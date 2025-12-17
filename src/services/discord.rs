use async_trait::async_trait;
use anyhow::Result;
use log::{info, error};
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::mpsc;
use serenity::{
    async_trait as serenity_async_trait,
    client::{Client, Context, EventHandler},
    model::{
        channel::Message,
        gateway::Ready,
        id::ChannelId,
    },
    prelude::*,
};

use crate::services::{Service, ServiceMessage};
use crate::config::DiscordServiceConfig;

/// Discord event handler
struct DiscordHandler {
    tx: mpsc::Sender<ServiceMessage>,
    service_name: String,
    debug: bool,
}

#[serenity_async_trait]
impl EventHandler for DiscordHandler {
    async fn message(&self, ctx: Context, msg: Message) {
        // Ignore bot messages
        if msg.author.bot {
            return;
        }

        // Try to get the guild-specific nickname, then global display name, then username
        let display_name = msg.author_nick(&ctx).await
            .or_else(|| msg.author.global_name.clone())
            .unwrap_or_else(|| msg.author.name.clone());

        if self.debug {
            info!("[Discord DEBUG] Message received from {} (id: {}): {}", display_name, msg.author.id, msg.content);
        }

        // Handle attachments
        let mut attachments = Vec::new();
        for attachment in &msg.attachments {
            if let Ok(response) = reqwest::get(&attachment.url).await {
                if let Ok(bytes) = response.bytes().await {
                    attachments.push(crate::services::Attachment {
                        filename: attachment.filename.clone(),
                        mime_type: attachment.content_type.clone().unwrap_or_else(|| "application/octet-stream".to_string()),
                        data: bytes.to_vec(),
                    });
                } else {
                    error!("[Discord] Failed to read attachment bytes for {}", attachment.filename);
                }
            } else {
                error!("[Discord] Failed to download attachment from URL: {}", attachment.url);
            }
        }

        let service_msg = ServiceMessage {
            sender: display_name,
            sender_id: msg.author.id.to_string(),
            content: msg.content.clone(),
            attachments,
            source_service: self.service_name.clone(),
            source_channel: msg.channel_id.to_string(),
        };

        if let Err(e) = self.tx.send(service_msg).await {
            error!("[Discord] Failed to send message: {}", e);
        }
    }

    async fn ready(&self, _ctx: Context, ready: Ready) {
        info!("[Discord:{}] Bot is ready as {}", self.service_name, ready.user.name);
    }
}

/// Discord service implementation
pub struct DiscordService {
    name: String,
    config: DiscordServiceConfig,
    client: Option<Arc<TokioMutex<Client>>>,
    http: Option<Arc<serenity::http::Http>>,
}

impl DiscordService {
    pub fn new(name: String, config: DiscordServiceConfig) -> Self {
        Self {
            name,
            config,
            client: None,
            http: None,
        }
    }
}

#[async_trait]
impl Service for DiscordService {
    async fn connect(&mut self) -> Result<()> {
        // We'll create the client in start() because we need the tx channel
        info!("[Discord:{}] Ready to connect", self.name);
        Ok(())
    }

    async fn start(&mut self, tx: mpsc::Sender<ServiceMessage>) -> Result<()> {
        let intents = GatewayIntents::GUILD_MESSAGES
            | GatewayIntents::DIRECT_MESSAGES
            | GatewayIntents::MESSAGE_CONTENT;

        let handler = DiscordHandler {
            tx,
            service_name: self.name.clone(),
            debug: self.config.debug,
        };

        let client = Client::builder(&self.config.bot_token, intents)
            .event_handler(handler)
            .await?;

        // Cache the HTTP client for sending messages without locking the full Client
        self.http = Some(client.http.clone());

        let client = Arc::new(TokioMutex::new(client));
        self.client = Some(client.clone());

        // Start the Discord client in a background task
        tokio::spawn(async move {
            let mut client_guard = client.lock().await;
            if let Err(e) = client_guard.start().await {
                error!("[Discord] Client error: {}", e);
            }
        });

        Ok(())
    }

    async fn send_message(&self, channel: &str, message: &ServiceMessage) -> Result<()> {
        let http = self.http.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Discord HTTP client not connected"))?;

        // Helpful check for users who paste "GuildID/ChannelID" or URLs
        if channel.contains('/') {
            return Err(anyhow::anyhow!(
                "Invalid Discord Channel ID '{}'. Please use ONLY the specific Channel ID (the numeric part, e.g., '1207209226951335976'). Do not include the Server ID or URL.", 
                channel
            ));
        }

        let channel_id: u64 = channel.parse()
            .map_err(|_| anyhow::anyhow!("Invalid Discord channel ID format: '{}'. Expected a numeric ID.", channel))?;
        let channel_id = ChannelId::new(channel_id);

        let formatted_message = format!("{}: {}", message.sender, message.content);

        if self.config.debug {
             info!("[Discord DEBUG] Attempting to send to channel {}: '{}'", channel_id, formatted_message);
        }

        // Send message using the HTTP API with timeout
        // No lock needed as we use the cached Http client
        
        // Wrap the Future in a timeout to detect hangs
        // Create the message builder
        let mut builder = serenity::builder::CreateMessage::new()
            .content(&formatted_message);

        // Add attachments if present
        let mut files = Vec::new(); // Keep files in scope
        
        for attachment in &message.attachments {
            let file = serenity::builder::CreateAttachment::bytes(
                attachment.data.clone(), 
                attachment.filename.clone()
            );
            files.push(file);
        }
        
        if !files.is_empty() {
             builder = builder.files(files);
        }

        // Send message using the HTTP API with timeout
        // Wrap the Future in a timeout to detect hangs
        let send_future = channel_id.send_message(http, builder);
        match tokio::time::timeout(std::time::Duration::from_secs(30), send_future).await { // Increased timeout for uploads
            Ok(result) => {
                match result {
                    Ok(_) => {
                        // info!("[Discord DEBUG] Successfully sent to channel {}", channel);
                        Ok(())
                    },
                    Err(e) => {
                        error!("[Discord] Failed to send message to {}: {:?}", channel, e);
                        // Log specific HTTP errors if possible
                        Err(anyhow::anyhow!("Discord send error: {}", e))
                    }
                }
            },
            Err(_) => {
                error!("[Discord] Send timed out for channel {}", channel);
                Err(anyhow::anyhow!("Discord send timed out"))
            }
        }
    }

    fn service_name(&self) -> &str {
        &self.name
    }

    fn should_bridge_own_messages(&self) -> bool {
        self.config.bridge_own_messages
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("[Discord:{}] Disconnecting", self.name);
        // Serenity client cleanup handled by drop
        Ok(())
    }
}
