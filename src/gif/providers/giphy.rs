use anyhow::{Context, Result};
use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use url::Url;

use super::GifProvider;
use crate::gif::ResolvedGif;

/// Giphy API provider
/// API docs: https://developers.giphy.com/docs/api/endpoint/gifs
pub struct GiphyProvider {
    client: Arc<Client>,
    api_key: String,
    gif_id_regex: Regex,
}

impl GiphyProvider {
    pub fn new(api_key: String) -> Result<Self> {
        let client = Arc::new(Client::new());
        // Matches giphy.com/gifs/ID, giphy.com/ID, gph.is/ID, media.giphy.com/media/ID
        let gif_id_regex = Regex::new(
            r"(?:giphy\.com/(?:gifs/)?|gph\.is/|media\.giphy\.com/media/)([a-zA-Z0-9]+)",
        )?;

        Ok(Self {
            client,
            api_key,
            gif_id_regex,
        })
    }

    /// Extract GIF ID from Giphy URL
    fn extract_gif_id(&self, url: &str) -> Option<String> {
        self.gif_id_regex
            .captures(url)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Check if URL is a Giphy URL
    fn is_giphy_url(&self, url: &str) -> bool {
        url.contains("giphy.com") || url.contains("gph.is") || url.contains("media.giphy.com")
    }
}

#[derive(Debug, Deserialize)]
struct GiphyResponse {
    data: GiphyGif,
}

#[derive(Debug, Deserialize)]
struct GiphyGif {
    id: String,
    images: GiphyImages,
}

#[derive(Debug, Deserialize)]
struct GiphyImages {
    original: GiphyImage,
    #[serde(rename = "original_mp4")]
    original_mp4: Option<GiphyImage>,
    #[serde(rename = "preview_mp4")]
    preview_mp4: Option<GiphyImage>,
    #[serde(rename = "preview_gif")]
    preview_gif: Option<GiphyImage>,
    #[allow(dead_code)]
    downsized: Option<GiphyImage>,
}

#[derive(Debug, Deserialize)]
struct GiphyImage {
    url: String,
    #[allow(dead_code)]
    width: Option<String>,
    #[allow(dead_code)]
    height: Option<String>,
    #[allow(dead_code)]
    size: Option<String>,
    #[allow(dead_code)]
    frames: Option<String>,
    #[allow(dead_code)]
    mp4: Option<String>,
    #[allow(dead_code)]
    mp4_size: Option<String>,
    #[allow(dead_code)]
    webp: Option<String>,
    #[allow(dead_code)]
    webp_size: Option<String>,
}

#[async_trait]
impl GifProvider for GiphyProvider {
    fn name(&self) -> &str {
        "giphy"
    }

    fn supports_domain(&self, domain: &str) -> bool {
        domain.contains("giphy.com")
            || domain.contains("gph.is")
            || domain.contains("media.giphy.com")
    }

    async fn resolve(&self, url: &str) -> Result<Option<ResolvedGif>> {
        if !self.is_giphy_url(url) {
            return Ok(None);
        }

        let gif_id = match self.extract_gif_id(url) {
            Some(id) => id,
            None => {
                log::debug!("[Giphy] Could not extract GIF ID from: {}", url);
                return Ok(None);
            }
        };

        let api_url = format!(
            "https://api.giphy.com/v1/gifs/{}?api_key={}",
            gif_id, self.api_key
        );

        let resp = self
            .client
            .get(&api_url)
            .send()
            .await
            .context("Failed to call Giphy API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            log::warn!("[Giphy] API error {}: {}", status, text);
            return Ok(None);
        }

        let giphy_resp: GiphyResponse = resp
            .json()
            .await
            .context("Failed to parse Giphy response")?;

        let gif = giphy_resp.data;

        // Prefer MP4 (original_mp4 or preview_mp4), then original GIF, then preview GIF
        let (media_url, mime_type, filename) = if let Some(mp4) = gif.images.original_mp4 {
            (mp4.url, "video/mp4", "giphy.mp4")
        } else if let Some(mp4) = gif.images.preview_mp4 {
            (mp4.url, "video/mp4", "giphy.mp4")
        } else if let Some(webp) = gif.images.original.webp {
            (webp, "image/webp", "giphy.webp")
        } else {
            (gif.images.original.url, "image/gif", "giphy.gif")
        };

        // Extract filename from URL
        let filename = Url::parse(&media_url)
            .ok()
            .and_then(|u| {
                u.path_segments().and_then(|mut seg| {
                    seg.next_back()
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                })
            })
            .unwrap_or_else(|| filename.to_string());

        Ok(Some(ResolvedGif {
            url: media_url.to_string(),
            mime_type: mime_type.to_string(),
            filename,
            provider: "giphy".to_string(),
        }))
    }
}
