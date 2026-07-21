//! GIF Resolver Module
//!
//! Provides a unified interface for resolving GIF website URLs (Tenor, Giphy, etc.)
//! to direct media files (MP4, GIF, WebM) using official APIs with fallback scraping.

pub mod providers;
pub mod resolver;

pub use providers::ResolvedGif;
pub use resolver::GifResolver;
