use async_trait::async_trait;
use anyhow::Result;
use log::{info, error, warn};
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;
use matrix_sdk::media::{MediaFormat, MediaRequestParameters};
use matrix_sdk::{
    Client as MatrixClient,
    config::SyncSettings,
    ruma::{
        events::room::message::{OriginalSyncRoomMessageEvent, RoomMessageEventContent, MessageType, TextMessageEventContent},
        RoomId, OwnedEventId,
    },
    Room,
};
use crate::services::{Service, ServiceMessage};
use crate::config::MatrixServiceConfig;

pub struct MatrixService {
    name: String,
    config: MatrixServiceConfig,
    client: Option<MatrixClient>,
    start_time: std::time::SystemTime,
}

impl MatrixService {
    pub fn new(name: String, config: MatrixServiceConfig) -> Self {
        Self {
            name,
            config,
            client: None,
            start_time: std::time::SystemTime::now(),
        }
    }
    
    #[allow(dead_code)]
    pub fn client(&self) -> Option<&MatrixClient> {
        self.client.as_ref()
    }
}

#[async_trait]
impl Service for MatrixService {
    async fn connect(&mut self) -> Result<()> {
        let client = MatrixClient::builder()
            .homeserver_url(&self.config.homeserver_url)
            .build()
            .await?;

        client
            .matrix_auth()
            .login_username(&self.config.username, &self.config.password)
            .send()
            .await?;
            
        info!("[Matrix:{}] Logged in as user: {}", self.name, client.user_id().unwrap());
        
        // Auto-join invitations
        client.add_event_handler(|_event: matrix_sdk::ruma::events::room::member::StrippedRoomMemberEvent, room: Room| async move {
            if room.state() == matrix_sdk::RoomState::Invited {
                info!("Autojoining room {}", room.room_id());
                match room.join().await {
                    Ok(_) => info!("Successfully joined room {}", room.room_id()),
                    Err(e) => error!("Failed to join room: {}", e),
                }
            }
        });
        
        self.client = Some(client);
        Ok(())
    }

