use async_trait::async_trait;
use anyhow::Result;
use tokio::sync::mpsc;

/// Common message structure for cross-service communication
#[derive(Debug, Clone)]
pub struct ServiceMessage {
    /// The sender's display name (after alias resolution)
    pub sender: String,
    /// The original sender ID (protocol-specific)
    pub sender_id: String,
    /// The message content
    pub content: String,
    /// Source service name
    pub source_service: String,
    /// Source channel identifier
    pub source_channel: String,
}

/// Trait that all chat service implementations must implement
#[async_trait]
pub trait Service: Send + Sync {
    /// Connect to the service and authenticate
    async fn connect(&mut self) -> Result<()>;
    
    /// Start listening for incoming messages
    /// Returns a receiver channel for messages from this service
    async fn start(&mut self, tx: mpsc::Sender<ServiceMessage>) -> Result<()>;
    
    /// Send a message to a specific channel on this service
    async fn send_message(&self, channel: &str, message: &ServiceMessage) -> Result<()>;
    
    /// Get the service name
    fn service_name(&self) -> &str;
    
    /// Disconnect from the service
    async fn disconnect(&mut self) -> Result<()>;
}

// Re-export service implementations
pub mod matrix;
pub mod whatsapp;
pub mod discord;
