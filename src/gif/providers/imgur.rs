use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, info, warn};
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use url::Url;

use super::GifProvider;
use crate::gif::ResolvedGif;

/// Imgur API provider
/// API docs: https://apidocs.imgur.com/
pub struct ImgurProvider {
    client: Arc<Client>,
    client_id: String,
    post_id_regex: Regex,
}

impl ImgurProvider {
    pub fn new(client_id: String) -> Result<Self> {
        let client = Arc::new(Client::new());
        // Matches imgur.com/gallery/ID, imgur.com/ID, imgur.com/a/ID, i.imgur.com/ID
        let post_id_regex = Regex::new(
            r"(?:imgur\.com/(?:gallery/|a/)?|i\.imgur\.com/)([a-zA-Z0-9]+)(?:\.[a-zA-Z0-9]+)?",
        )?;

        Ok(Self {
            client,
            client_id,
            post_id_regex,
        })
    }

    fn extract_post_id(&self, url: &str) -> Option<String> {
        self.post_id_regex
            .captures(url)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    fn is_imgur_url(&self, url: &str) -> bool {
        url.contains("imgur.com") || url.contains("i.imgur.com")
    }
}

#[derive(Debug, Deserialize)]
struct ImgurResponse {
    data: ImgurData,
    success: bool,
    status: u32,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum ImgurData {
    Image(ImgurImage),
    Album(ImgurAlbum),
}

#[derive(Debug, Deserialize)]
struct ImgurImage {
    id: String,
    title: Option<String>,
    description: Option<String>,
    #[serde(rename = "type")]
    mime_type: String,
    animated: bool,
    link: String,
    #[serde(rename = "mp4")]
    mp4_url: Option<String>,
    #[serde(rename = "webm")]
    webm_url: Option<String>,
    #[serde(rename = "gifv")]
    gifv_url: Option<String>,
    width: u32,
    height: u32,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct ImgurAlbum {
    id: String,
    images: Vec<ImgurImage>,
}

#[async_trait]
impl GifProvider for ImgurProvider {
    fn name(&self) -> &str {
        "imgur"
    }

    fn supports_domain(&self, domain: &str) -> bool {
        domain.contains("imgur.com")
    }

    fn set_max_upload_size(&mut self, _max_bytes: u64) {
        // Imgur doesn't currently support size filtering
    }

    async fn resolve(&self, url: &str) -> Result<Option<ResolvedGif>> {
        if !self.is_imgur_url(url) {
            return Ok(None);
        }

        let post_id = match self.extract_post_id(url) {
            Some(id) => id,
            None => {
                log::debug!("[Imgur] Could not extract post ID from: {}", url);
                return Ok(None);
            }
        };

        info!("[Imgur] Resolving post ID: {} from URL: {}", post_id, url);

        let api_url = format!("https://api.imgur.com/3/image/{}", post_id);

        let resp = self
            .client
            .get(&api_url)
            .header("Authorization", format!("Client-ID {}", self.client_id))
            .send()
            .await
            .context("Failed to call Imgur API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            log::warn!("[Imgur] API error {}: {}", status, text);
            return Ok(None);
        }

        let imgur_resp: ImgurResponse = resp
            .json()
            .await
            .context("Failed to parse Imgur response")?;

        if !imgur_resp.success {
            log::warn!("[Imgur] API returned error: {}", imgur_resp.status);
            return Ok(None);
        }

        // Handle both single image and album
        let image = match imgur_resp.data {
            ImgurData::Image(img) => img,
            ImgurData::Album(album) => {
                // For albums, pick the first animated image or first image
                let mut images = album.images;
                // Try to find animated first
                if let Some(pos) = images.iter().position(|img| img.animated) {
                    images.swap_remove(pos)
                } else {
                    images
                        .into_iter()
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("No images in album"))?
                }
            }
        };

        // Prefer MP4, then WebM, then GIFV, then original link
        let (media_url, mime_type): (String, String) = if let Some(mp4) = image.mp4_url {
            (mp4, "video/mp4".to_string())
        } else if let Some(webm) = image.webm_url {
            (webm, "video/webm".to_string())
        } else if let Some(gifv) = image.gifv_url {
            (gifv, "video/mp4".to_string()) // gifv is essentially MP4
        } else {
            (image.link, image.mime_type)
        };

        let filename = Url::parse(&media_url)
            .ok()
            .and_then(|u| {
                u.path_segments().and_then(|mut seg| {
                    seg.next_back()
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                })
            })
            .unwrap_or_else(|| {
                let ext = if mime_type.contains("mp4") || mime_type.contains("webm") {
                    "mp4"
                } else if mime_type.contains("gif") {
                    "gif"
                } else {
                    "jpg"
                };
                format!("imgur_{}.{}", image.id, ext)
            });

        Ok(Some(ResolvedGif {
            url: media_url,
            mime_type: mime_type.to_string(),
            filename,
            provider: "imgur".to_string(),
        }))
    }
}
