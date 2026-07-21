use anyhow::Result;
use async_trait::async_trait;

/// Result of resolving a GIF URL to direct media
#[derive(Debug, Clone)]
pub struct ResolvedGif {
    /// Direct URL to the media file (MP4, GIF, WebM)
    pub url: String,
    /// MIME type of the media (e.g., "video/mp4", "image/gif")
    pub mime_type: String,
    /// Suggested filename
    pub filename: String,
    /// Provider name that resolved this ("tenor", "giphy", "fallback")
    pub provider: String,
}

/// Trait for GIF URL resolution providers
#[async_trait]
pub trait GifProvider: Send + Sync {
    /// Provider name for logging/debugging
    fn name(&self) -> &str;

    /// Check if this provider handles the given domain
    fn supports_domain(&self, domain: &str) -> bool;

    /// Resolve a URL to direct media
    /// Returns None if the URL cannot be resolved (not an error)
    async fn resolve(&self, url: &str) -> Result<Option<ResolvedGif>>;

    /// Set the maximum upload size in bytes (from Matrix homeserver config)
    /// Providers can use this to select appropriate file sizes
    fn set_max_upload_size(&mut self, _max_bytes: u64) {
        // Default implementation does nothing
    }
}

/// Fallback HTML scraper (extracted from discord.rs)
pub mod fallback;
/// Giphy API provider
pub mod giphy;
/// Imgur API provider
pub mod imgur;
/// Klipy API provider
pub mod klipy;
/// Tenor API provider
pub mod tenor;

pub use fallback::FallbackScraper;
pub use giphy::GiphyProvider;
pub use imgur::ImgurProvider;
pub use klipy::KlipyProvider;
pub use tenor::TenorProvider;
