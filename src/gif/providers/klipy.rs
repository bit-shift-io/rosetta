use anyhow::{Context, Result};
use async_trait::async_trait;
use log::info;
use regex::Regex;
use reqwest::Client;
use serde::Deserialize;
use std::sync::Arc;
use url::Url;
use urlencoding;

use super::GifProvider;
use crate::gif::ResolvedGif;

/// Klipy API provider
/// API docs: https://docs.klipy.com/
/// Uses endpoint: GET https://api.klipy.com/api/v1/{API_KEY}/{media_type}/{SLUG}
pub struct KlipyProvider {
    client: Arc<Client>,
    api_key: String,
    slug_regex: Regex,
    max_upload_size: Option<u64>,
}

impl KlipyProvider {
    pub fn new(api_key: String) -> Result<Self> {
        let client = Arc::new(Client::new());
        // Matches klipy.com/gifs/slug-name for extracting full slug
        let slug_regex = Regex::new(r"klipy\.com/(?:gifs?|clips?|stickers?|memes?)/([^/?#]+)")?;

        Ok(Self {
            client,
            api_key,
            slug_regex,
            max_upload_size: None,
        })
    }

    pub fn set_max_upload_size(&mut self, max_size: u64) {
        self.max_upload_size = Some(max_size);
    }

    fn extract_slug(&self, url: &str) -> Option<String> {
        self.slug_regex
            .captures(url)
            .and_then(|caps| caps.get(1))
            .map(|m| m.as_str().to_string())
    }

    fn extract_media_type(&self, url: &str) -> Option<String> {
        // Extract media type from URL path: /gif/, /gifs/, /clip/, /clips/, /sticker/, /stickers/, /meme/, /memes/
        if let Ok(parsed) = Url::parse(url) {
            let path = parsed.path();
            for segment in path.split('/') {
                let lower = segment.to_lowercase();
                if matches!(
                    lower.as_str(),
                    "gif" | "gifs" | "clip" | "clips" | "sticker" | "stickers" | "meme" | "memes"
                ) {
                    // Return plural form for API endpoint
                    return Some(lower.trim_end_matches('s').to_string() + "s");
                }
            }
        }
        None
    }

    fn is_klipy_url(&self, url: &str) -> bool {
        url.contains("klipy.com")
    }

    async fn try_direct_lookup(&self, media_type: &str, slug: &str) -> Result<Option<ResolvedGif>> {
        let api_url = format!(
            "https://api.klipy.com/api/v1/{}/{}/{}",
            self.api_key, media_type, slug
        );

        let resp = self
            .client
            .get(&api_url)
            .send()
            .await
            .context("Failed to call Klipy direct lookup API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            log::warn!("[Klipy] Direct lookup error {}: {}", status, text);
            return Ok(None);
        }

        let response_text = resp.text().await?;

        let klipy_resp: KlipyDirectResponse = serde_json::from_str(&response_text)
            .context("Failed to parse Klipy direct lookup response")?;

        self.extract_media_from_klipy_item(klipy_resp.data)
    }

    async fn try_search_endpoint(&self, query: &str) -> Result<Option<ResolvedGif>> {
        let api_url = format!(
            "https://api.klipy.com/api/v1/{}/search?q={}&limit=1",
            self.api_key,
            urlencoding::encode(query)
        );

        info!("[Klipy] Search: {}", api_url);

        let resp = self
            .client
            .get(&api_url)
            .send()
            .await
            .context("Failed to call Klipy search API")?;

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            log::warn!("[Klipy] Search API error {}: {}", status, text);
            return Ok(None);
        }

        let response_text = resp.text().await?;
        info!("[Klipy] Search response: {}", response_text);

        // Try parsing as search response format (has results array with media_formats)
        if let Ok(search_resp) = serde_json::from_str::<KlipySearchResponse>(&response_text) {
            let item = match search_resp.results.into_iter().next() {
                Some(item) => item,
                None => {
                    info!("[Klipy] No search results for query: {}", query);
                    return Ok(None);
                }
            };
            return self.extract_media_from_search_item(item);
        }

