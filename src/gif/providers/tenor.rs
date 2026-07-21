use anyhow::{Context, Result};
use async_trait::async_trait;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use url::Url;

use super::GifProvider;
use crate::gif::ResolvedGif;

/// Tenor API provider using Google's Tenor API
/// API docs: https://developers.google.com/tenor/guides/quickstart
pub struct TenorProvider {
    client: Arc<Client>,
    api_key: String,
    client_key: String,
    post_id_regex: Regex,
}

impl TenorProvider {
    pub fn new(api_key: String, client_key: Option<String>) -> Result<Self> {
        let client = Arc::new(Client::new());
        let post_id_regex =
            Regex::new(r"tenor\.com/(?:view|[^/]+/)(?:[^/]+-)?([a-zA-Z0-9]+)(?:/|\?|$)")?;

        Ok(Self {
            client,
            api_key,
            client_key: client_key.unwrap_or_else(|| "rosetta_bridge".to_string()),
            post_id_regex,
        })
    }

    /// Extract post ID from Tenor URL
    fn extract_post_id(&self, url: &str) -> Option<String> {
        self.post_id_regex
            .captures(url)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    /// Check if URL is a Tenor URL
    fn is_tenor_url(&self, url: &str) -> bool {
        url.contains("tenor.com") || url.contains("tenor.co")
    }
}

#[derive(Debug, Deserialize)]
struct TenorResponse {
    results: Vec<TenorPost>,
}

#[derive(Debug, Deserialize)]
struct TenorPost {
    id: String,
    media_formats: std::collections::HashMap<String, TenorMediaFormat>,
}

#[derive(Debug, Deserialize)]
struct TenorMediaFormat {
    url: String,
    #[serde(rename = "type")]
    media_type: String,
    #[allow(dead_code)]
    duration: Option<f64>,
    #[allow(dead_code)]
    preview: Option<String>,
    #[allow(dead_code)]
    dims: Option<Vec<u32>>,
    #[allow(dead_code)]
    size: Option<u64>,
}

#[async_trait]
impl GifProvider for TenorProvider {
    fn name(&self) -> &str {
        "tenor"
    }

    fn supports_domain(&self, domain: &str) -> bool {
        domain.contains("tenor.com") || domain.contains("tenor.co")
    }

    async fn resolve(&self, url: &str) -> Result<Option<ResolvedGif>> {
        if !self.is_tenor_url(url) {
            return Ok(None);
        }

        let post_id = match self.extract_post_id(url) {
            Some(id) => id,
            None => {
                log::debug!("[Tenor] Could not extract post ID from: {}", url);
                return Ok(None);
            }
        };

        let api_url = format!(
            "https://tenor.googleapis.com/v2/posts?ids={}&key={}&client_key={}",
            post_id, self.api_key, self.client_key
        );

        let resp = self
            .client
            .get(&api_url)
            .send()
            .await
            .context("Failed to call Tenor API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            log::warn!("[Tenor] API error {}: {}", status, text);
            return Ok(None);
        }

        let tenor_resp: TenorResponse = resp
            .json()
            .await
            .context("Failed to parse Tenor response")?;

        let post = tenor_resp.results.into_iter().next();
        let post = match post {
            Some(p) => p,
            None => {
                log::debug!("[Tenor] No results for post ID: {}", post_id);
                return Ok(None);
            }
        };

        // Prefer MP4 (video/mp4), then GIF, then WebM
        let preferred_formats = ["mp4", "gif", "webm", "tinygif", "nanogif"];
        let mut best_format: Option<(&str, &TenorMediaFormat)> = None;

        for format in &preferred_formats {
            if let Some(media) = post.media_formats.get(*format) {
                best_format = Some((*format, media));
                break;
            }
        }

        // Fallback to any format
        if best_format.is_none() {
            best_format = post
                .media_formats
                .iter()
                .next()
                .map(|(k, v)| (k.as_str(), v));
        }

        let (format_name, media) = match best_format {
            Some(f) => f,
            None => {
                log::debug!("[Tenor] No media formats found for post: {}", post_id);
                return Ok(None);
            }
        };

        // Determine mime type from format
        let mime_type = match format_name.as_ref() {
            "mp4" | "tinygif" | "nanogif" => "video/mp4",
            "gif" => "image/gif",
            "webm" => "video/webm",
            _ => "application/octet-stream",
        };

        // Extract filename from URL
        let filename = Url::parse(&media.url)
            .ok()
            .and_then(|u| {
                u.path_segments().and_then(|mut seg| {
                    seg.next_back()
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                })
            })
            .unwrap_or_else(|| {
                format!(
                    "tenor_{}.{}",
                    post_id,
                    if mime_type.contains("mp4") {
                        "mp4"
                    } else if mime_type.contains("gif") {
                        "gif"
                    } else {
                        "webm"
                    }
                )
            });

        Ok(Some(ResolvedGif {
            url: media.url.clone(),
            mime_type: mime_type.to_string(),
            filename,
            provider: "tenor".to_string(),
        }))
    }
}
