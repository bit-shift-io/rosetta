use anyhow::Result;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;

/// Main configuration structure
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Map of service name to service configuration
    pub services: HashMap<String, ServiceConfig>,
    /// Map of bridge name to bridge configuration
    pub bridges: HashMap<String, Vec<ChannelConfig>>,
    /// Global media configuration
    #[serde(default)]
    pub media: Option<MediaConfig>,
}

/// Media configuration for GIF providers and size limits
#[derive(Debug, Deserialize, Clone, Default)]
pub struct MediaConfig {
    /// Maximum media upload size in megabytes
    #[serde(default)]
    pub max_size_mb: u64,
    /// Map of provider name to provider configuration
    #[serde(default)]
    pub gif_providers: HashMap<String, GifProviderEntry>,
}

/// Single GIF provider configuration entry
#[derive(Debug, Deserialize, Clone)]
pub struct GifProviderEntry {
    /// Whether this provider is enabled
    #[serde(default = "default_false")]
    pub enabled: bool,
    /// API key for the provider
    #[serde(default)]
    pub api_key: String,
}

fn default_false() -> bool {
    false
}

/// Service configuration - tagged by protocol type
#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "protocol", rename_all = "lowercase")]
pub enum ServiceConfig {
    Matrix(MatrixServiceConfig),
    WhatsApp(WhatsAppServiceConfig),
    Discord(DiscordServiceConfig),
}

/// Matrix-specific service configuration
#[derive(Debug, Deserialize, Clone)]
pub struct MatrixServiceConfig {
    pub homeserver_url: String,
    pub username: String,
    pub password: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub device_id: Option<String>,
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub display_name: Option<String>,
}

/// WhatsApp-specific service configuration
#[derive(Debug, Deserialize, Clone)]
pub struct WhatsAppServiceConfig {
    #[serde(default)]
    pub session_path: Option<String>,
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub display_name: Option<String>,
}

/// Discord-specific service configuration
#[derive(Debug, Deserialize, Clone)]
pub struct DiscordServiceConfig {
    pub bot_token: String,
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub display_name: Option<String>,
}

/// Channel configuration within a bridge
#[derive(Debug, Deserialize, Clone)]
pub struct ChannelConfig {
    /// Service name (references a key in Config.services)
    pub service: String,
    /// Channel/room identifier (protocol-specific format)
    pub channel: String,
    /// Optional room name for the room/channel (populated from service API on connect, shown in status messages)
    #[serde(default)]
    pub room_name: Option<String>,
    /// Whether to bridge display names in forwarded messages
    #[serde(default = "default_true")]
    pub display_names: bool,
    /// Whether to enable media bridging (images, etc.) for this channel
    #[serde(default = "default_true")]
    pub enable_media: bool,
    /// Whether to bridge messages sent by the bot/service-user itself in this bridge
    #[serde(default)]
    pub bridge_own_messages: bool,
    /// Map of User ID -> Display Name aliases
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

fn default_true() -> bool {
    true
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        Ok(config)
    }

    /// Get a service configuration by name
    #[allow(dead_code)]
    pub fn get_service(&self, name: &str) -> Option<&ServiceConfig> {
        self.services.get(name)
    }
}