        // Try parsing as direct lookup format (has data with file object)
        if let Ok(direct_resp) = serde_json::from_str::<KlipyDirectResponse>(&response_text) {
            return self.extract_media_from_klipy_item(direct_resp.data);
        }

        log::warn!("[Klipy] Unknown search response format");
        Ok(None)
    }

    fn extract_media_from_search_item(&self, item: KlipySearchItem) -> Result<Option<ResolvedGif>> {
        let mut formats = std::collections::HashMap::new();

        for (format_key, media) in item.media_formats {
            formats.insert(format_key, media.url);
        }

        // preferred order. dont change this order!
        let preferred_formats = [
            ("gif", "image/gif"),
            ("webp", "image/webp"),
            ("mp4", "video/mp4"),
            ("webm", "video/webm"),
        ];

        let mut best_format: Option<(&str, &str, &String)> = None;

        for (format_key, mime_type) in &preferred_formats {
            if let Some(url) = formats.get(*format_key) {
                best_format = Some((format_key, mime_type, url));
                break;
            }
        }

        if best_format.is_none() {
            for (format_key, url) in &formats {
                if let Some(mime_type) = guess_mime_from_format(format_key) {
                    best_format = Some((format_key, mime_type, url));
                    break;
                }
            }
        }

        let (_format_name, mime_type, file_url) = match best_format {
            Some(f) => f,
            None => {
                log::debug!("[Klipy] No media formats found for item: {}", item.id);
                return Ok(None);
            }
        };

        let filename = Url::parse(&file_url)
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
                    "webp"
                };
                format!("klipy_{}.{}", item.id, ext)
            });

        Ok(Some(ResolvedGif {
            url: file_url.clone(),
            mime_type: mime_type.to_string(),
            filename,
            provider: "klipy".to_string(),
        }))
    }

    fn extract_media_from_klipy_item(&self, item: KlipyItem) -> Result<Option<ResolvedGif>> {
        // preferred order. dont change this order!
        // remove hd for now "hd"
        let size_tiers = ["md", "sm", "xs"];

        let preferred_formats = [
            ("gif", "image/gif"),
            ("webp", "image/webp"),
            ("mp4", "video/mp4"),
            ("webm", "video/webm"),
        ];

        log::info!(
            "[Klipy] extract_media_from_klipy_item called for item {}, max_upload_size={:?}",
            item.id,
            self.max_upload_size
        );

        // Iterate through size tiers
        for size_name in &size_tiers {
            if let Some(file_size) = item.file_sizes.get(*size_name) {
                // Check each preferred format in order
                for (format_key, mime_type) in &preferred_formats {
                    let format_opt = match *format_key {
                        "gif" => file_size.gif.as_ref(),
                        "webp" => file_size.webp.as_ref(),
                        "mp4" => file_size.mp4.as_ref(),
                        "webm" => file_size.webm.as_ref(),
                        _ => None,
                    };

                    if let Some(format_info) = format_opt {
                        // Check if file size is within the max upload limit
                        if let Some(max_size) = self.max_upload_size {
                            if let Some(file_size_bytes) = format_info.size {
                                log::info!(
                                    "[Klipy] Checking {} bytes against max {} bytes for {} {}",
                                    file_size_bytes,
                                    max_size,
                                    size_name,
                                    format_key
                                );
                                if file_size_bytes > max_size {
                                    log::info!(
                                        "[Klipy] Skipping {} bytes - exceeds max upload size {} bytes",
                                        file_size_bytes,
                                        max_size
                                    );
                                    continue;
                                }
                            }
                        }

                        let file_url = &format_info.url;
                        let filename = Url::parse(file_url)
                            .ok()
                            .and_then(|u| {
                                u.path_segments().and_then(|mut seg| {
                                    seg.next_back()
                                        .filter(|s| !s.is_empty())
                                        .map(|s| s.to_string())
                                })
                            })
                            .unwrap_or_else(|| {
                                let ext = if mime_type.contains("mp4") || mime_type.contains("webm")
                                {
                                    "mp4"
                                } else if mime_type.contains("gif") {
                                    "gif"
                                } else {
                                    "webp"
                                };
                                format!("klipy_{}.{}", item.id, ext)
                            });

                        info!(
                            "[Klipy] Selected {} {} ({} bytes) for item {}",
                            size_name,
                            format_key,
                            format_info.size.unwrap_or(0),
                            item.id
                        );

                        return Ok(Some(ResolvedGif {
                            url: file_url.clone(),
                            mime_type: mime_type.to_string(),
                            filename,
                            provider: "klipy".to_string(),
                        }));
                    }
                }
            }
        }

        log::debug!("[Klipy] No media formats found for item: {}", item.id);
        Ok(None)
    }
}

