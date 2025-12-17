use async_trait::async_trait;
use anyhow::Result;
use log::{info, error, warn};
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
use reqwest::Url;

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
        
        // Handle Links (Scraping)
        // Check for URLs in content that might be images/gifs (Tenor, etc.)
        let mut content = msg.content.clone();
        
        // Simple regex for URLs
        if let Ok(url_regex) = regex::Regex::new(r"https?://[^\s]+") {
            let mut urls_to_process = Vec::new();
            for caps in url_regex.captures_iter(&msg.content) {
                if let Some(url_match) = caps.get(0) {
                    urls_to_process.push(url_match.as_str().to_string());
                }
            }
            
            // Also check Embeds for URLs (e.g. Tenor GIFs that might be embeds)
            for embed in &msg.embeds {
                if let Some(url) = &embed.url {
                    urls_to_process.push(url.to_string());
                } else if let Some(image) = &embed.image {
                     urls_to_process.push(image.url.to_string());
                } else if let Some(video) = &embed.video {
                     urls_to_process.push(video.url.to_string());
                }
            }
            
            // Deduplicate URLs
            urls_to_process.sort();
            urls_to_process.dedup();
            
            // Create a client with a browser-like User-Agent to avoid 403s (Tenor/Giphy often block default agents)
            let client = reqwest::Client::builder()
                .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
                .build()
                .unwrap_or_default();

            for url in urls_to_process {
                if self.debug {
                    info!("[Discord DEBUG] Scraping URL: {}", url);
                }

                // Check if it's media
                 match client.get(&url).send().await {
                     Ok(resp) => {
                         let status = resp.status();
                         if self.debug {
                             info!("[Discord DEBUG] Scrape HTTP Status: {}", status);
                         }

                         let mime = resp.headers().get("content-type")
                             .and_then(|v| v.to_str().ok())
                             .unwrap_or("")
                             .to_string();
                             
                         let mut media_data = None;
                         let mut filename = "image".to_string();
                         let mut resolved_mime = mime.clone();
                         
                         if mime.starts_with("image/") {
                             // Direct image link
                             if let Ok(bytes) = resp.bytes().await {
                                 media_data = Some(bytes.to_vec());
                                 // Try to derive filename from URL
                                 if let Ok(u) = Url::parse(&url) {
                                     if let Some(segments) = u.path_segments() {
                                         if let Some(last) = segments.last() {
                                             if !last.is_empty() {
                                                 filename = last.to_string();
                                             }
                                         }
                                     }
                                 }
                             }
                         } else if mime.starts_with("text/html") {
                             // HTML page (potential OG:Image, e.g. Tenor)
                             if let Ok(text) = resp.text().await {
                                 // Look for <meta property="og:image" content="...">
                                 // Try multiple patterns to handle attribute ordering, quoting, and intervening attributes (like class="dynamic")
                                 let patterns = [
                                     r#"<meta\s+[^>]*?property=["']og:image["'][^>]*?content=["']([^"']+)["']"#,
                                     r#"<meta\s+[^>]*?content=["']([^"']+)["'][^>]*?property=["']og:image["']"#,
                                 ];
                                 
                                 let mut found_og_url = None;
                                 
                                 for pattern in &patterns {
                                    if let Ok(re) = regex::Regex::new(pattern) {
                                        if let Some(caps) = re.captures(&text) {
                                            if let Some(match_url) = caps.get(1) {
                                                found_og_url = Some(match_url.as_str().to_string());
                                                break;
                                            }
                                        }
                                    }
                                 }

                                 if let Some(target_url) = found_og_url {
                                     let target_url = target_url.as_str();
                                     if self.debug { info!("[Discord DEBUG] Found OG:Image: {}", target_url); }
                                     
                                     // Download the OG image using the same client
                                     if let Ok(og_resp) = client.get(target_url).send().await {
                                         let og_mime = og_resp.headers().get("content-type")
                                             .and_then(|v| v.to_str().ok())
                                             .unwrap_or("image/jpeg") // Assume image if OG:Image
                                             .to_string();
                                         
                                         if let Ok(bytes) = og_resp.bytes().await {
                                              media_data = Some(bytes.to_vec());
                                              resolved_mime = og_mime;
                                              filename = "embed.gif".to_string(); // Default to gif for tenor usually
                                              
                                              // Try to improve filename
                                              if let Ok(u) = Url::parse(target_url) {
                                                 if let Some(segments) = u.path_segments() {
                                                     if let Some(last) = segments.last() {
                                                         filename = last.to_string();
                                                     }
                                                 }
                                             }
                                         }
                                     }
                                 } else {
                                     if self.debug {
                                         warn!("[Discord DEBUG] No OG:Image found in HTML. Snippet: {}", &text.chars().take(500).collect::<String>());
                                     }
                                 }
                             }
                         }
                         
                         if let Some(data) = media_data {
                             if self.debug { info!("[Discord DEBUG] Scraped media from link: {} ({} bytes)", url, data.len()); }
                             
                             attachments.push(crate::services::Attachment {
                                 filename,
                                 mime_type: resolved_mime,
                                 data,
                             });
                             
                             // Remove the URL from content if it was successfully scraped
                             // causing the message to become effectively empty if it was just a link
                             content = content.replace(&url, "").trim().to_string();
                         }
                     },
                     Err(e) => if self.debug { error!("[Discord DEBUG] Failed to fetch link {}: {}", url, e); }
                 }
            }
        }

        let service_msg = ServiceMessage {
            sender: display_name,
            sender_id: msg.author.id.to_string(),
            content, // Use the modified content (links removed)
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
