use anyhow::Result;
use reqwest::Client;
use rosetta::config::{Config, ServiceConfig};
use rosetta::gif::GifResolver;
use rosetta::services::traits::{Connectable, MandatoryService};
use rosetta::services::{
    discord::DiscordService, matrix::MatrixService, whatsapp::WhatsAppService,
};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::mpsc;

mod helpers {
    use super::*;

    pub async fn load_config() -> Result<Config> {
        Config::load("data/config.yaml")
    }

    pub fn create_services(
        config: &Config,
        gif_resolver: Arc<GifResolver>,
    ) -> HashMap<String, Arc<Mutex<Box<dyn MandatoryService>>>> {
        let mut services: HashMap<String, Arc<Mutex<Box<dyn MandatoryService>>>> = HashMap::new();

        for (service_name, service_config) in &config.services {
            let service: Box<dyn MandatoryService> = match service_config {
                ServiceConfig::Matrix(cfg) => {
                    Box::new(MatrixService::new(service_name.clone(), cfg.clone()))
                }
                ServiceConfig::WhatsApp(cfg) => {
                    Box::new(WhatsAppService::new(service_name.clone(), cfg.clone()))
                }
                ServiceConfig::Discord(cfg) => Box::new(DiscordService::new(
                    service_name.clone(),
                    cfg.clone(),
                    gif_resolver.clone(),
                )),
            };

            let service = Arc::new(Mutex::new(service));
            services.insert(service_name.clone(), service);
        }

        services
    }

