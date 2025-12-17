use async_trait::async_trait;
use anyhow::Result;
use log::{info, error, warn};
use std::sync::Arc;
use tokio::sync::mpsc;
use matrix_sdk::{
    config::SyncSettings,
    room::Room,
    ruma::{
        events::room::message::{MessageType, OriginalSyncRoomMessageEvent, RoomMessageEventContent},
        RoomId, OwnedEventId,
    },
    Client as MatrixClient,
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
        
        client.add_event_handler(move |event: OriginalSyncRoomMessageEvent, room: Room| {
            let tx = tx.clone();
            let service_name = service_name.clone();
            let my_user_id = my_user_id.clone();
            let recent_ids = recent_ids_handler.clone();
            
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

                // Filter old messages (history)
                let event_ts_millis: u64 = event.origin_server_ts.get().into();
                let event_time = std::time::UNIX_EPOCH + std::time::Duration::from_millis(event_ts_millis);
                
                if event_time < start_time {
                    if debug {
                        info!("[Matrix DEBUG] Ignoring old message (ts: {:?})", event.origin_server_ts);
                    }
                    return;
                }

                if let MessageType::Text(text_content) = &event.content.msgtype {
                    if debug {
                        info!("[Matrix DEBUG] text message '{}', forwarding...", text_content.body);
                    }
                    
                    let sender_id = event.sender.to_string();
                    
                    // Get display name from room member
                    let display_name = match room.get_member(&event.sender).await {
                        Ok(Some(member)) => member.display_name()
                            .map(|s| s.to_owned())
                            .unwrap_or_else(|| event.sender.to_string()),
                        _ => event.sender.to_string(),
                    };

                    let msg = ServiceMessage {
                        sender: display_name,
                        sender_id: sender_id.clone(),
                        content: text_content.body.clone(),
                        source_service: service_name.clone(),
                        source_channel: room.room_id().to_string(),
                    };
                    
                    match tx.send(msg).await {
                        Ok(_) => if debug { info!("[Matrix DEBUG] Msg accepted by Bridge"); },
                        Err(e) => error!("[Matrix] Failed to send msg to Bridge: {}", e),
                    }
                } else if debug {
                    info!("[Matrix DEBUG] Ignored non-text message");
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
            let content = RoomMessageEventContent::text_plain(
                format!("{}: {}", message.sender, message.content)
            );
            
            let resp = room.send(content).await?;
            
            if self.config.debug {
                info!("[Matrix DEBUG] Successfully sent to {}! EventID: {}", channel, resp.event_id);
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

    async fn disconnect(&mut self) -> Result<()> {
        // Matrix SDK doesn't require explicit disconnect
        info!("[Matrix:{}] Disconnecting", self.name);
        Ok(())
    }
}
