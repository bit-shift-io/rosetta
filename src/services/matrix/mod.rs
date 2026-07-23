// Matrix service implementation
pub mod formatter;

use crate::bridge::formatter::MessageFormatter;
use crate::config::MatrixServiceConfig;
use crate::services::formatter::MatrixFormatter;
use crate::services::traits::{
    Connectable, MemberLister, MessageEditor, MessageSender, ReactionSender, ServiceInfo,
};
use crate::services::{ServiceEvent, ServiceMessage};
use anyhow::Result;
use async_trait::async_trait;
use log::{error, info, warn};
use matrix_sdk::{
    Client as MatrixClient, Room,
    config::SyncSettings,
    ruma::{
        OwnedEventId, RoomId,
        api::client::authenticated_media::get_media_config::v1::Request as GetMediaConfigRequestV1,
        events::reaction::OriginalSyncReactionEvent,
        events::room::message::{
            MessageType, OriginalSyncRoomMessageEvent, RoomMessageEventContent,
            TextMessageEventContent,
        },
    },
};
use std::any::Any;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct MatrixService {
    name: String,
    config: MatrixServiceConfig,
    client: Option<MatrixClient>,
    start_time: std::time::SystemTime,
    max_upload_size: Option<u64>,
    formatter: MatrixFormatter,
}

impl MatrixService {
    pub fn new(name: String, config: MatrixServiceConfig) -> Self {
        Self {
            name,
            config,
            client: None,
            start_time: std::time::SystemTime::now(),
            max_upload_size: None,
            formatter: MatrixFormatter::new(),
        }
    }

    #[allow(dead_code)]
    pub fn client(&self) -> Option<&MatrixClient> {
        self.client.as_ref()
    }

    pub fn max_upload_size(&self) -> Option<u64> {
        self.max_upload_size
    }
}

#[async_trait]
impl Connectable for MatrixService {
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

        info!(
            "[Matrix:{}] Logged in as user: {}",
            self.name,
            client.user_id().unwrap()
        );

        // Fetch max upload size from server capabilities using v1 API
        let request = GetMediaConfigRequestV1::default();
        match client.send(request).await {
            Ok(response) => {
                let max_bytes: u64 = response.upload_size.into();
                self.max_upload_size = Some(max_bytes);
                info!(
                    "[Matrix:{}] Max upload size: {} bytes ({:.1} MB)",
                    self.name,
                    max_bytes,
                    max_bytes as f64 / 1_048_576.0
                );
            }
            Err(e) => {
                warn!("[Matrix:{}] Failed to fetch media config: {}", self.name, e);
            }
        }

        // Set display name if configured
        if let Some(display_name) = &self.config.display_name {
            if let Err(e) = client.account().set_display_name(Some(display_name)).await {
                error!("Failed to set display name: {}", e);
            } else {
                info!(
                    "[Matrix:{}] Set display name to: {}",
                    self.name, display_name
                );
            }
        }

        // Auto-join invitations
        client.add_event_handler(
            |_event: matrix_sdk::ruma::events::room::member::StrippedRoomMemberEvent,
             room: Room| async move {
                if room.state() == matrix_sdk::RoomState::Invited {
                    info!("Autojoining room {}", room.room_id());
                    match room.join().await {
                        Ok(_) => info!("Successfully joined room {}", room.room_id()),
                        Err(e) => error!("Failed to join room: {}", e),
                    }
                }
            },
        );

