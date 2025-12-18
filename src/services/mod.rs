use async_trait::async_trait;
use anyhow::Result;
use tokio::sync::mpsc;

/// Represents a file attachment (image, video, etc.)
#[derive(Debug, Clone)]
pub struct Attachment {
    pub filename: String,
    pub mime_type: String,
    pub data: Vec<u8>,
}

/// Common message structure for cross-service communication
#[derive(Debug, Clone)]
pub struct ServiceMessage {
    /// The sender's display name (after alias resolution)
    pub sender: String,
    /// The original sender ID (protocol-specific)
    pub sender_id: String,
    /// The message content
    pub content: String,
    /// Optional file attachments
    pub attachments: Vec<Attachment>,
    /// Source service name
    pub source_service: String,
    /// Source channel identifier
    pub source_channel: String,
    /// Source message identifier (for editing/replying)
    pub source_id: String,
}

// Re-export service implementations
pub mod matrix;
pub mod whatsapp;
pub mod discord;

/// Represents an update to an existing message (edit)
#[derive(Debug, Clone)]
pub struct ServiceUpdate {
    pub source_service: String,
    pub source_channel: String,
    pub source_id: String,
    pub new_content: String,
}

/// Represents a reaction to a message
#[derive(Debug, Clone)]
pub struct ServiceReaction {
    pub source_service: String,
    pub source_channel: String,
    pub source_message_id: String,
    pub sender: String,
    pub emoji: String,
}

/// Enum covering all events a service can emit
#[derive(Debug, Clone)]
pub enum ServiceEvent {
    NewMessage(ServiceMessage),
    UpdateMessage(ServiceUpdate),
    NewReaction(ServiceReaction),
    // Potential future events: DeleteMessage, UserJoin, etc.
}

/// Trait that all chat service implementations must implement
#[async_trait]
pub trait Service: Send + Sync {
    /// Connect to the service and authenticate
    async fn connect(&mut self) -> Result<()>;
    
    /// Start listening for incoming messages
    /// Returns a receiver channel for events from this service
    async fn start(&mut self, tx: mpsc::Sender<ServiceEvent>) -> Result<()>;
    
    /// Send a message to a specific channel on this service
    /// Returns the ID of the sent message (for tracking)
    async fn send_message(&self, channel: &str, message: &ServiceMessage) -> Result<String>;

    /// Edit an existing message
    async fn edit_message(&self, channel: &str, message_id: &str, new_content: &str) -> Result<()>;

    /// React to a message
    async fn react_to_message(&self, channel: &str, message_id: &str, emoji: &str) -> Result<()>;
    
    /// Whether this service should bridge its own messages
    fn should_bridge_own_messages(&self) -> bool {
        false
    }
    
    /// Get the service name
    #[allow(dead_code)]
    fn service_name(&self) -> &str;
    
    /// Check if the service is currently connected
    fn is_connected(&self) -> bool {
        true
    }
    
    /// Get list of members in a channel/room (names)
    /// Returns empty list if not supported or failed
    async fn get_room_members(&self, _channel: &str) -> Result<Vec<String>> {
        Ok(vec![])
    }
    
    /// Disconnect from the service
    #[allow(dead_code)]
    async fn disconnect(&mut self) -> Result<()>;
}
