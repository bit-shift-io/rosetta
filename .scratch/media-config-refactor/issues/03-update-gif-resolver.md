Status: done

# 03-update-gif-resolver

## What to build

Update `src/gif/resolver.rs` to accept the new `MediaConfig` instead of separate `GifProviderConfig` and `media_whitelist`. The resolver should:
1. Iterate over enabled providers in the `gif_providers` map
2. Create provider instances based on provider name (tenor, giphy, klipy, imgur)
3. Derive fallback scraper whitelist from enabled providers' domains
4. Apply `max_size_mb` (converted to bytes) to all providers via `set_max_upload_size`

## Files to create/modify

- src/gif/resolver.rs

## Test approach

- Unit test `GifResolver::new` with various provider configurations
- Test that disabled providers are not instantiated
- Test that fallback whitelist contains correct domains for enabled providers
- Test that max_upload_size is propagated to all providers

## Acceptance criteria

- [ ] `GifResolver::new` signature accepts `MediaConfig` (or `Option<MediaConfig>`)
- [ ] Provider instantiation iterates over `gif_providers` map entries
- [ ] Only providers with `enabled: true` are added
- [ ] Fallback scraper whitelist built from enabled providers' domains
- [ ] `max_size_mb` converted to bytes and passed to all providers
- [ ] `set_max_upload_size` called on all providers during construction and via async method

## Blocked by

01-update-config-structs, 02-update-gif-provider-trait