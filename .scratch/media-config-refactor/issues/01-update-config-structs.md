Status: done

# 01-update-config-structs

## What to build

Update `src/config.rs` to parse the new media configuration format. Replace the flat `gif_providers` and `media_whitelist` fields with a nested `media` section containing `max_size_mb` and a `gif_providers` map.

## Files to create/modify

- src/config.rs

## Test approach

- Add unit tests for `Config::load` with the new YAML structure
- Test that all four providers (tenor, giphy, klipy, imgur) parse correctly with `enabled` and `api_key`
- Test that `max_size_mb` parses as u64
- Test that disabled providers (`enabled: false`) still parse but are marked disabled
- Verify old config format fails to deserialize (expected breaking change)

## Acceptance criteria

- [ ] New `MediaConfig`, `GifProviderEntry` structs added
- [ ] `Config` struct has `media: Option<MediaConfig>` field
- [ ] Old fields `gif_providers: GifProviderConfig` and `media_whitelist: Vec<String>` removed
- [ ] `config_example.yaml` loads successfully with new format
- [ ] Unit tests pass for new config parsing

## Blocked by

None — can start immediately