    async fn start(&mut self, tx: mpsc::Sender<ServiceMessage>) -> Result<()> {
        let client = self.client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Matrix client not connected"))?
            .clone();
        
        let service_name = self.name.clone();
        let start_time = self.start_time;
        let debug = self.config.debug;
        
        // Event deduplication - store event IDs we've sent to avoid loops
        type RecentIds = Arc<std::sync::Mutex<std::collections::VecDeque<OwnedEventId>>>;
        let recent_event_ids: RecentIds = Arc::new(std::sync::Mutex::new(
            std::collections::VecDeque::with_capacity(30)
        ));
        
        let my_user_id = client.user_id().unwrap().to_owned();
        let recent_ids_handler = recent_event_ids.clone();
        let safe_client = client.clone(); // Clone for the handler
        
        client.add_event_handler(move |event: OriginalSyncRoomMessageEvent, room: Room| {
            let tx = tx.clone();
            let service_name = service_name.clone();
            let my_user_id = my_user_id.clone();
            let recent_ids = recent_ids_handler.clone();
            let safe_client = safe_client.clone();
            
            async move {
                // Ignore own messages (Critical for preventing loops)
                if event.sender == my_user_id {
                    return;
                }

                if debug {
                    info!("[Matrix] Event received from {}: {:?}", event.sender, event.content.msgtype);
                }

                // Check deduplication
                {
                    let ids = recent_ids.lock().unwrap();
                    if ids.contains(&event.event_id) {
                        if debug { 
                            info!("[Matrix] Ignoring loop (event sent by us): {}", event.event_id); 
                        }
                        return;
                    }
                }

                // Filter old messages
                let event_ts_millis: u64 = event.origin_server_ts.get().into();
                let event_time = std::time::UNIX_EPOCH + std::time::Duration::from_millis(event_ts_millis);
                
                if event_time < start_time {
                    if debug {
                        info!("[Matrix DEBUG] Ignoring old message (ts: {:?})", event.origin_server_ts);
                    }
                    return;
                }

                // Handle different message types (Text and Image)
                let (body, attachments) = match &event.content.msgtype {
                    MessageType::Text(text_content) => {
                        (text_content.body.clone(), vec![])
                    },
                    MessageType::Image(image_content) => {
                        if debug { info!("[Matrix] Received image: {}", image_content.body); }
                        
                        // Download the image
                         let mut attachments = Vec::new();
                         let source = &image_content.source;
                         
                         // Correct usage based on matrix-sdk 0.7+ API patterns
                         // We use the media client from the logged-in client
                         let media_client = safe_client.media();
                         
                         // matrix-sdk's `get_media_content` expects a `MediaRequestParameters`
                         let params = MediaRequestParameters {
                             source: source.clone(),
                             format: MediaFormat::File,
                         };
                         
                         match media_client.get_media_content(&params, true).await {
                             Ok(bytes) => {
                                 let mime = image_content.info.as_ref()
                                     .and_then(|i| i.mimetype.clone())
                                     .unwrap_or("image/jpeg".to_string()); // Default fallback
                                     
                                 let mut filename = image_content.body.clone();
                                 // Ensure extension matches mime (fixes Discord rendering for things like Tenor GIFs)
                                 let extension = match mime.as_str() {
                                     "image/jpeg" | "image/jpg" => ".jpg",
                                     "image/png" => ".png",
                                     "image/gif" => ".gif",
                                     "image/webp" => ".webp",
                                     _ => "",
                                 };
                                 
                                 if !extension.is_empty() && !filename.to_lowercase().ends_with(extension) {
                                     filename.push_str(extension);
                                 }
                                     
                                 attachments.push(crate::services::Attachment {
                                     filename,
                                     mime_type: mime,
                                     data: bytes,
                                 });
                             },
                             Err(e) => error!("[Matrix] Failed to download media: {}", e),
                         }
                         
                        (image_content.body.clone(), attachments)
                    },
                     // Add other types later (Video, etc.)
                    _ => {
                        if debug { info!("[Matrix] Ignored unsupported message type"); }
                        return;
                    }
                };

                let sender_id = event.sender.to_string();
                
                // Get display name from room member
                let display_name = match room.get_member(&event.sender).await {
                    Ok(Some(member)) => member.display_name()
                        .map(|s: &str| s.to_string())
                        .unwrap_or_else(|| event.sender.to_string()),
                    _ => event.sender.to_string(),
                };

                let msg = ServiceMessage {
                    sender: display_name,
                    sender_id: sender_id.clone(),
                    content: body, // Use the body resolved above
                    attachments,   // Use attachments resolved above
                    source_service: service_name.clone(),
                    source_channel: room.room_id().to_string(),
                };
                
                match tx.send(msg).await {
                    Ok(_) => {}, // Success
                    Err(e) => error!("[Matrix] Failed to send msg to Bridge: {}", e),
                }
            }
        });

        // Start sync in background
        let sync_client = client.clone();
        tokio::spawn(async move {
            loop {
                match sync_client.sync(SyncSettings::default()).await {
                    Ok(_) => {
                        info!("Matrix sync stopped gracefully");
                        break;
                    },
                    Err(e) => {
                        error!("Matrix sync failed: {}. Retrying in 5 seconds...", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }
            }
        });

        Ok(())
    }

    async fn send_message(&self, channel: &str, message: &ServiceMessage) -> Result<()> {
        let client = self.client.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Matrix client not connected"))?;
        
        let room_id = <&RoomId>::try_from(channel)?;
        
        if let Some(room) = client.get_room(room_id) {
            // 1. Send text content (if not empty or no attachments - ensuring at least something is sent)
            
            // 1. Send text content (ONLY if there are no attachments, otherwise text goes in caption)
            let send_text = !message.content.is_empty() && message.attachments.is_empty();
            


            if send_text {
            if send_text {
                 let body = format!("{}: {}", message.sender, message.content);
                 
                 // Convert markdown to HTML for Matrix
                 let mut options = pulldown_cmark::Options::empty();
                 options.insert(pulldown_cmark::Options::ENABLE_STRIKETHROUGH);
                 options.insert(pulldown_cmark::Options::ENABLE_TABLES);
                 
                 let parser = pulldown_cmark::Parser::new_ext(&body, options);
                 let mut items = Vec::new();
                 for event in parser {
                    items.push(event);
                 }
                 // Re-create parser since the iterator was consumed (or just iterate directly if compiler allows, but html::push_html takes iterable)
                 let parser = pulldown_cmark::Parser::new_ext(&body, options); // Re-create for push_html
                 
                 let mut html_body = String::new();
                 pulldown_cmark::html::push_html(&mut html_body, parser);
                 
                 let content = RoomMessageEventContent::text_html(body, html_body);
                 room.send(content).await?;
            }
            }

            // 2. Send attachments
            for attachment in &message.attachments {
                // Determine mime type enum
                let mime = mime::Mime::from_str(&attachment.mime_type).unwrap_or(mime::APPLICATION_OCTET_STREAM);
                
                // room.send_attachment handles upload and event creation
                // We construct the attachment config
                let caption = if message.content.is_empty() {
                    format!("Sent by {}", message.sender)
                } else {
                    format!("{}: {}", message.sender, message.content)
                };
                let caption_content = TextMessageEventContent::plain(caption);
                let config = matrix_sdk::attachment::AttachmentConfig::new().caption(Some(caption_content));
                
                if let Err(e) = room.send_attachment(
                    &attachment.filename,
                    &mime,
                    attachment.data.clone(), // Clone data as required by send_attachment
                    config,
                ).await {
                    error!("[Matrix] Failed to send attachment {}: {}", attachment.filename, e);
                }
            }
            
            if self.config.debug {
                info!("[Matrix] Successfully sent message to {}", channel);
            }
            
            Ok(())
        } else {
            warn!("Matrix room '{}' not found! Ensure the bot is joined and synced.", channel);
            Err(anyhow::anyhow!("Room not found: {}", channel))
        }
    }

    fn service_name(&self) -> &str {
        &self.name
    }

    fn should_bridge_own_messages(&self) -> bool {
        self.config.bridge_own_messages
    }

    fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    async fn disconnect(&mut self) -> Result<()> {
        // Matrix SDK doesn't require explicit disconnect
        info!("[Matrix:{}] Disconnecting", self.name);
        Ok(())
    }
}
