#![recursion_limit = "256"]
use anyhow::Result;
use log::{error, info};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

mod bridge;
mod config;
mod gif;
mod persistence;
mod services;

use crate::bridge::BridgeCoordinator;
use crate::config::{Config, GifProviderConfig, ServiceConfig};
use crate::gif::GifResolver;
use crate::services::{
    Service, discord::DiscordService, matrix::MatrixService, whatsapp::WhatsAppService,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger with filters to reduce noise
    let mut builder =
        env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));

    // Silence noisy libraries
    builder.filter_module("whatsapp_rust", log::LevelFilter::Warn);
    builder.filter_module("whatsapp_rust_tokio_transport", log::LevelFilter::Warn);
    builder.filter_module("whatsapp_rust_ureq_http_client", log::LevelFilter::Warn);
    builder.filter_module("wacore", log::LevelFilter::Warn);
    builder.filter_module("waproto", log::LevelFilter::Warn);
    builder.filter_module("Client", log::LevelFilter::Warn); // WhatsApp client logs
    builder.filter_module("matrix_sdk", log::LevelFilter::Warn);
    builder.filter_module("matrix_sdk_crypto", log::LevelFilter::Error);
    builder.filter_module("ruma", log::LevelFilter::Warn);
    builder.filter_module("tracing", log::LevelFilter::Warn);
    builder.filter_module("serenity", log::LevelFilter::Warn);
    builder.filter_module("h2", log::LevelFilter::Warn);
    builder.filter_module("hyper", log::LevelFilter::Warn);
    builder.filter_module("rustls", log::LevelFilter::Warn);
    builder.filter_module("reqwest", log::LevelFilter::Warn);

    builder.init();

    // Load configuration
    let config = Config::load("data/config.yaml")?;

    let gif_resolver = Arc::new(GifResolver::new(
        &config.gif_providers,
        config.media_whitelist.clone(),
        config
            .services
            .values()
            .any(|s| matches!(s, ServiceConfig::Discord(cfg) if cfg.debug)),
        None, // Will be set after Matrix connects
    ));

    info!(
        "Loaded configuration with {} services and {} bridges",
        config.services.len(),
        config.bridges.len()
    );

    // Initialize all services
    let mut services: HashMap<String, Arc<Mutex<Box<dyn Service>>>> = HashMap::new();

    for (service_name, service_config) in &config.services {
        info!(
            "Initializing service: {} ({:?})",
            service_name,
            match service_config {
                ServiceConfig::Matrix(_) => "Matrix",
                ServiceConfig::WhatsApp(_) => "WhatsApp",
                ServiceConfig::Discord(_) => "Discord",
            }
        );

        let service: Box<dyn Service> = match service_config {
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

        // Connect to the service
        {
            let mut svc = service.lock().await;
            match svc.connect().await {
                Ok(_) => info!("Successfully connected to service: {}", service_name),
                Err(e) => {
                    error!("Failed to connect to service {}: {}", service_name, e);
                    // For Discord (not implemented), we'll skip it but continue
                    if matches!(service_config, ServiceConfig::Discord(_)) {
                        continue;
                    }
                    return Err(e);
                }
            }

            // If this is a Matrix service, get the max upload size and set it on the GifResolver
            if let Some(matrix_svc) = svc.as_any().downcast_ref::<MatrixService>() {
                if let Some(max_size) = matrix_svc.max_upload_size() {
                    gif_resolver.set_max_upload_size(max_size).await;
                    info!("Set max upload size on GifResolver: {} bytes", max_size);
                }
            }
        }

        services.insert(service_name.clone(), service);
    }

    // Log bridge configuration
    for (bridge_name, channels) in &config.bridges {
        info!(
            "Bridge '{}' connects {} channels:",
            bridge_name,
            channels.len()
        );
        for channel in channels {
            info!("  - {}:{}", channel.service, channel.channel);
        }
    }

    // Create and start bridge coordinator
    let coordinator = BridgeCoordinator::new(config, services)?;
    info!("All services started. Bridge is running...");

    // Run coordinator and wait for Ctrl-C
    tokio::select! {
        res = coordinator.start() => {
            if let Err(e) = res {
                error!("Bridge coordinator crashed: {}", e);
                return Err(e);
            }
        },
        _ = tokio::signal::ctrl_c() => {
             info!("Ctrl-C received, shutting down...");
             coordinator.shutdown().await;
        }
    }

    Ok(())
}
