// Discord service implementation
pub mod formatter;

use crate::bridge::formatter::MessageFormatter;
use crate::config::DiscordServiceConfig;
use crate::gif::GifResolver;
use crate::services::formatter::DiscordFormatter;
use crate::services::traits::{
    Connectable, MemberLister, MessageEditor, MessageSender, ReactionSender, ServiceInfo,
};
use crate::services::{ServiceEvent, ServiceMessage, ServiceUpdate};
use anyhow::Result;
use async_trait::async_trait;
use log::{error, info, warn};
use serenity::{
    async_trait as serenity_async_trait,
    client::{Client, Context, EventHandler},
    model::{channel::Message, gateway::Ready, id::ChannelId},
};
use std::any::Any;
use std::sync::Arc;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::mpsc;

struct DiscordHandler {
    tx: mpsc::Sender<ServiceEvent>,
    service_name: String,
    debug: bool,
    display_name: Option<String>,
    gif_resolver: Arc<GifResolver>,
}

#[serenity_async_trait]
impl EventHandler for DiscordHandler {
    async fn message(&self, ctx: Context, msg: Message) {
        let is_own = msg.author.id == ctx.cache.current_user().id;

        if is_own {
            // bridge coordinator will filter if needed
        } else if msg.author.bot {
            // Ignore other bots
            return;
        }

        // Try to get the guild-specific nickname, then global display name, then username
        let display_name = msg
            .author_nick(&ctx)
            .await
            .or_else(|| msg.author.global_name.clone())
            .unwrap_or_else(|| msg.author.name.clone());

        if self.debug {
            info!(
                "[Discord DEBUG] Message received from {} (id: {}): {}",
                display_name, msg.author.id, msg.content
            );
        }

        // Handle attachments
        let mut attachments = Vec::new();
        for attachment in &msg.attachments {
            if let Ok(response) = reqwest::get(&attachment.url).await {
                if let Ok(bytes) = response.bytes().await {
                    attachments.push(crate::services::Attachment {
                        filename: attachment.filename.clone(),
                        mime_type: attachment
                            .content_type
                            .clone()
                            .unwrap_or_else(|| "application/octet-stream".to_string()),
                        data: bytes.to_vec(),
                    });
                } else {
                    if self.debug {
                        error!(
                            "[Discord] Failed to read attachment bytes for {}",
                            attachment.filename
                        );
                    }
                }
            } else {
                if self.debug {
                    error!(
                        "[Discord] Failed to download attachment from URL: {}",
                        attachment.url
                    );
                }
            }
        }

        let mut content = msg.content.clone();

        // Resolve GIF URLs using GifResolver (APIs + fallback scraper)
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

            for url in urls_to_process {
                info!("[Discord] Processing URL: {}", url);
                if let Ok(resolved) = self.gif_resolver.resolve(&url).await {
                    if let Some(gif) = resolved {
                        info!(
                            "[Discord] Resolved {} via {}: {} (mime: {})",
                            url, gif.provider, gif.url, gif.mime_type
                        );

                        // Download the resolved media
                        if let Ok(media_data) = download_media(&gif.url).await {
                            attachments.push(crate::services::Attachment {
                                filename: gif.filename,
                                mime_type: gif.mime_type,
                                data: media_data,
                            });

                            // Remove URL from content
                            content = content.replace(&url, "").trim().to_string();
                        }
                    }
                }
            }
        }

        async fn download_media(url: &str) -> anyhow::Result<Vec<u8>> {
            let resp = reqwest::get(url).await?;
            let bytes = resp.bytes().await?;
            Ok(bytes.to_vec())
        }

        let service_msg = ServiceMessage {
            sender: display_name,
            sender_id: msg.author.id.to_string(),
            content,
            attachments,
            source_service: self.service_name.clone(),
            source_channel: msg.channel_id.to_string(),
            source_id: msg.id.to_string(),
            is_own,
        };

