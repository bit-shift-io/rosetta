#![recursion_limit = "256"]
use anyhow::Result;
use log::{info, error, warn};
use std::sync::Arc;
use tokio::sync::mpsc;
use matrix_sdk::{
    config::SyncSettings,
    room::Room,
    ruma::{
        events::room::message::{MessageType, OriginalSyncRoomMessageEvent, RoomMessageEventContent},
        RoomId,
    },
    Client as MatrixClient,
};
use whatsapp_rust::{
    bot::Bot,
    store::SqliteStore,
};
use waproto::whatsapp as wa;
use wacore_binary::jid::Jid;
use wacore::types::events::Event as WaEvent;
use whatsapp_rust::proto_helpers::MessageExt;
use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;
use whatsapp_rust_ureq_http_client::UreqHttpClient;
use crate::config::Config;

mod config;

#[derive(Debug)]
struct ToMatrixMsg {
    sender: String,
    content: String,
}

#[derive(Debug)]
struct ToWaMsg {
    sender: String,
    content: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger with filters to reduce noise
    let mut builder = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    // Silence WhatsApp transport "ping/pong" and XML stanza logs (which are INFO level)
    builder.filter_module("whatsapp_rust", log::LevelFilter::Warn);
    builder.filter_module("whatsapp_rust_tokio_transport", log::LevelFilter::Warn);
    builder.filter_module("whatsapp_rust_ureq_http_client", log::LevelFilter::Warn);
    builder.init();

    // Config
    let config = Config::load("config.yaml")?;
    let mx_homeserver = config.matrix.homeserver_url.clone();
    let mx_user = config.matrix.username.clone();
    let mx_pass = config.matrix.password.clone();
    let mx_room_id_str = config.matrix.room_id.clone();
    let wa_target_id = config.whatsapp.target_id.clone();

    // Debug Flags
    let wa_debug = config.whatsapp.debug;
    let mx_debug = config.matrix.debug;
    let bridge_own_messages = config.whatsapp.bridge_own_messages;

    info!("Configuration loaded. Matrix Room: {}, WhatsApp Target: {}", mx_room_id_str, wa_target_id);

    let mx_room_id = <&RoomId>::try_from(mx_room_id_str.as_str())
        .expect("Invalid Matrix Room ID")
        .to_owned();

    // Aliases
    let wa_aliases = Arc::new(config.whatsapp.aliases);
    let mx_aliases = Arc::new(config.matrix.aliases);

    // Channels
    let (tx_to_wa, mut rx_to_wa) = mpsc::channel::<ToWaMsg>(100);
    let (tx_to_mx, mut rx_to_mx) = mpsc::channel::<ToMatrixMsg>(100);

    // --- WhatsApp Setup ---
    let wa_backend = SqliteStore::new("whatsapp.db").await?;
    let wa_backend = Arc::new(wa_backend);
    let wa_transport = TokioWebSocketTransportFactory::new();
    let wa_http = UreqHttpClient::new();

    // Clone tx for the event loop
    let tx_to_mx_clone = tx_to_mx.clone();
    let wa_target_id_clone = wa_target_id.clone();
    let wa_aliases_clone = wa_aliases.clone();