        self.client = Some(client);
        Ok(())
    }

    async fn start(&mut self, tx: mpsc::Sender<ServiceEvent>) -> Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Matrix client not connected"))?
            .clone();

        let service_name = self.name.clone();
        let start_time = self.start_time;
        let debug = self.config.debug;

        // Event deduplication - store event IDs we've sent to avoid loops
        type RecentIds = Arc<std::sync::Mutex<std::collections::VecDeque<OwnedEventId>>>;
        let recent_event_ids: RecentIds = Arc::new(std::sync::Mutex::new(
            std::collections::VecDeque::with_capacity(30),
        ));

        let my_user_id = client.user_id().unwrap().to_owned();
        let my_user_id_msgs = my_user_id.clone();
        let recent_ids_handler = recent_event_ids.clone();
        let safe_client = client.clone(); // Clone for the handler

        let tx_msgs = tx.clone();
        let service_name_msgs = service_name.clone();
        client.add_event_handler(move |event: OriginalSyncRoomMessageEvent, room: Room| {
            let tx = tx_msgs.clone();
            let service_name = service_name_msgs.clone();
            let my_user_id = my_user_id_msgs.clone();
            let recent_ids = recent_ids_handler.clone();
            let safe_client = safe_client.clone();

            async move {
                // Ignore own messages if bridging is disabled
                let is_own = event.sender == my_user_id;
                if debug {
                    info!(
                        "[Matrix] Event received from {}: {:?}",
                        event.sender, event.content.msgtype
                    );
                }

                // Check deduplication
                {
                    let ids = recent_ids.lock().unwrap();
                    if ids.contains(&event.event_id) {
                        if debug {
                            info!(
                                "[Matrix] Ignoring loop (event sent by us): {}",
                                event.event_id
                            );
                        }
                        return;
                    }
                }

                // Filter old messages
                let event_ts_millis: u64 = event.origin_server_ts.get().into();
                let event_time =
                    std::time::UNIX_EPOCH + std::time::Duration::from_millis(event_ts_millis);

                if event_time < start_time {
                    if debug {
                        info!(
                            "[Matrix DEBUG] Ignoring old message (ts: {:?})",
                            event.origin_server_ts
                        );
                    }
                    return;
                }

                // Handle Edits (m.replace)
                if let Some(matrix_sdk::ruma::events::room::message::Relation::Replacement(
                    replacement,
                )) = &event.content.relates_to
                {
                    if is_own {
                        if debug {
                            info!("[Matrix] Ignoring own edit loop");
                        }
                        return;
                    }
                    let new_body = match &replacement.new_content.msgtype {
                        MessageType::Text(t) => t.body.clone(),
                        _ => "Unsupported edit content".to_string(),
                    };

                    let update = crate::services::ServiceUpdate {
                        source_service: service_name.clone(),
                        source_channel: room.room_id().to_string(),
                        source_id: replacement.event_id.to_string(), // The original event ID
                        new_content: new_body,
                        is_own,
                    };

                    if let Err(e) = tx.send(ServiceEvent::UpdateMessage(update)).await {
                        error!("[Matrix] Failed to send update: {}", e);
                    }
                    return;
                }

                // Handle different message types (Text and Image)
                let (body, attachments) = match &event.content.msgtype {
                    MessageType::Text(text_content) => (text_content.body.clone(), vec![]),
                    MessageType::Image(image_content) => {
                        if debug {
                            info!("[Matrix] Received image: {}", image_content.body);
                        }

                        // Download the image
                        let mut attachments = Vec::new();
                        let source = &image_content.source;

                        // Correct usage based on matrix-sdk 0.7+ API patterns
                        // We use the media client from the logged-in client
                        let media_client = safe_client.media();

                        // matrix-sdk's `get_media_content` expects a `MediaRequestParameters`
                        let params = matrix_sdk::media::MediaRequestParameters {
                            source: source.clone(),
                            format: matrix_sdk::media::MediaFormat::File,
                        };

                        match media_client.get_media_content(&params, true).await {
                            Ok(bytes) => {
                                let mime = image_content
                                    .info
                                    .as_ref()
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

                                if !extension.is_empty()
                                    && !filename.to_lowercase().ends_with(extension)
                                {
                                    filename.push_str(extension);
                                }

                                attachments.push(crate::services::Attachment {
                                    filename,
                                    mime_type: mime,
                                    data: bytes,
                                });
                            }
                            Err(e) => error!("[Matrix] Failed to download media: {}", e),
                        }

                        ("".to_string(), attachments)
                    }
                    // Add other types later (Video, etc.)
                    _ => {
                        if debug {
                            info!("[Matrix] Ignored unsupported message type");
                        }
                        return;
                    }
                };

                // Get display name from room member
                let display_name = match room.get_member(&event.sender).await {
                    Ok(Some(member)) => member
                        .display_name()
                        .map(|s: &str| s.to_string())
                        .unwrap_or_else(|| event.sender.to_string()),
                    _ => event.sender.to_string(),
                };

                let msg = ServiceMessage {
                    sender: display_name,
                    sender_id: event.sender.to_string(),
                    content: body.to_string(),
                    attachments,
                    source_service: service_name,
                    source_channel: room.room_id().to_string(),
                    source_id: event.event_id.to_string(),
                    is_own,
                };

                match tx.send(ServiceEvent::NewMessage(msg)).await {
                    Ok(_) => {} // Success
                    Err(e) => error!("[Matrix] Failed to send msg to Bridge: {}", e),
                }
            }
        });

        // Add handler for Reactions
        let tx_reactions = tx.clone();
        let service_name_reactions = service_name.clone();
        let my_user_id_reactions = my_user_id.clone();
        client.add_event_handler(
            move |event: OriginalSyncReactionEvent, room: Room| async move {
                let tx = tx_reactions.clone();
                let service_name = service_name_reactions.clone();
                let my_user_id = my_user_id_reactions.clone();

                let is_own = event.sender == my_user_id;

                info!(
                    "[Matrix:{}] Received reaction '{}' in room {}",
                    service_name,
                    event.content.relates_to.key,
                    room.room_id()
                );

                let reaction_event = crate::services::ServiceReaction {
                    source_service: service_name.clone(),
                    source_channel: room.room_id().to_string(),
                    source_message_id: event.content.relates_to.event_id.to_string(),
                    _sender: event.sender.to_string(),
                    emoji: event.content.relates_to.key.clone(),
                    is_own,
                };

                if let Err(e) = tx.send(ServiceEvent::NewReaction(reaction_event)).await {
                    error!("[Matrix] Failed to send reaction event: {}", e);
                }
            },
        );

        // Start sync in background
        let sync_client = client.clone();
        tokio::spawn(async move {
            loop {
                match sync_client.sync(SyncSettings::default()).await {
                    Ok(_) => {
                        info!("Matrix sync stopped gracefully");
                        break;
                    }
                    Err(e) => {
                        error!("Matrix sync failed: {}. Retrying in 5 seconds...", e);
                        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    }
                }
            }
        });

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.client.is_some()
    }

    async fn wait_until_ready(&self) -> Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Matrix client not connected"))?;

        info!("[Matrix:{}] Waiting for initial sync...", self.name);

        for _ in 0..30 {
            if !client.joined_rooms().is_empty() {
                info!("[Matrix:{}] Synchronized and ready!", self.name);
                return Ok(());
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        warn!(
            "[Matrix:{}] Wait until ready timed out, but proceeding anyway.",
            self.name
        );
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("[Matrix:{}] Disconnecting", self.name);
        Ok(())
    }
}

