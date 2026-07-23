use anyhow::Result;
use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;
use std::sync::Arc;
use url::Url;

use super::GifProvider;
use crate::gif::ResolvedGif;

/// Fallback HTML scraper for GIF sites without API keys
/// Extracted from discord.rs original scraping logic
pub struct FallbackScraper {
    client: Arc<Client>,
    whitelist: Vec<String>,
    og_image_regex: Regex,
}

impl FallbackScraper {
    pub fn new(whitelist: Vec<String>) -> Self {
        let client = Arc::new(Client::builder()
            .user_agent("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .unwrap_or_default());

        // Regex to find og:image meta tag
        let og_image_regex =
            Regex::new(r#"property=["']og:image["'][^>]*?content=["']([^"']+)["']"#).unwrap();

        Self {
            client,
            whitelist,
            og_image_regex,
        }
    }

    /// Check if domain is in whitelist
    fn is_allowed(&self, domain: &str) -> bool {
        self.whitelist
            .iter()
            .any(|w| domain == w || domain.ends_with(&format!(".{}", w)))
    }

    /// Extract domain from URL
    fn extract_domain(&self, url: &str) -> Option<String> {
        Url::parse(url)
            .ok()
            .and_then(|u| u.host_str().map(|s| s.to_string()))
    }
}

#[async_trait]
impl GifProvider for FallbackScraper {
    fn name(&self) -> &str {
        "fallback"
    }

    fn supports_domain(&self, domain: &str) -> bool {
        self.is_allowed(domain)
    }

    async fn resolve(&self, url: &str) -> Result<Option<ResolvedGif>> {
        let domain = match self.extract_domain(url) {
            Some(d) => d,
            None => return Ok(None),
        };

        if !self.is_allowed(&domain) {
            return Ok(None);
        }

        log::debug!("[Fallback] Attempting to scrape: {}", url);

        // First try HEAD to check content type
        let head_resp = match self.client.head(url).send().await {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };

        let content_type = head_resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        // If it's already a direct image/video, download it
        if content_type.starts_with("image/") || content_type.starts_with("video/") {
            return self.download_direct(url, &content_type).await;
        }

        // If HTML, try to find og:image
        if content_type.starts_with("text/html") {
            return self.scrape_og_image(url).await;
        }

        Ok(None)
    }
}

impl FallbackScraper {
    /// Download direct media URL
    async fn download_direct(&self, url: &str, mime_type: &str) -> Result<Option<ResolvedGif>> {
        let resp = match self.client.get(url).send().await {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };

        let _bytes = match resp.bytes().await {
            Ok(b) => b,
            Err(_) => return Ok(None),
        };

        let filename = Url::parse(url)
            .ok()
            .and_then(|u| {
                u.path_segments()
                    .map(|mut seg| {
                        seg.next_back()
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                    })
                    .flatten()
            })
            .unwrap_or_else(|| {
                let ext = if mime_type.contains("gif") {
                    "gif"
                } else if mime_type.contains("mp4") {
                    "mp4"
                } else if mime_type.contains("webm") {
                    "webm"
                } else {
                    "bin"
                };
                format!("media.{}", ext)
            });

        Ok(Some(ResolvedGif {
            url: url.to_string(),
            mime_type: mime_type.to_string(),
            filename,
            provider: "fallback".to_string(),
        }))
    }

    /// Scrape HTML page for og:image
    async fn scrape_og_image(&self, url: &str) -> Result<Option<ResolvedGif>> {
        let resp = match self.client.get(url).send().await {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };

        let text = match resp.text().await {
            Ok(t) => t,
            Err(_) => return Ok(None),
        };

        // Find og:image
        if let Some(caps) = self.og_image_regex.captures(&text) {
            if let Some(og_url) = caps.get(1) {
                let og_url = og_url.as_str();

                // Resolve relative URLs
                let absolute_url = Url::parse(og_url)
                    .map(|u| u.to_string())
                    .unwrap_or_else(|_| {
                        Url::parse(url)
                            .ok()
                            .and_then(|base| base.join(og_url).ok())
                            .map(|u| u.to_string())
                            .unwrap_or_else(|| og_url.to_string())
                    });

                // Try to download the og:image
                return self.download_og_image(&absolute_url).await;
            }
        }

        Ok(None)
    }

    /// Download og:image and detect mime type
    async fn download_og_image(&self, url: &str) -> Result<Option<ResolvedGif>> {
        let resp = match self.client.get(url).send().await {
            Ok(r) => r,
            Err(_) => return Ok(None),
        };

        let mime_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("image/gif")
            .to_string();

        let filename = Url::parse(url)
            .ok()
            .and_then(|u| {
                u.path_segments()
                    .map(|mut seg| {
                        seg.next_back()
                            .filter(|s| !s.is_empty())
                            .map(|s| s.to_string())
                    })
                    .flatten()
            })
            .unwrap_or_else(|| {
                let ext = if mime_type.contains("gif") {
                    "gif"
                } else if mime_type.contains("mp4") {
                    "mp4"
                } else if mime_type.contains("webm") {
                    "webm"
                } else {
                    "gif"
                };
                format!("embed.{}", ext)
            });

        Ok(Some(ResolvedGif {
            url: url.to_string(),
            mime_type,
            filename,
            provider: "fallback".to_string(),
        }))
    }
}
