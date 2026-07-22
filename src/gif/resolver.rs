use anyhow::Result;
use log::{debug, info, warn};
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use url::Url;

use crate::config::MediaConfig;
use crate::gif::providers::{
    FallbackScraper, GiphyProvider, ImgurProvider, KlipyProvider, TenorProvider,
};
use crate::gif::providers::{GifProvider, ResolvedGif};

/// Main GIF resolver that chains providers
/// Tries providers in order: Tenor API -> Giphy API -> Klipy API -> Imgur API -> Fallback Scraper
pub struct GifResolver {
    providers: Arc<Mutex<Vec<Box<dyn GifProvider>>>>,
    fallback: Option<FallbackScraper>,
    debug: bool,
    max_upload_size: Mutex<Option<u64>>,
    config_max_size_mb: Option<u64>,
}

impl GifResolver {
    /// Create a new GifResolver from MediaConfig
    pub fn new(
        media_config: &Option<MediaConfig>,
        debug: bool,
        max_upload_size: Option<u64>,
    ) -> Self {
        let mut providers: Vec<Box<dyn GifProvider>> = Vec::new();
        let mut fallback_domains: HashSet<String> = HashSet::new();

        // Get the initial max upload size (from config or passed directly)
        let initial_max_size =
            max_upload_size.or_else(|| media_config.as_ref().map(|m| m.max_size_mb * 1_048_576));

        // Store config max_size_mb for override logging
        let config_max_size_mb = media_config
            .as_ref()
            .map(|m| m.max_size_mb)
            .filter(|&s| s > 0);

        if let Some(media_config) = media_config {
            // Iterate over enabled providers
            for (name, entry) in &media_config.gif_providers {
                if !entry.enabled {
                    debug!("[GifResolver] Provider '{}' is disabled, skipping", name);
                    continue;
                }

                if entry.api_key.is_empty() {
                    debug!(
                        "[GifResolver] Provider '{}' has empty API key, skipping",
                        name
                    );
                    continue;
                }

                match name.as_str() {
                    "tenor" => match TenorProvider::new(entry.api_key.clone(), None) {
                        Ok(mut p) => {
                            if let Some(max_size) = initial_max_size {
                                p.set_max_upload_size(max_size);
                            }
                            info!("[GifResolver] Tenor provider enabled");
                            providers.push(Box::new(p));
                            fallback_domains.insert("tenor.com".to_string());
                            fallback_domains.insert("tenor.co".to_string());
                        }
                        Err(e) => warn!("[GifResolver] Failed to create Tenor provider: {}", e),
                    },
                    "giphy" => match GiphyProvider::new(entry.api_key.clone()) {
                        Ok(mut p) => {
                            if let Some(max_size) = initial_max_size {
                                p.set_max_upload_size(max_size);
                            }
                            info!("[GifResolver] Giphy provider enabled");
                            providers.push(Box::new(p));
                            fallback_domains.insert("giphy.com".to_string());
                            fallback_domains.insert("gph.is".to_string());
                            fallback_domains.insert("media.giphy.com".to_string());
                        }
                        Err(e) => warn!("[GifResolver] Failed to create Giphy provider: {}", e),
                    },
                    "klipy" => match KlipyProvider::new(entry.api_key.clone()) {
                        Ok(mut p) => {
                            if let Some(max_size) = initial_max_size {
                                p.set_max_upload_size(max_size);
                            }
                            info!("[GifResolver] Klipy provider enabled");
                            providers.push(Box::new(p));
                            fallback_domains.insert("klipy.com".to_string());
                        }
                        Err(e) => warn!("[GifResolver] Failed to create Klipy provider: {}", e),
                    },
                    "imgur" => match ImgurProvider::new(entry.api_key.clone()) {
                        Ok(mut p) => {
                            if let Some(max_size) = initial_max_size {
                                p.set_max_upload_size(max_size);
                            }
                            info!("[GifResolver] Imgur provider enabled");
                            providers.push(Box::new(p));
                            fallback_domains.insert("imgur.com".to_string());
                            fallback_domains.insert("i.imgur.com".to_string());
                        }
                        Err(e) => warn!("[GifResolver] Failed to create Imgur provider: {}", e),
                    },
                    unknown => {
                        warn!("[GifResolver] Unknown provider '{}', skipping", unknown);
                    }
                }
            }
        } else {
            debug!("[GifResolver] No media config provided");
        }

        // Always add fallback scraper if we have any domains
        let fallback = if !fallback_domains.is_empty() {
            let whitelist: Vec<String> = fallback_domains.into_iter().collect();
            info!(
                "[GifResolver] Fallback scraper enabled for domains: {:?}",
                whitelist
            );
            Some(FallbackScraper::new(whitelist))
        } else {
            debug!("[GifResolver] Fallback scraper disabled (no provider domains)");
            None
        };

        Self {
            providers: Arc::new(Mutex::new(providers)),
            fallback,
            debug,
            max_upload_size: Mutex::new(initial_max_size),
            config_max_size_mb,
        }
    }

    /// Set the maximum upload size (from Matrix homeserver config)
    pub async fn set_max_upload_size(&self, max_bytes: u64) {
        let max_mb = max_bytes as f64 / 1_048_576.0;

        // Log if config's max_size_mb is being overridden by homeserver value
        if let Some(config_mb) = self.config_max_size_mb {
            let config_bytes = config_mb * 1_048_576;
            if max_bytes != config_bytes {
                info!(
                    "[GifResolver] Config max_size_mb ({} MB = {} bytes) overridden by homeserver value ({} bytes = {:.1} MB)",
                    config_mb, config_bytes, max_bytes, max_mb
                );
            }
        }

        *self.max_upload_size.lock().await = Some(max_bytes);
        let mut providers = self.providers.lock().await;
        for provider in providers.iter_mut() {
            provider.set_max_upload_size(max_bytes);
        }
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
                        warn!(
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
                            info!(
                                "[GifResolver] Resolved via fallback: {} -> {}",
                                url, result.url
                            );
                        }
                        return Ok(Some(result));
                    }
                    Ok(None) => {
                        if self.debug {
                            debug!("[GifResolver] Fallback could not resolve {}", url);
                        }
                    }
                    Err(e) => {
                        warn!("[GifResolver] Fallback error for {}: {}", url, e);
                    }
                }
            }
        }

        if self.debug {
            debug!("[GifResolver] No provider could resolve: {}", url);
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
