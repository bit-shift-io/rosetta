use anyhow::Result;
use async_trait::async_trait;
use std::any::Any;
use tokio::sync::mpsc;

use crate::services::{ServiceEvent, ServiceMessage};

/// Trait for service connection lifecycle
#[async_trait]
pub trait Connectable: Send + Sync + Any {
    /// Connect to the service and authenticate
    async fn connect(&mut self) -> Result<()>;

    /// Start listening for incoming messages
    async fn start(&mut self, tx: mpsc::Sender<ServiceEvent>) -> Result<()>;

    /// Check if the service is currently connected
    fn is_connected(&self) -> bool;

    /// Wait until the service is fully initialized and synchronized
    async fn wait_until_ready(&self) -> Result<()>;

    /// Disconnect from the service
    async fn disconnect(&mut self) -> Result<()>;
}

/// Trait for sending messages
#[async_trait]
pub trait MessageSender: Send + Sync {
    /// Send a message to a specific channel on this service
    /// Returns the ID of the sent message (for tracking)
    async fn send_message(&self, channel: &str, message: &ServiceMessage) -> Result<String>;
}

/// Trait for editing messages (optional capability)
#[async_trait]
pub trait MessageEditor: Send + Sync {
    /// Edit an existing message
    async fn edit_message(&self, channel: &str, message_id: &str, new_content: &str) -> Result<()>;
}

/// Trait for sending reactions (optional capability)
#[async_trait]
pub trait ReactionSender: Send + Sync {
    /// React to a message
    async fn react_to_message(&self, channel: &str, message_id: &str, emoji: &str) -> Result<()>;
}

/// Trait for listing channel/room members (optional capability)
#[async_trait]
pub trait MemberLister: Send + Sync {
    /// Get list of members in a channel/room (names)
    /// Returns empty list if not supported or failed
    async fn get_room_members(&self, channel: &str) -> Result<Vec<String>>;
}

/// Trait for fetching room/channel display names (optional capability)
#[async_trait]
pub trait RoomNameFetcher: Send + Sync {
    /// Get the display name for a room/channel
    /// Returns None if not supported or failed
    async fn get_room_name(&self, channel: &str) -> Result<Option<String>>;
}

/// Trait for service metadata
pub trait ServiceInfo: Send + Sync + Any {
    /// Get the service name
    fn service_name(&self) -> &str;

    /// Get the underlying Any for downcasting
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

/// Alias for mandatory traits that all services must implement
pub trait MandatoryService:
    Connectable + MessageSender + ReactionSender + MemberLister + RoomNameFetcher + ServiceInfo + Send + Sync + Any
{
}

// Blanket implementation for types implementing all mandatory traits
impl<T> MandatoryService for T where
    T: Connectable + MessageSender + ReactionSender + MemberLister + RoomNameFetcher + ServiceInfo + Send + Sync + Any
{
}

// Implement subtraits for Box<dyn MandatoryService>
#[async_trait]
impl Connectable for Box<dyn MandatoryService> {
    async fn connect(&mut self) -> Result<()> {
        (**self).connect().await
    }

    async fn start(&mut self, tx: mpsc::Sender<ServiceEvent>) -> Result<()> {
        (**self).start(tx).await
    }

    fn is_connected(&self) -> bool {
        (**self).is_connected()
    }

    async fn wait_until_ready(&self) -> Result<()> {
        (**self).wait_until_ready().await
    }

    async fn disconnect(&mut self) -> Result<()> {
        (**self).disconnect().await
    }
}

#[async_trait]
impl MessageSender for Box<dyn MandatoryService> {
    async fn send_message(&self, channel: &str, message: &ServiceMessage) -> Result<String> {
        (**self).send_message(channel, message).await
    }
}

#[async_trait]
impl ReactionSender for Box<dyn MandatoryService> {
    async fn react_to_message(&self, channel: &str, message_id: &str, emoji: &str) -> Result<()> {
        (**self).react_to_message(channel, message_id, emoji).await
    }
}

impl ServiceInfo for Box<dyn MandatoryService> {
    fn service_name(&self) -> &str {
        (**self).service_name()
    }

    fn as_any(&self) -> &dyn Any {
        (**self).as_any()
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        (**self).as_any_mut()
    }
}

/// Alias for optional editor capability
pub trait OptionalEditor: MessageEditor + Send + Sync + Any {}

// Blanket implementation for types implementing editor capability
impl<T> OptionalEditor for T where T: MessageEditor + Send + Sync + Any {}

/// Alias for optional members capability
pub trait OptionalMembers: MemberLister + Send + Sync + Any {}

// Blanket implementation for types implementing members capability
impl<T> OptionalMembers for T where T: MemberLister + Send + Sync + Any {}

/// Alias for optional room name fetcher capability
pub trait OptionalRoomNameFetcher: RoomNameFetcher + Send + Sync + Any {}

// Blanket implementation for types implementing room name fetcher capability
impl<T> OptionalRoomNameFetcher for T where T: RoomNameFetcher + Send + Sync + Any {}

// Implement OptionalEditor for Box<dyn MandatoryService>
#[async_trait]
impl MessageEditor for Box<dyn MandatoryService> {
    async fn edit_message(&self, channel: &str, message_id: &str, new_content: &str) -> Result<()> {
        if let Some(editor) = self.as_any().downcast_ref::<Box<dyn OptionalEditor>>() {
            editor.edit_message(channel, message_id, new_content).await
        } else {
            Err(anyhow::anyhow!("Service does not support message editing"))
        }
    }
}

// Implement OptionalMembers for Box<dyn MandatoryService> - NOT NEEDED since MandatoryService: MemberLister
// The trait object dyn MandatoryService already implements MemberLister directly

// Implement OptionalRoomNameFetcher for Box<dyn MandatoryService> - NOT NEEDED since MandatoryService: RoomNameFetcher

#[async_trait]
impl MemberLister for Box<dyn MandatoryService> {
    async fn get_room_members(&self, channel: &str) -> Result<Vec<String>> {
        (**self).get_room_members(channel).await
    }
}

#[async_trait]
impl RoomNameFetcher for Box<dyn MandatoryService> {
    async fn get_room_name(&self, channel: &str) -> Result<Option<String>> {
        (**self).get_room_name(channel).await
    }
}
