use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use anyhow::Result;

/// Main configuration structure
#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    /// Map of service name to service configuration
    pub services: HashMap<String, ServiceConfig>,
    /// Map of bridge name to bridge configuration
    pub bridges: HashMap<String, Vec<ChannelConfig>>,
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
    pub debug: bool,
}

/// WhatsApp-specific service configuration
#[derive(Debug, Deserialize, Clone)]
pub struct WhatsAppServiceConfig {
    #[serde(default)]
    pub debug: bool,
    #[serde(default = "default_whatsapp_db")]
    pub database_path: String,
}

fn default_whatsapp_db() -> String {
    "whatsapp.db".to_string()
}

/// Discord-specific service configuration
#[derive(Debug, Deserialize, Clone)]
pub struct DiscordServiceConfig {
    #[serde(default)]
    pub debug: bool,
    pub token: Option<String>,
}

/// Channel configuration within a bridge
#[derive(Debug, Deserialize, Clone)]
pub struct ChannelConfig {
    /// Service name (references a key in Config.services)
    pub service: String,
    /// Channel/room identifier (protocol-specific format)
    pub channel: String,
    /// Whether to bridge messages sent by this service's own user
    #[serde(default)]
    pub bridge_own_messages: bool,
    /// Whether to include display names in forwarded messages
    #[serde(default = "default_true")]
    pub display_names: bool,
    /// User aliases: maps user IDs to custom display names
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
    pub fn get_service(&self, name: &str) -> Option<&ServiceConfig> {
        self.services.get(name)
    }
}