impl ServiceInfo for MatrixService {
    fn service_name(&self) -> &str {
        &self.name
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

#[async_trait]
impl MessageSender for MatrixService {
    async fn send_message(&self, channel: &str, message: &ServiceMessage) -> Result<String> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Matrix client not connected"))?;

        let room_id = <&RoomId>::try_from(channel)?;

        if let Some(room) = client.get_room(room_id) {
            let mut last_event_id = String::new();

            // 1. Send text if present AND no attachments (otherwise text goes in caption)
            if !message.content.is_empty() && message.attachments.is_empty() {
                let body =
                    self.formatter
                        .format_text(&message.sender, &message.content, message.is_own);

                // Convert markdown to HTML for Matrix
                let html_body = self.formatter.markdown_to_html(&body);

                let content = RoomMessageEventContent::text_html(body, html_body);
                let resp = room.send(content).await?;
                last_event_id = resp.response.event_id.to_string();
            }

            // 2. Send attachments
            for attachment in &message.attachments {
                let mime = mime::Mime::from_str(&attachment.mime_type)
                    .unwrap_or_else(|_| mime::APPLICATION_OCTET_STREAM);

                // Construct caption using formatter
                let caption = self.formatter.format_caption(
                    &message.sender,
                    &message.content,
                    message.is_own,
                );

                let caption_content = TextMessageEventContent::plain(caption);
                let config =
                    matrix_sdk::attachment::AttachmentConfig::new().caption(Some(caption_content));

                match room
                    .send_attachment(&attachment.filename, &mime, attachment.data.clone(), config)
                    .await
                {
                    Ok(resp) => last_event_id = resp.event_id.to_string(),
                    Err(e) => error!(
                        "[Matrix] Failed to send attachment {}: {}",
                        attachment.filename, e
                    ),
                }
            }

            Ok(last_event_id)
        } else {
            warn!(
                "Matrix room '{}' not found! Ensure the bot is joined and synced.",
                channel
            );
            Err(anyhow::anyhow!("Room not found: {}", channel))
        }
    }
}

#[async_trait]
impl MessageEditor for MatrixService {
    async fn edit_message(&self, channel: &str, message_id: &str, new_content: &str) -> Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Matrix client not connected"))?;
        let room_id = <&RoomId>::try_from(channel)?;

        let event_id = OwnedEventId::try_from(message_id)
            .map_err(|e| anyhow::anyhow!("Invalid event ID: {}", e))?;

        if let Some(room) = client.get_room(room_id) {
            let content = RoomMessageEventContent::text_plain(new_content).make_replacement(
                matrix_sdk::ruma::events::room::message::ReplacementMetadata::new(event_id, None),
            );

            room.send(content).await?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Room not found"))
        }
    }
}

#[async_trait]
impl ReactionSender for MatrixService {
    async fn react_to_message(&self, channel: &str, message_id: &str, emoji: &str) -> Result<()> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Matrix client not connected"))?;
        let room_id = <&RoomId>::try_from(channel)?;
        let event_id = OwnedEventId::try_from(message_id)
            .map_err(|e| anyhow::anyhow!("Invalid event ID: {}", e))?;

        if let Some(room) = client.get_room(room_id) {
            let reaction = matrix_sdk::ruma::events::reaction::ReactionEventContent::new(
                matrix_sdk::ruma::events::relation::Annotation::new(event_id, emoji.to_string()),
            );
            room.send(reaction).await?;
            Ok(())
        } else {
            Err(anyhow::anyhow!("Room not found"))
        }
    }
}

#[async_trait]
impl MemberLister for MatrixService {
    async fn get_room_members(&self, channel: &str) -> Result<Vec<String>> {
        let client = self
            .client
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Matrix client not connected"))?;

        let room_id = <&RoomId>::try_from(channel)?;

        if let Some(room) = client.get_room(room_id) {
            let members = room
                .members_no_sync(matrix_sdk::RoomMemberships::JOIN)
                .await?;
            let mut names = Vec::new();
            for member in members.iter().take(50) {
                // Limit to 50
                names.push(
                    member
                        .display_name()
                        .unwrap_or(member.user_id().as_str())
                        .to_string(),
                );
            }
            if members.len() > 50 {
                names.push(format!("...and {} more", members.len() - 50));
            }
            Ok(names)
        } else {
            Ok(vec!["Room not found".to_string()])
        }
    }
}
