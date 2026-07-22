# Media Configuration Refactor

## Problem Statement

The current media/GIF provider configuration in `config.yaml` uses a flat structure with separate top-level keys `gif_providers` (flat struct with hardcoded provider fields) and `media_whitelist` (array of domain strings). This design has several limitations:

1. **Inflexible provider configuration**: Adding a new provider requires code changes (new field in `GifProviderConfig` struct, new provider creation logic in `GifResolver`)
2. **No per-provider enable/disable**: Providers are enabled by presence of API key only; no explicit `enabled` toggle
3. **Separate whitelist maintenance**: The `media_whitelist` for fallback scraping must be manually kept in sync with configured providers
4. **No global media size limit**: The `max_size_mb` setting only exists in the new config example but isn't parsed or used

## Solution

Refactor the configuration to a nested `media` section with:
- `max_size_mb`: Global maximum media upload size in megabytes (applied to all providers)
- `gif_providers`: A map of provider name → provider config, where each entry has:
  - `enabled: bool` — explicit toggle
  - `api_key: String` — provider API key/credentials

This allows:
- Adding new providers purely via config (no code changes for basic providers)
- Explicit enable/disable without removing API keys
- Automatic fallback whitelist derivation from enabled providers
- Global size limit enforced across all providers

## User Stories

1. As a **bridge operator**, I want to configure a global maximum media size so that large files are rejected before downloading
2. As a **bridge operator**, I want to explicitly enable/disable GIF providers without deleting API keys
3. As a **bridge operator**, I want to add new GIF providers by just adding config entries (for providers that follow standard patterns)
4. As a **developer**, I want the fallback scraper whitelist to be automatically derived from enabled providers
5. As a **bridge operator**, I want to configure Klipy, Giphy, Tenor, and Imgur with a consistent config structure

## Implementation Decisions

### Modules Built or Modified
- `src/config.rs` — New `MediaConfig`, `GifProviderEntry` structs; remove old `GifProviderConfig` and `media_whitelist`
- `src/gif/providers/mod.rs` — Add default `set_max_upload_size` to `GifProvider` trait
- `src/gif/providers/tenor.rs`, `giphy.rs`, `imgur.rs` — Implement no-op `set_max_upload_size`
- `src/gif/providers/klipy.rs` — Verify existing `set_max_upload_size` works
- `src/gif/resolver.rs` — Accept `MediaConfig`, iterate provider map, derive fallback domains, apply size limit
- `src/main.rs` — Pass `config.media` to `GifResolver::new`

### Interfaces and Signatures
```rust
// config.rs
pub struct MediaConfig {
    pub max_size_mb: u64,
    pub gif_providers: HashMap<String, GifProviderEntry>,
}

pub struct GifProviderEntry {
    pub enabled: bool,
    pub api_key: String,
}

// gif/providers/mod.rs
#[async_trait]
pub trait GifProvider {
    fn name(&self) -> &str;
    fn supports_domain(&self, domain: &str) -> bool;
    async fn resolve(&self, url: &str) -> Result<Option<ResolvedGif>>;
    fn set_max_upload_size(&mut self, max_bytes: u64) { }  // NEW default impl
}

// gif/resolver.rs
impl GifResolver {
    pub fn new(
        media_config: &Option<MediaConfig>,
        debug: bool,
        max_upload_size: Option<u64>,
    ) -> Self
}
```

### Architectural Decisions
- Provider map uses string keys ("klipy", "giphy", "tenor", "imgur") matched in `GifResolver::new` to instantiate concrete types
- Fallback scraper whitelist derived from enabled providers' known domains (tenor.com, giphy.com, etc.)
- `max_size_mb` converted to bytes (×1,048,576) and passed to all providers
- `GifResolver::set_max_upload_size` still propagates to all providers at runtime (Matrix connection)

### Schema Changes
- Remove: `Config.gif_providers: GifProviderConfig`, `Config.media_whitelist: Vec<String>`
- Add: `Config.media: Option<MediaConfig>`

## Testing Decisions

- **Unit tests in `config.rs`**: Test `Config::load` with new YAML structure, verify all provider entries parse correctly
- **Unit tests in `gif/resolver.rs`**: Test `GifResolver::new` with various provider configs (enabled/disabled, missing keys)
- **Integration test**: Start app with `config_example.yaml`, verify GIF resolver initializes with 4 providers (klipy, giphy enabled; tenor, imgur disabled)

## Out of Scope

- Adding new GIF providers beyond the existing four (Tenor, Giphy, Klipy, Imgur)
- Per-provider size limits (global only)
- Per-bridge media settings (global only)
- Hot-reload of media config

## File Structure

```
src/
├── config.rs                    # Modified: new structs, removed old
├── gif/
│   ├── providers/
│   │   ├── mod.rs               # Modified: trait default method
│   │   ├── tenor.rs             # Modified: no-op impl
│   │   ├── giphy.rs             # Modified: no-op impl
│   │   ├── klipy.rs             # Verify: existing impl
│   │   └── imgur.rs             # Modified: no-op impl
│   └── resolver.rs              # Modified: new constructor, provider iteration
└── main.rs                      # Modified: pass config.media
```

## Acceptance Criteria

- [ ] `config_example.yaml` loads successfully with new `media:` format
- [ ] All four providers (klipy, giphy, tenor, imgur) instantiate when enabled
- [ ] Disabled providers (`enabled: false`) are not instantiated
- [ ] Fallback scraper whitelist contains domains of enabled providers only
- [ ] `max_size_mb: 10` sets 10MB limit on all providers (including Klipy size filtering)
- [ ] Matrix service `max_upload_size()` still propagates to resolver at runtime
- [ ] `cargo fmt && cargo clippy --all-targets --all-features` passes
- [ ] Application starts and runs with example config

## References

- Current config: `config_example.yaml` (new format), `src/config.rs:7-36`
- Current resolver: `src/gif/resolver.rs:22-113`
- Provider trait: `src/gif/providers/mod.rs`
- Klipy size filtering: `src/gif/providers/klipy.rs:38-40, 246-262`