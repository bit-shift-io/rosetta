use anyhow::Result;
use log::{debug, info, warn};
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

use crate::config::GifProviderConfig;
use crate::gif::providers::{
    FallbackScraper, GiphyProvider, ImgurProvider, KlipyProvider, TenorProvider,
};
use crate::gif::providers::{GifProvider, ResolvedGif};

/// Main GIF resolver that chains providers
/// Tries providers in order: Tenor API -> Giphy API -> Fallback Scraper
pub struct GifResolver {
    providers: Arc<Mutex<Vec<Box<dyn GifProvider>>>>,
    fallback: Option<FallbackScraper>,
    debug: bool,
    max_upload_size: Mutex<Option<u64>>,
}

impl GifResolver {
    /// Create a new GifResolver from config
    pub fn new(
        config: &GifProviderConfig,
        media_whitelist: Vec<String>,
        debug: bool,
        max_upload_size: Option<u64>,
    ) -> Self {
        let mut providers: Vec<Box<dyn GifProvider>> = Vec::new();

        // Add Tenor provider if API key configured
        if let Some(api_key) = &config.tenor_api_key {
            if !api_key.is_empty() {
                match TenorProvider::new(api_key.clone(), None) {
                    Ok(p) => {
                        log::info!("[GifResolver] Tenor provider enabled");
                        providers.push(Box::new(p));
                    }
                    Err(e) => log::warn!("[GifResolver] Failed to create Tenor provider: {}", e),
                }
            }
        } else {
            log::debug!("[GifResolver] Tenor API key not configured");
        }

        // Add Giphy provider if API key configured
        if let Some(api_key) = &config.giphy_api_key {
            if !api_key.is_empty() {
                match GiphyProvider::new(api_key.clone()) {
                    Ok(p) => {
                        log::info!("[GifResolver] Giphy provider enabled");
                        providers.push(Box::new(p));
                    }
                    Err(e) => log::warn!("[GifResolver] Failed to create Giphy provider: {}", e),
                }
            }
        } else {
            log::debug!("[GifResolver] Giphy API key not configured");
        }

        // Add Klipy provider if API key configured
        if let Some(api_key) = &config.klipy_api_key {
            if !api_key.is_empty() {
                match KlipyProvider::new(api_key.clone()) {
                    Ok(mut p) => {
                        if let Some(max_size) = max_upload_size {
                            p.set_max_upload_size(max_size);
                        }
                        log::info!("[GifResolver] Klipy provider enabled");
                        providers.push(Box::new(p));
                    }
                    Err(e) => log::warn!("[GifResolver] Failed to create Klipy provider: {}", e),
                }
            }
        } else {
            log::debug!("[GifResolver] Klipy API key not configured");
        }

        // Add Imgur provider if API key configured
        if let Some(api_key) = &config.imgur_client_id {
            if !api_key.is_empty() {
                match ImgurProvider::new(api_key.clone()) {
                    Ok(p) => {
                        log::info!("[GifResolver] Imgur provider enabled");
                        providers.push(Box::new(p));
                    }
                    Err(e) => log::warn!("[GifResolver] Failed to create Imgur provider: {}", e),
                }
            }
        } else {
            log::debug!("[GifResolver] Imgur client ID not configured");
        }

        // Always add fallback scraper if whitelist is not empty
        let fallback = if !media_whitelist.is_empty() {
            log::info!(
                "[GifResolver] Fallback scraper enabled for domains: {:?}",
                media_whitelist
            );
            Some(FallbackScraper::new(media_whitelist))
        } else {
            log::debug!("[GifResolver] Fallback scraper disabled (no whitelist)");
            None
        };

        Self {
            providers: Arc::new(Mutex::new(providers)),
            fallback,
            debug,
            max_upload_size: Mutex::new(max_upload_size),
        }
    }

    /// Set the maximum upload size (from Matrix homeserver config)
    pub async fn set_max_upload_size(&self, max_bytes: u64) {
        info!(
            "[GifResolver] set_max_upload_size called with {} bytes ({:.1} MB)",
            max_bytes,
            max_bytes as f64 / 1_048_576.0
        );
        *self.max_upload_size.lock().await = Some(max_bytes);
        let mut providers = self.providers.lock().await;
        info!("[GifResolver] Propagating to {} providers", providers.len());
        for provider in providers.iter_mut() {
            info!("[GifResolver] Calling set_max_upload_size on provider: {}", provider.name());
            provider.set_max_upload_size(max_bytes);
        }
        info!(
            "[GifResolver] Max upload size set to {} bytes ({:.1} MB)",
            max_bytes,
            max_bytes as f64 / 1_048_576.0
        );
    }

    /// Resolve a URL to direct media using available providers
    pub async fn resolve(&self, url: &str) -> Result<Option<ResolvedGif>> {
        let domain = Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
            .unwrap_or_default();

        // Try each provider in order
        let providers = self.providers.lock().await;
        for provider in providers.iter() {
            if provider.supports_domain(&domain) {
                info!(
                    "[GifResolver] Trying provider '{}' for {}",
                    provider.name(),
                    url
                );
                match provider.resolve(url).await {
                    Ok(Some(result)) => {
                        info!(
                            "[GifResolver] Resolved via '{}': {} -> {}",
                            provider.name(),
                            url,
                            result.url
                        );
                        return Ok(Some(result));
                    }
                    Ok(None) => {
                        info!(
                            "[GifResolver] Provider '{}' could not resolve {}",
                            provider.name(),
                            url
                        );
                    }
                    Err(e) => {
                        log::warn!(
                            "[GifResolver] Provider '{}' error for {}: {}",
                            provider.name(),
                            url,
                            e
                        );
                        // Continue to next provider
                    }
                }
            }
        }

        // Try fallback scraper
        if let Some(fallback) = &self.fallback {
            if fallback.supports_domain(&domain) {
                info!("[GifResolver] Trying fallback scraper for {}", url);
                match fallback.resolve(url).await {
                    Ok(Some(result)) => {
                        if self.debug {
                            log::info!(
                                "[GifResolver] Resolved via fallback: {} -> {}",
                                url,
                                result.url
                            );
                        }
                        return Ok(Some(result));
                    }
                    Ok(None) => {
                        if self.debug {
                            log::debug!("[GifResolver] Fallback could not resolve {}", url);
                        }
                    }
                    Err(e) => {
                        log::warn!("[GifResolver] Fallback error for {}: {}", url, e);
                    }
                }
            }
        }

        if self.debug {
            log::debug!("[GifResolver] No provider could resolve: {}", url);
        }
        Ok(None)
    }

    /// Check if any provider supports the given domain
    pub fn supports_domain(&self, domain: &str) -> bool {
        let providers = self.providers.blocking_lock();
        providers.iter().any(|p| p.supports_domain(domain))
            || self
                .fallback
                .as_ref()
                .map(|f| f.supports_domain(domain))
                .unwrap_or(false)
    }
}