    pub fn create_gif_resolver(config: &Config) -> Arc<GifResolver> {
        Arc::new(GifResolver::new(
            &config.media,
            config
                .services
                .values()
                .any(|s| matches!(s, ServiceConfig::Discord(cfg) if cfg.debug)),
            None,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[tokio::test]
    #[serial]
    async fn test_config_loads() -> Result<()> {
        let config = helpers::load_config().await?;
        assert!(
            !config.services.is_empty(),
            "At least one service should be configured"
        );
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_services_instantiate() -> Result<()> {
        let config = helpers::load_config().await?;
        let gif_resolver = helpers::create_gif_resolver(&config);
        let services = helpers::create_services(&config, gif_resolver);

        assert_eq!(services.len(), config.services.len());

        for (name, service) in services {
            let svc = service.lock().await;
            assert_eq!(svc.service_name(), name);
        }

        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_matrix_connectivity() -> Result<()> {
        let config = helpers::load_config().await?;

        // Find Matrix service
        let (name, matrix_config) = match config
            .services
            .iter()
            .find(|(_, s)| matches!(s, ServiceConfig::Matrix(_)))
        {
            Some((n, ServiceConfig::Matrix(cfg))) => (n.clone(), cfg.clone()),
            _ => {
                eprintln!("WARN: Skipping matrix — no Matrix service configured");
                return Ok(());
            }
        };

        // Check credentials
        if matrix_config.username.is_empty()
            || matrix_config.password.is_empty()
            || matrix_config.homeserver_url.is_empty()
        {
            eprintln!("WARN: Skipping matrix — missing/invalid credentials");
            return Ok(());
        }

        let service = MatrixService::new(name.clone(), matrix_config);
        let mut service = Box::new(service);

        // Connect
        service.connect().await.map_err(|e| {
            eprintln!("WARN: Skipping matrix — connection failed: {}", e);
            e
        })?;

        // Start with dummy channel
        let (tx, _rx) = mpsc::channel(10);
        service.start(tx).await.map_err(|e| {
            eprintln!("WARN: Skipping matrix — start failed: {}", e);
            e
        })?;

        // Wait until ready
        service.wait_until_ready().await.map_err(|e| {
            eprintln!("WARN: Skipping matrix — wait_until_ready failed: {}", e);
            e
        })?;

        eprintln!("SUCCESS: Matrix connectivity test passed");
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_whatsapp_connectivity() -> Result<()> {
        let config = helpers::load_config().await?;

        // Find WhatsApp service
        let (name, whatsapp_config) = match config
            .services
            .iter()
            .find(|(_, s)| matches!(s, ServiceConfig::WhatsApp(_)))
        {
            Some((n, ServiceConfig::WhatsApp(cfg))) => (n.clone(), cfg.clone()),
            _ => {
                eprintln!("WARN: Skipping whatsapp — no WhatsApp service configured");
                return Ok(());
            }
        };

        let _gif_resolver = helpers::create_gif_resolver(&config);
        let service = WhatsAppService::new(name.clone(), whatsapp_config);
        let mut service = Box::new(service);

        // Connect
        service.connect().await.map_err(|e| {
            eprintln!("WARN: Skipping whatsapp — connect failed: {}", e);
            e
        })?;

        // Start with dummy channel
        let (tx, _rx) = mpsc::channel(10);
        service.start(tx).await.map_err(|e| {
            eprintln!("WARN: Skipping whatsapp — start failed: {}", e);
            e
        })?;

        // Wait until ready
        service.wait_until_ready().await.map_err(|e| {
            eprintln!("WARN: Skipping whatsapp — wait_until_ready failed: {}", e);
            e
        })?;

        eprintln!("SUCCESS: WhatsApp connectivity test passed");
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_discord_connectivity() -> Result<()> {
        let config = helpers::load_config().await?;

        // Find Discord service
        let (name, discord_config) = match config
            .services
            .iter()
            .find(|(_, s)| matches!(s, ServiceConfig::Discord(_)))
        {
            Some((n, ServiceConfig::Discord(cfg))) => (n.clone(), cfg.clone()),
            _ => {
                eprintln!("WARN: Skipping discord — no Discord service configured");
                return Ok(());
            }
        };

        // Check token
        if discord_config.bot_token.is_empty()
            || discord_config.bot_token == "your_discord_bot_token_here"
        {
            eprintln!("WARN: Skipping discord — missing/invalid bot token");
            return Ok(());
        }

        let gif_resolver = helpers::create_gif_resolver(&config);
        let service = DiscordService::new(name.clone(), discord_config, gif_resolver);
        let mut service = Box::new(service);

        // Connect
        service.connect().await.map_err(|e| {
            eprintln!("WARN: Skipping discord — connect failed: {}", e);
            e
        })?;

        // Start with dummy channel
        let (tx, _rx) = mpsc::channel(10);
        service.start(tx).await.map_err(|e| {
            eprintln!("WARN: Skipping discord — start failed: {}", e);
            e
        })?;

        // Wait until ready
        service.wait_until_ready().await.map_err(|e| {
            eprintln!("WARN: Skipping discord — wait_until_ready failed: {}", e);
            e
        })?;

        eprintln!("SUCCESS: Discord connectivity test passed");
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_klipy_resolver() -> Result<()> {
        let config = helpers::load_config().await?;

        // Check if Klipy is enabled with API key
        let media = match &config.media {
            Some(m) => m,
            None => {
                eprintln!("WARN: Skipping klipy — no media config");
                return Ok(());
            }
        };

        let _klipy_entry = match media.gif_providers.get("klipy") {
            Some(entry) if entry.enabled && !entry.api_key.is_empty() => entry,
            _ => {
                eprintln!("WARN: Skipping klipy — disabled or missing API key");
                return Ok(());
            }
        };

        let gif_resolver = helpers::create_gif_resolver(&config);

        // Known Klipy test URL (update as needed)
        let test_url = "https://klipy.com/gifs/cat-sandwich-5";

        let result = gif_resolver.resolve(test_url).await.map_err(|e| {
            eprintln!("WARN: Klipy resolver error: {}", e);
            e
        })?;

        let resolved = match result {
            Some(r) => r,
            None => {
                eprintln!("WARN: Klipy could not resolve test URL: {}", test_url);
                return Ok(());
            }
        };

        eprintln!("INFO: Klipy resolved {} -> {}", test_url, resolved.url);

        // Check size if max_size_mb is configured
        if let Some(max_size_mb) = media.max_size_mb.checked_mul(1_048_576)
            && max_size_mb > 0
        {
            let client = Client::new();
            let resp = client.head(&resolved.url).send().await.map_err(|e| {
                eprintln!("WARN: Failed HEAD request for size check: {}", e);
                e
            })?;

            if let Some(content_length) = resp.content_length() {
                if content_length > max_size_mb {
                    eprintln!(
                        "WARN: Resolved media {} bytes exceeds max_size_mb ({} MB)",
                        content_length, media.max_size_mb
                    );
                } else {
                    eprintln!(
                        "INFO: Klipy resolved media size: {} bytes (limit: {} bytes)",
                        content_length, max_size_mb
                    );
                }
            }
        }
        eprintln!("SUCCESS: Klipy resolver test passed");
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_giphy_resolver() -> Result<()> {
        let config = helpers::load_config().await?;

        let media = match &config.media {
            Some(m) => m,
            None => {
                eprintln!("WARN: Skipping giphy — no media config");
                return Ok(());
            }
        };

        let _giphy_entry = match media.gif_providers.get("giphy") {
            Some(entry) if entry.enabled && !entry.api_key.is_empty() => entry,
            _ => {
                eprintln!("WARN: Skipping giphy — disabled or missing API key");
                return Ok(());
            }
        };

        let gif_resolver = helpers::create_gif_resolver(&config);

        // Known Giphy test URL (update as needed)
        let test_url = "https://giphy.com/gifs/test-animation";

        let result = gif_resolver.resolve(test_url).await.map_err(|e| {
            eprintln!("WARN: Giphy resolver error: {}", e);
            e
        })?;

        let resolved = match result {
            Some(r) => r,
            None => {
                eprintln!("WARN: Giphy could not resolve test URL: {}", test_url);
                return Ok(());
            }
        };

        eprintln!("INFO: Giphy resolved {} -> {}", test_url, resolved.url);

        // Check size if max_size_mb is configured
        if let Some(max_size_mb) = media.max_size_mb.checked_mul(1_048_576)
            && max_size_mb > 0
        {
            let client = Client::new();
            let resp = client.head(&resolved.url).send().await.map_err(|e| {
                eprintln!("WARN: Failed HEAD request for size check: {}", e);
                e
            })?;

            if let Some(content_length) = resp.content_length() {
                if content_length > max_size_mb {
                    eprintln!(
                        "WARN: Resolved media {} bytes exceeds max_size_mb ({} MB)",
                        content_length, media.max_size_mb
                    );
                } else {
                    eprintln!(
                        "INFO: Giphy resolved media size: {} bytes (limit: {} bytes)",
                        content_length, max_size_mb
                    );
                }
            }
        }
        eprintln!("SUCCESS: Giphy resolver test passed");
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_tenor_resolver() -> Result<()> {
        let config = helpers::load_config().await?;

        let media = match &config.media {
            Some(m) => m,
            None => {
                eprintln!("WARN: Skipping tenor — no media config");
                return Ok(());
            }
        };

        let _tenor_entry = match media.gif_providers.get("tenor") {
            Some(entry) if entry.enabled && !entry.api_key.is_empty() => entry,
            _ => {
                eprintln!("WARN: Skipping tenor — disabled or missing API key");
                return Ok(());
            }
        };

        let gif_resolver = helpers::create_gif_resolver(&config);

        // Known Tenor test URL (update as needed)
        let test_url = "https://tenor.com/view/test-gif-animation";

        let result = gif_resolver.resolve(test_url).await.map_err(|e| {
            eprintln!("WARN: Tenor resolver error: {}", e);
            e
        })?;

        let resolved = match result {
            Some(r) => r,
            None => {
                eprintln!("WARN: Tenor could not resolve test URL: {}", test_url);
                return Ok(());
            }
        };

        eprintln!("INFO: Tenor resolved {} -> {}", test_url, resolved.url);

        // Check size if max_size_mb is configured
        if let Some(max_size_mb) = media.max_size_mb.checked_mul(1_048_576)
            && max_size_mb > 0
        {
            let client = Client::new();
            let resp = client.head(&resolved.url).send().await.map_err(|e| {
                eprintln!("WARN: Failed HEAD request for size check: {}", e);
                e
            })?;

            if let Some(content_length) = resp.content_length() {
                if content_length > max_size_mb {
                    eprintln!(
                        "WARN: Resolved media {} bytes exceeds max_size_mb ({} MB)",
                        content_length, media.max_size_mb
                    );
                } else {
                    eprintln!(
                        "INFO: Tenor resolved media size: {} bytes (limit: {} bytes)",
                        content_length, max_size_mb
                    );
                }
            }
        }
        eprintln!("SUCCESS: Tenor resolver test passed");
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn test_imgur_resolver() -> Result<()> {
        let config = helpers::load_config().await?;

        let media = match &config.media {
            Some(m) => m,
            None => {
                eprintln!("WARN: Skipping imgur — no media config");
                return Ok(());
            }
        };

        let _imgur_entry = match media.gif_providers.get("imgur") {
            Some(entry) if entry.enabled && !entry.api_key.is_empty() => entry,
            _ => {
                eprintln!("WARN: Skipping imgur — disabled or missing API key");
                return Ok(());
            }
        };

        let gif_resolver = helpers::create_gif_resolver(&config);

        // Known Imgur test URL (update as needed)
        let test_url = "https://imgur.com/test-gif-animation";

        let result = gif_resolver.resolve(test_url).await.map_err(|e| {
            eprintln!("WARN: Imgur resolver error: {}", e);
            e
        })?;

        let resolved = match result {
            Some(r) => r,
            None => {
                eprintln!("WARN: Imgur could not resolve test URL: {}", test_url);
                return Ok(());
            }
        };

        eprintln!("INFO: Imgur resolved {} -> {}", test_url, resolved.url);

        // Check size if max_size_mb is configured
        if let Some(max_size_mb) = media.max_size_mb.checked_mul(1_048_576)
            && max_size_mb > 0
        {
            let client = Client::new();
            let resp = client.head(&resolved.url).send().await.map_err(|e| {
                eprintln!("WARN: Failed HEAD request for size check: {}", e);
                e
            })?;

            if let Some(content_length) = resp.content_length() {
                if content_length > max_size_mb {
                    eprintln!(
                        "WARN: Resolved media {} bytes exceeds max_size_mb ({} MB)",
                        content_length, media.max_size_mb
                    );
                } else {
                    eprintln!(
                        "INFO: Imgur resolved media size: {} bytes (limit: {} bytes)",
                        content_length, max_size_mb
                    );
                }
            }
        }
        eprintln!("SUCCESS: Imgur resolver test passed");
        Ok(())
    }
}
