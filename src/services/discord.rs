use async_trait::async_trait;
use anyhow::Result;
use tokio::sync::mpsc;

use crate::services::{Service, ServiceMessage};
use crate::config::DiscordServiceConfig;

/// Discord service stub - placeholder for future implementation
pub struct DiscordService {
    name: String,
    config: DiscordServiceConfig,
}

impl DiscordService {
    pub fn new(name: String, config: DiscordServiceConfig) -> Self {
        Self {
            name,
            config,
        }
    }
}

#[async_trait]
impl Service for DiscordService {
    async fn connect(&mut self) -> Result<()> {
        Err(anyhow::anyhow!("[Discord:{}] Not yet implemented. Contributions welcome!", self.name))
    }

    async fn start(&mut self, _tx: mpsc::Sender<ServiceMessage>) -> Result<()> {
        Err(anyhow::anyhow!("[Discord:{}] Not yet implemented. Contributions welcome!", self.name))
    }

    async fn send_message(&self, _channel: &str, _message: &ServiceMessage) -> Result<()> {
        Err(anyhow::anyhow!("[Discord:{}] Not yet implemented. Contributions welcome!", self.name))
    }

    fn service_name(&self) -> &str {
        &self.name
    }

    async fn disconnect(&mut self) -> Result<()> {
        Ok(())
    }
}