fn guess_mime_from_format(format: &str) -> Option<&'static str> {
    if format.contains("mp4") {
        Some("video/mp4")
    } else if format.contains("webm") {
        Some("video/webm")
    } else if format.contains("gif") {
        Some("image/gif")
    } else if format.contains("webp") {
        Some("image/webp")
    } else {
        None
    }
}

#[derive(Debug, Deserialize)]
struct KlipyDirectResponse {
    data: KlipyItem,
    #[allow(dead_code)]
    #[serde(rename = "result")]
    success: bool,
}

#[derive(Debug, Deserialize)]
struct KlipySearchResponse {
    results: Vec<KlipySearchItem>,
    #[allow(dead_code)]
    #[serde(rename = "result")]
    success: bool,
}

#[derive(Debug, Deserialize)]
struct KlipySearchItem {
    id: serde_json::Value,
    #[allow(dead_code)]
    title: Option<String>,
    media_formats: std::collections::HashMap<String, KlipyMediaFormat>,
    #[allow(dead_code)]
    tags: Option<Vec<String>>,
    #[allow(dead_code)]
    dims: Option<Vec<u32>>,
}

#[derive(Debug, Deserialize)]
struct KlipyMediaFormat {
    url: String,
    #[allow(dead_code)]
    duration: Option<f64>,
    #[allow(dead_code)]
    preview: Option<String>,
    #[allow(dead_code)]
    dims: Option<Vec<u32>>,
    #[allow(dead_code)]
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct KlipyItem {
    id: serde_json::Value,
    #[allow(dead_code)]
    title: Option<String>,
    #[serde(rename = "file")]
    file_sizes: std::collections::HashMap<String, KlipyFileSize>,
    #[allow(dead_code)]
    tags: Option<Vec<String>>,
    #[allow(dead_code)]
    dims: Option<Vec<u32>>,
}

#[derive(Debug, Deserialize)]
struct KlipyFileFormat {
    url: String,
    #[allow(dead_code)]
    width: Option<u32>,
    #[allow(dead_code)]
    height: Option<u32>,
    #[allow(dead_code)]
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct KlipyFileSize {
    gif: Option<KlipyFileFormat>,
    webp: Option<KlipyFileFormat>,
    #[allow(dead_code)]
    jpg: Option<KlipyFileFormat>,
    mp4: Option<KlipyFileFormat>,
    webm: Option<KlipyFileFormat>,
}

#[async_trait]
impl GifProvider for KlipyProvider {
    fn name(&self) -> &str {
        "klipy"
    }

    fn supports_domain(&self, domain: &str) -> bool {
        domain.contains("klipy.com")
    }

    fn set_max_upload_size(&mut self, max_bytes: u64) {
        self.max_upload_size = Some(max_bytes);
    }

    async fn resolve(&self, url: &str) -> Result<Option<ResolvedGif>> {
        if !self.is_klipy_url(url) {
            return Ok(None);
        }

        // Try direct lookup with slug and media type
        if let (Some(slug), Some(media_type)) =
            (self.extract_slug(url), self.extract_media_type(url))
        {
            if let Some(result) = self.try_direct_lookup(&media_type, &slug).await? {
                return Ok(Some(result));
            }
        }

        // If direct lookup fails, try search with slug as query
        if let Some(slug) = self.extract_slug(url) {
            let query = slug.replace('-', " ");
            info!("[Klipy] Trying search with query: '{}'", query);
            if let Some(result) = self.try_search_endpoint(&query).await? {
                return Ok(Some(result));
            }
        }

        info!("[Klipy] Could not resolve via API, will try fallback scraper");
        Ok(None)
    }
}
