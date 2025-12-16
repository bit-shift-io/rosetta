use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use anyhow::Result;

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    pub matrix: MatrixConfig,
    pub whatsapp: WhatsappConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct MatrixConfig {
    pub homeserver_url: String,
    pub username: String,
    pub password: String,
    pub room_id: String,
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct WhatsappConfig {
    pub target_id: String,
    #[serde(default)]
    pub debug: bool,
    #[serde(default)]
    pub bridge_own_messages: bool,
    #[serde(default)]
    pub aliases: HashMap<String, String>,
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&content)?;
        return Ok(config);
    }
}