    let mut bot = Bot::builder()
        .with_backend(wa_backend)
        .with_transport_factory(wa_transport)
        .with_http_client(wa_http)
        .on_event(move |event, _client| {
            let tx = tx_to_mx_clone.clone();
            let target = wa_target_id_clone.clone();
            let aliases = wa_aliases_clone.clone();
            async move {
                match event {
                    WaEvent::Message(msg, info) => {
                        let sender_jid = info.source.sender.to_string();
                        let chat_jid = info.source.chat.to_string();
                        
                        // Debug logging
                        if wa_debug {
                            let text_preview = msg.text_content().unwrap_or("<no text>");
                            info!("[WA DEBUG] Msg in Chat: {} From: {} (from_me={}) Content: {}", chat_jid, sender_jid, info.source.is_from_me, text_preview);
                            info!("[WA DEBUG] Comparing Chat '{}' against target '{}'", chat_jid, target);
                        } else {
                            // Minimal log
                            info!("Received WhatsApp message in chat: {}", chat_jid);
                        }
                        
                        // Check if from the bridged contact
                        let is_allowed_sender = !info.source.is_from_me || bridge_own_messages;
                        
                        // We strictly bridge based on the CONVERSATION (chat JID), not the SENDER.
                        // For 1:1 chats, chat_jid == partner_jid.
                        // For groups, chat_jid == group_jid.
                        let is_target = chat_jid.contains(&target) || target.contains(&chat_jid);

                        if is_allowed_sender && is_target {
                             if wa_debug { info!("[WA DEBUG] Target match! Processing message..."); }
                             
                             if let Some(text) = msg.text_content() {
                                 if wa_debug { info!("[WA DEBUG] Sending to Matrix logic task..."); }
                                 
                                 // Aply Alias (User Display Name)
                                 let display_name = aliases.get(&sender_jid).cloned()
                                     .or_else(|| {
                                         if !info.push_name.is_empty() {
                                             Some(info.push_name.clone())
                                         } else {
                                             None
                                         }
                                     }) // Try push_name from WhatsApp (if not empty)
                                     .unwrap_or_else(|| sender_jid.clone()); // Fallback to Sender JID
                                 
                                 if let Err(e) = tx.send(ToMatrixMsg {
                                     sender: display_name,
                                     content: text.to_string(),
                                 }).await {
                                     error!("Failed to send internal message: {}", e);
                                 }
                             } else {
                                 if wa_debug { info!("[WA DEBUG] Message has no text content, skipping."); }
                             }
                        } else if wa_debug {
                            if !is_target {
                                info!("[WA DEBUG] Chat mismatch. Got: '{}', Expected: '{}'", chat_jid, target);
                            } else if !is_allowed_sender {
                                info!("[WA DEBUG] Ignoring own message (BRIDGE_OWN_MESSAGES=false)");
                            }
                        }
                    }
                    WaEvent::PairingQrCode { code, .. } => {
                        info!("Scan this QR code to link WhatsApp:");
                        qr2term::print_qr(&code).unwrap();
                    }
                    WaEvent::Connected(_) => info!("WhatsApp connected!"),
                    _ => {}
                }
            }
        })
        .build()
        .await?;
    
    let wa_client = bot.client();
    let wa_bot_handle = bot.run().await?;

    // --- Matrix Setup ---
    let mx_client = MatrixClient::builder()
        .homeserver_url(mx_homeserver)
        .build()
        .await?;

    mx_client.matrix_auth().login_username(&mx_user, &mx_pass).send().await?;
    info!("Matrix logged in as user: {}", mx_client.user_id().unwrap());

    // Event Dedup (prevent loops)
    // When we send to Matrix, we store the EventID.
    // When we receive from Matrix, we check if it is one we just sent.
    type RecentIds = Arc<std::sync::Mutex<std::collections::VecDeque<matrix_sdk::ruma::OwnedEventId>>>;
    let recent_event_ids: RecentIds = Arc::new(std::sync::Mutex::new(std::collections::VecDeque::with_capacity(30)));

    // Matrix Event Handler
    let mx_client_clone = mx_client.clone();
    let mx_room_id_for_handler = mx_room_id.clone();
    let mx_aliases_clone = mx_aliases.clone();
    
    // Auto-join invitations
    mx_client.add_event_handler(|_event: matrix_sdk::ruma::events::room::member::StrippedRoomMemberEvent, room: Room| async move {
        if room.state() == matrix_sdk::RoomState::Invited {
            info!("Autojoining room {}", room.room_id());
            match room.join().await {
                Ok(_) => info!("Successfully joined room {}", room.room_id()),
                Err(e) => error!("Failed to join room: {}", e),
            }
        }
    });

    // Capture start time to filter old messages
    let start_time = std::time::SystemTime::now();

