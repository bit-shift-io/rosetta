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
    Connectable + MessageSender + ReactionSender + ServiceInfo + Send + Sync + Any
{
}

// Blanket implementation for types implementing all mandatory traits
impl<T> MandatoryService for T where
    T: Connectable + MessageSender + ReactionSender + ServiceInfo + Send + Sync + Any
{
}

/// Alias for optional editor capability
pub trait OptionalEditor: MessageEditor + Send + Sync + Any {}

// Blanket implementation for types implementing editor capability
impl<T> OptionalEditor for T where T: MessageEditor + Send + Sync + Any {}

/// Alias for optional members capability
pub trait OptionalMembers: MemberLister + Send + Sync + Any {}

// Blanket implementation for types implementing members capability
impl<T> OptionalMembers for T where T: MemberLister + Send + Sync + Any {}
