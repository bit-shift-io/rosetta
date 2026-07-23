#![recursion_limit = "256"]
use anyhow::Result;
use log::{error, info};
use std::sync::Arc;
use tokio::sync::mpsc;

use rosetta::bridge;
use rosetta::config::{Config, ServiceConfig};
use rosetta::gif::GifResolver;
use rosetta::services::{Service, ServiceBuilder, ServiceRegistry};

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
    builder.filter_module("ruma_common::api::path_builder", log::LevelFilter::Error);
    builder.filter_module("tracing", log::LevelFilter::Warn);
    builder.filter_module("serenity", log::LevelFilter::Warn);
    builder.filter_module("h2", log::LevelFilter::Warn);
    builder.filter_module("hyper", log::LevelFilter::Warn);
    builder.filter_module("rustls", log::LevelFilter::Warn);
    builder.filter_module("reqwest", log::LevelFilter::Warn);

    builder.init();

    // Load configuration
    let config = Config::load("data/config.yaml")?;

    // Create GifResolver
    let gif_resolver = Arc::new(GifResolver::new(
        &config.media,
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

    // Build all services using ServiceBuilder
    let service_builder = ServiceBuilder::new(gif_resolver.clone());
    let mut services = service_builder.build_all(&config)?;

    // Update GifResolver with Matrix max upload size
    service_builder
        .update_gif_resolver_from_matrix(&services)
        .await?;

    // Create registry and register services
    let mut registry = ServiceRegistry::new();
    for (name, service) in services {
        registry.register(name, service);
    }

    // Connect all services
    registry.connect_all().await?;

    // Update GifResolver again after Matrix connects
    if let Some((_, matrix_svc)) = registry.services().iter().find(|(name, _)| {
        config
            .services
            .get(*name)
            .map(|cfg| matches!(cfg, ServiceConfig::Matrix(_)))
            .unwrap_or(false)
    }) {
        let svc = matrix_svc.lock().await;
        if let Some(matrix_svc) = svc
            .as_any()
            .downcast_ref::<rosetta::services::matrix::MatrixService>()
        {
            if let Some(max_size) = matrix_svc.max_upload_size() {
                gif_resolver.set_max_upload_size(max_size).await;
            }
        }
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

    // Create event channel and start all services
    let (tx, _rx) = mpsc::channel::<rosetta::services::ServiceEvent>(100);
    registry.start_all(tx.clone()).await?;

    // Wait for all services to be ready
    registry.wait_all_ready().await?;

    // Create and start bridge coordinator
    let coordinator = bridge::BridgeCoordinator::new(config, registry.services().clone())?;
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
             registry.shutdown_all().await;
        }
    }

    Ok(())
}