    let recent_ids_handler = recent_event_ids.clone();
    mx_client.add_event_handler(move |event: OriginalSyncRoomMessageEvent, room: Room| {
        let tx = tx_to_wa.clone();
        let target_room = mx_room_id_for_handler.clone();
        let my_user_id = mx_client_clone.user_id().unwrap().to_owned();
        let aliases = mx_aliases_clone.clone();
        let start_time = start_time;
        let recent_ids = recent_ids_handler.clone();

        async move {
            if mx_debug {
                info!("[MX DEBUG] Event received from {}: {:?}", event.sender, event.content.msgtype);
            }

            // Check Deduplication
            {
                let ids = recent_ids.lock().unwrap();
                if ids.contains(&event.event_id) {
                    if mx_debug { info!("[MX DEBUG] Ignoring Loop (this event was sent by us): {}", event.event_id); }
                    return;
                }
            }

            // Filter old messages (history)
            // We compare the event timestamp against the bot startup time.
            let event_ts_millis: u64 = event.origin_server_ts.get().into();
            let event_time = std::time::UNIX_EPOCH + std::time::Duration::from_millis(event_ts_millis);
            
            if event_time < start_time {
                if mx_debug {
                     info!("[MX DEBUG] Ignoring old message (ts: {:?})", event.origin_server_ts);
                }
                return;
            }

            if room.room_id() != target_room {
                if mx_debug {
                    info!("[MX DEBUG] Ignoring event from room {}", room.room_id());
                }
                return;
            }
            if event.sender == my_user_id {
                if !bridge_own_messages {
                    if mx_debug {
                        info!("[MX DEBUG] Ignoring own message");
                    }
                    return;
                } else if mx_debug {
                    info!("[MX DEBUG] Processing own message (BRIDGE_OWN_MESSAGES=true)");
                }
            }

            if let MessageType::Text(text_content) = &event.content.msgtype {
                 if mx_debug {
                     info!("[MX DEBUG] text message '{}', forwarding...", text_content.body);
                 }
                 
                 let sender_id = event.sender.to_string();
                                  
                 // Fallback logic for display name
                 let display_name = if let Some(alias) = aliases.get(&sender_id) {
                     alias.clone()
                 } else {
                      match room.get_member(&event.sender).await {
                         Ok(Some(member)) => member.display_name().map(|s| s.to_owned()).unwrap_or_else(|| event.sender.to_string()),
                         _ => event.sender.to_string(),
                     }
                 };

                 let _ = tx.send(ToWaMsg {
                     sender: display_name,
                     content: text_content.body.clone(),
                 }).await;
            } else if mx_debug {
                info!("[MX DEBUG] Ignored non-text message");
            }
        }
    });


    // --- Bridge Tasks ---
    
    // Task: WhatsApp -> Matrix
    let mx_client_send = mx_client.clone();
    let mx_room_id_send = mx_room_id.clone();
    let recent_ids_sender = recent_event_ids.clone();
    
    tokio::spawn(async move {
        while let Some(msg) = rx_to_mx.recv().await {
            if mx_debug { info!("[MX DEBUG] Received internal message: {:?}", msg); }
            
            if let Some(room) = mx_client_send.get_room(&mx_room_id_send) {
                if mx_debug { info!("[MX DEBUG] Found Matrix room, sending..."); }
                
                let content = RoomMessageEventContent::text_plain(format!("{}: {}", msg.sender, msg.content));
                match room.send(content).await {
                    Ok(resp) => {
                         if mx_debug { info!("[MX DEBUG] Successfully sent to Matrix! EventID: {}", resp.event_id); }
                         // Store ID to ignore loop
                         let mut ids = recent_ids_sender.lock().unwrap();
                         if ids.len() >= 30 {
                             ids.pop_front();
                         }
                         ids.push_back(resp.event_id);
                    },
                    Err(e) => error!("Failed to send to Matrix: {}", e),
                }
            } else {
                warn!("Matrix room '{}' not found! Ensure the bot is joined and synced.", mx_room_id_send);
                if mx_debug {
                    let joined = mx_client_send.joined_rooms();
                    warn!("[MX DEBUG] Joined rooms: {:?}", joined.iter().map(|r| r.room_id()).collect::<Vec<_>>());
                }
            }
        }
    });

    // Task: Matrix -> WhatsApp
    let wa_client_send = wa_client.clone();
    let wa_target_send = wa_target_id.clone();
    tokio::spawn(async move {
        while let Some(msg) = rx_to_wa.recv().await {
             // Construct message
             let message = wa::Message {
                extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                    text: Some(format!("{}: {}", msg.sender, msg.content)),
                    ..Default::default()
                })),
                ..Default::default()
             };
             
             // Parse JID
             use std::str::FromStr;
             
             let jid_res = Jid::from_str(&wa_target_send);
             
             if let Ok(jid) = jid_res {
                 if let Err(e) = wa_client_send.send_message(jid, message).await {
                      error!("Failed to send to WhatsApp: {}", e);
                 }
             } else {
                 error!("Invalid WhatsApp JID: {}", wa_target_send);
             }

        }
    });

    info!("Bot running...");
    
    // Sync Matrix in background or foreground?
    // We have bot.run() which is wa_bot_handle.
    // And matrix sync.
    // We should run both.
    
    let mx_sync_task = tokio::spawn(async move {
        loop {
            match mx_client.sync(SyncSettings::default()).await {
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

    tokio::select! {
        _ = wa_bot_handle => error!("WhatsApp bot stopped"),
        _ = mx_sync_task => error!("Matrix bot stopped"),
        _ = tokio::signal::ctrl_c() => info!("Ctrl-C received"),
    }

    Ok(())
}