        if let Err(e) = self.tx.send(ServiceEvent::NewMessage(service_msg)).await {
            error!("[Discord] Failed to send message: {}", e);
        }
    }

    async fn message_update(
        &self,
        ctx: Context,
        _old_if_available: Option<Message>,
        _new: Option<Message>,
        event: serenity::model::event::MessageUpdateEvent,
    ) {
        if let Some(content) = event.content {
            let is_own = if let Some(author) = &event.author {
                author.id == ctx.cache.current_user().id
            } else {
                false
            };

            let update = ServiceUpdate {
                source_service: self.service_name.clone(),
                source_channel: event.channel_id.to_string(),
                source_id: event.id.to_string(),
                new_content: content,
                is_own,
            };

            if let Err(e) = self.tx.send(ServiceEvent::UpdateMessage(update)).await {
                error!("[Discord] Failed to send update: {}", e);
            }
        }
    }

    async fn reaction_add(&self, _ctx: Context, add_reaction: serenity::model::channel::Reaction) {
        let is_own = if let Some(user_id) = add_reaction.user_id {
            user_id == _ctx.cache.current_user().id
        } else {
            false
        };

        if is_own {
            // bridge coordinator will filter if needed
        }

        info!(
            "[Discord:{}] Received reaction '{}' in channel {} on message {}",
            self.service_name, add_reaction.emoji, add_reaction.channel_id, add_reaction.message_id
        );

        let emoji = add_reaction.emoji.to_string();

        let reaction_event = crate::services::ServiceReaction {
            source_service: self.service_name.clone(),
            source_channel: add_reaction.channel_id.to_string(),
            source_message_id: add_reaction.message_id.to_string(),
            _sender: add_reaction
                .user_id
                .map(|u| u.to_string())
                .unwrap_or("unknown".to_string()),
            emoji,
            is_own,
        };

        if let Err(e) = self
            .tx
            .send(ServiceEvent::NewReaction(reaction_event))
            .await
        {
            error!("[Discord] Failed to send reaction event: {}", e);
        }
    }

    async fn ready(&self, ctx: Context, ready: Ready) {
        info!(
            "[Discord:{}] Bot is ready as {}",
            self.service_name, ready.user.name
        );

        // Update display name (username) if configured
        if let Some(target_name) = &self.display_name {
            if ready.user.name != *target_name {
                info!(
                    "[Discord:{}] Updating username to '{}'...",
                    self.service_name, target_name
                );

                // Fetch current user to edit
                match ctx.http.get_current_user().await {
                    Ok(mut user) => {
                        let builder = serenity::builder::EditProfile::new().username(target_name);
                        if let Err(e) = user.edit(&ctx, builder).await {
                            error!(
                                "[Discord:{}] Failed to update username: {}",
                                self.service_name, e
                            );
                        } else {
                            info!(
                                "[Discord:{}] Username updated successfully!",
                                self.service_name
                            );
                        }
                    }
                    Err(e) => {
                        error!(
                            "[Discord:{}] Failed to fetch current user: {}",
                            self.service_name, e
                        );
                    }
                }
            }
        }
    }
}

pub struct DiscordService {
    name: String,
    config: DiscordServiceConfig,
    client: Option<Arc<TokioMutex<Client>>>,
    http: Option<Arc<serenity::http::Http>>,
    cache: Option<Arc<serenity::cache::Cache>>,
    gif_resolver: Arc<GifResolver>,
    formatter: DiscordFormatter,
}

impl DiscordService {
    pub fn new(name: String, config: DiscordServiceConfig, gif_resolver: Arc<GifResolver>) -> Self {
        Self {
            name,
            config,
            client: None,
            http: None,
            cache: None,
            gif_resolver,
            formatter: DiscordFormatter::new(),
        }
    }
}

