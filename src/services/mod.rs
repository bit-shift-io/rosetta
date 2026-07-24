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
    /// Whether this message was sent by the bot itself on the source service
    pub is_own: bool,
}

// Re-export service implementations
pub mod builder;
pub mod discord;
pub mod matrix;
pub mod registry;
pub mod traits;
pub mod whatsapp;

// Re-export formatters
pub mod formatter {
    pub use crate::services::discord::formatter::DiscordFormatter;
    pub use crate::services::matrix::formatter::MatrixFormatter;
    pub use crate::services::whatsapp::formatter::WhatsAppFormatter;
}

// Re-export fine-grained traits
pub use crate::services::traits::{
    Connectable, MandatoryService, MemberLister, MessageEditor, MessageSender, OptionalEditor,
    OptionalMembers, OptionalRoomNameFetcher, ReactionSender, RoomNameFetcher, ServiceInfo,
};

// Re-export ServiceBuilder and ServiceRegistry
pub use crate::services::builder::ServiceBuilder;
pub use crate::services::registry::ServiceRegistry;

/// Represents an update to an existing message (edit)
#[derive(Debug, Clone)]
pub struct ServiceUpdate {
    pub source_service: String,
    pub source_channel: String,
    pub source_id: String,
    pub new_content: String,
    pub is_own: bool,
}

/// Represents a reaction to a message
#[derive(Debug, Clone)]
pub struct ServiceReaction {
    pub source_service: String,
    pub source_channel: String,
    pub source_message_id: String,
    pub _sender: String,
    pub emoji: String,
    /// Whether this reaction was sent by the bot itself on the source service
    pub is_own: bool,
}

/// Enum covering all events a service can emit
#[derive(Debug, Clone)]
pub enum ServiceEvent {
    NewMessage(ServiceMessage),
    UpdateMessage(ServiceUpdate),
    NewReaction(ServiceReaction),
    // Potential future events: DeleteMessage, UserJoin, etc.
}
