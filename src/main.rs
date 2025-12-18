#![recursion_limit = "256"]
use anyhow::Result;
use log::{info, error};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

mod config;
mod services;
mod bridge;
mod persistence;

use config::{Config, ServiceConfig};
use services::{Service, matrix::MatrixService, whatsapp::WhatsAppService, discord::DiscordService};
use bridge::BridgeCoordinator;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logger with filters to reduce noise
    let mut builder = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"));
    
    // Silence noisy libraries
    builder.filter_module("whatsapp_rust", log::LevelFilter::Warn);
    builder.filter_module("whatsapp_rust_tokio_transport", log::LevelFilter::Warn);
    builder.filter_module("whatsapp_rust_ureq_http_client", log::LevelFilter::Warn);
    builder.filter_module("matrix_sdk_base", log::LevelFilter::Warn);
    builder.filter_module("matrix_sdk_crypto", log::LevelFilter::Warn);
    builder.filter_module("ruma_common", log::LevelFilter::Warn);
    builder.filter_module("tracing", log::LevelFilter::Warn);
    builder.filter_module("serenity", log::LevelFilter::Warn);
    
    builder.init();

    // Load configuration
    let config = Config::load("data/config.yaml")?;
    
    info!("Loaded configuration with {} services and {} bridges", 
        config.services.len(), config.bridges.len());

    // Initialize all services
    let mut services: HashMap<String, Arc<Mutex<Box<dyn Service>>>> = HashMap::new();

    for (service_name, service_config) in &config.services {
        info!("Initializing service: {} ({:?})", service_name, 
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
            ServiceConfig::Discord(cfg) => {
                Box::new(DiscordService::new(service_name.clone(), cfg.clone()))
            }
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
        }

        services.insert(service_name.clone(), service);
    }

    // Log bridge configuration
    for (bridge_name, channels) in &config.bridges {
        info!("Bridge '{}' connects {} channels:", bridge_name, channels.len());
        for channel in channels {
            info!("  - {}:{}", channel.service, channel.channel);
        }
    }

    // Create and start bridge coordinator
    let coordinator = BridgeCoordinator::new(config, services);
    coordinator?.start().await?;

    info!("All services started. Bridge is running...");

    // Wait for Ctrl-C
    tokio::signal::ctrl_c().await?;
    info!("Ctrl-C received, shutting down...");

    Ok(())
}