#[async_trait]
impl Connectable for DiscordService {
    async fn connect(&mut self) -> Result<()> {
        // We'll create the client in start() because we need the tx channel
        info!("[Discord:{}] Ready to connect", self.name);
        Ok(())
    }

    async fn start(&mut self, tx: mpsc::Sender<ServiceEvent>) -> Result<()> {
        let intents = serenity::model::gateway::GatewayIntents::GUILD_MESSAGES
            | serenity::model::gateway::GatewayIntents::DIRECT_MESSAGES
            | serenity::model::gateway::GatewayIntents::MESSAGE_CONTENT
            | serenity::model::gateway::GatewayIntents::GUILD_MEMBERS
            | serenity::model::gateway::GatewayIntents::GUILD_MESSAGE_REACTIONS
            | serenity::model::gateway::GatewayIntents::DIRECT_MESSAGE_REACTIONS
            | serenity::model::gateway::GatewayIntents::GUILDS;

        let handler = DiscordHandler {
            tx,
            service_name: self.name.clone(),
            debug: self.config.debug,
            display_name: self.config.display_name.clone(),
            gif_resolver: self.gif_resolver.clone(),
        };

        let client = Client::builder(&self.config.bot_token, intents)
            .event_handler(handler)
            .await?;

        // Cache the HTTP client for sending messages without locking the full Client
        self.http = Some(client.http.clone());
        self.cache = Some(client.cache.clone());

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

    fn is_connected(&self) -> bool {
        self.client.is_some() && self.http.is_some()
    }

    async fn wait_until_ready(&self) -> Result<()> {
        info!("[Discord:{}] Waiting for gateway connection...", self.name);

        let cache = self
            .cache
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Discord Cache not initialized"))?;

        // Wait up to 30 seconds for guilds to be cached (indicates ready)
        for _ in 0..30 {
            if cache.guild_count() > 0 {
                info!(
                    "[Discord:{}] Gateway connected and cache populated!",
                    self.name
                );
                return Ok(());
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        warn!(
            "[Discord:{}] Wait until ready timed out, but proceeding anyway.",
            self.name
        );
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<()> {
        info!("[Discord:{}] Disconnecting", self.name);
        // Serenity client cleanup handled by drop
        Ok(())
    }
}

#[async_trait]
impl MessageSender for DiscordService {
    async fn send_message(&self, channel: &str, message: &ServiceMessage) -> Result<String> {
        let http = self
            .http
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Discord HTTP client not connected"))?;

        if channel.contains('/') {
            return Err(anyhow::anyhow!("Invalid Channel ID"));
        }

        let channel_id: u64 = channel.parse()?;
        let channel_id = ChannelId::new(channel_id);

        let formatted_message =
            self.formatter
                .format_text(&message.sender, &message.content, message.is_own);
        let mut builder = serenity::builder::CreateMessage::new().content(&formatted_message);

        let mut files = Vec::new();
        for attachment in &message.attachments {
            let file = serenity::builder::CreateAttachment::bytes(
                attachment.data.clone(),
                attachment.filename.clone(),
            );
            files.push(file);
        }

        if !files.is_empty() {
            builder = builder.files(files);
        }

        match channel_id.send_message(http, builder).await {
            Ok(msg) => Ok(msg.id.to_string()),
            Err(e) => Err(anyhow::anyhow!("Discord send error: {}", e)),
        }
    }
}

#[async_trait]
impl MessageEditor for DiscordService {
    async fn edit_message(&self, channel: &str, message_id: &str, new_content: &str) -> Result<()> {
        let http = self
            .http
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Discord HTTP client not connected"))?;

        let channel_id: u64 = channel.parse()?;
        let channel_id = ChannelId::new(channel_id);
        let msg_id: u64 = message_id.parse()?;
        let msg_id = serenity::model::id::MessageId::new(msg_id);

        let builder = serenity::builder::EditMessage::new().content(new_content);

        channel_id.edit_message(http, msg_id, builder).await?;
        Ok(())
    }
}

#[async_trait]
impl ReactionSender for DiscordService {
    async fn react_to_message(&self, channel: &str, message_id: &str, emoji: &str) -> Result<()> {
        let http = self
            .http
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Discord HTTP client not connected"))?;

        let channel_id: u64 = channel.parse()?;
        let channel_id = ChannelId::new(channel_id);
        let msg_id: u64 = message_id.parse()?;
        let msg_id = serenity::model::id::MessageId::new(msg_id);

        // Parse emoji:
        // For simple Unicode, we just pass the char.
        // Serenity expects ReactionType.
        let reaction_type = serenity::model::channel::ReactionType::try_from(emoji)
            .map_err(|_| anyhow::anyhow!("Invalid emoji"))?;

        channel_id
            .create_reaction(http, msg_id, reaction_type)
            .await?;
        Ok(())
    }
}

#[async_trait]
impl MemberLister for DiscordService {
    async fn get_room_members(&self, channel: &str) -> Result<Vec<String>> {
        let cache = self
            .cache
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Discord Cache not initialized"))?;

        let channel_id: u64 = channel.parse()?;
        let channel_id = ChannelId::new(channel_id);

        // SCOPE 1: Get Guild ID and Cached Members
        // We use a block to ensure CacheRef (guild_channel, guild) are dropped before any await
        let (guild_id_method, mut names) = {
            #[allow(deprecated)]
            if let Some(guild_channel) = cache.channel(channel_id) {
                let guild_id = guild_channel.guild_id;
                let mut current_names = Vec::new();

                // Get the full guild from cache to access members
                if let Some(guild) = cache.guild(guild_id) {
                    // Strategy 1: Check Cached Members
                    for (_user_id, member) in &guild.members {
                        let perms = guild.user_permissions_in(&*guild_channel, member);
                        if perms.contains(serenity::model::permissions::Permissions::VIEW_CHANNEL) {
                            current_names.push(member.display_name().to_string());
                        }
                    }
                }
                (Some(guild_id), current_names)
            } else {
                (None, Vec::new())
            }
        };

        // Handle case where channel wasn't found in cache at all
        let guild_id = match guild_id_method {
            Some(gid) => gid,
            None => {
                return Ok(vec![
                    "Channel not found in cache (Bot starting up?)".to_string(),
                ]);
            }
        };

        // SCOPE 2: HTTP Fallback (Async)
        // Only run if we found few members (likely just the bot)
        if names.len() <= 1 {
            if let Some(http) = self.http.as_ref() {
                // Fetch up to 1000 members via API. Safe to await here as CacheRefs are gone.
                if let Ok(http_members) = guild_id.members(http, Some(1000), None).await {
                    // SCOPE 3: Re-acquire CacheRefs to filter the HTTP results
                    #[allow(deprecated)]
                    if let Some(guild_channel) = cache.channel(channel_id) {
                        if let Some(guild) = cache.guild(guild_id) {
                            names.clear(); // Restart list with authoritative data
                            for member in http_members {
                                // Calculate permissions for this HTTP member using cached Guild structure
                                let perms = guild.user_permissions_in(&*guild_channel, &member);
                                if perms.contains(
                                    serenity::model::permissions::Permissions::VIEW_CHANNEL,
                                ) {
                                    names.push(member.display_name().to_string());
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort for consistency
        names.sort();
        names.dedup();

        // Pagination logic for display
        let total = names.len();

        if total <= 1 {
            names.push(
                "(Found no members with access. Check 'Server Members Intent' and Bot Permissions)"
                    .to_string(),
            );
        }

        let display_names: Vec<String> = names.into_iter().take(50).collect();
        let mut result = display_names;

        if total > 50 {
            result.push(format!("...and {} more", total - 50));
        }

        Ok(result)
    }
}

impl ServiceInfo for DiscordService {
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
