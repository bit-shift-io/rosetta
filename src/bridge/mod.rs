pub mod alias_resolver;
pub mod bridge;
pub mod deduplicator;
pub mod dispatcher;
pub mod edit_handler;
pub mod formatter;
pub mod matcher;
pub mod media;
pub mod reaction_handler;
pub mod router;
pub mod status_handler;

pub use crate::bridge::bridge::BridgeCoordinator;
