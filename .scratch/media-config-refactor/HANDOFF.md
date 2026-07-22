# Media Configuration Refactor - Handoff

## Summary

Refactor the media/GIF provider configuration from a flat structure to a nested `media` section with:
- `max_size_mb`: Global max media size (MB)
- `gif_providers`: Map of provider name → `{enabled, api_key}`

This enables explicit provider toggles, automatic fallback whitelist derivation, and global size limiting across all providers.

## Implementation Order

1. **01-update-config-structs** — Update `src/config.rs` with new `MediaConfig`/`GifProviderEntry` structs, remove old fields
2. **02-update-gif-provider-trait** — Add `set_max_upload_size` default impl to `GifProvider` trait, implement no-op on Tenor/Giphy/Imgur
3. **03-update-gif-resolver** — Rewrite `GifResolver::new` to accept `MediaConfig`, iterate provider map, derive fallback domains, apply size limit
4. **04-update-main-rs** — Pass `config.media` to resolver, remove old config references
5. **05-verify-config-example-and-docs** — Verify example config works, clean up doc comments

## Key Files

| File | Changes |
|------|---------|
| `src/config.rs` | New structs, remove `GifProviderConfig`, `media_whitelist` |
| `src/gif/providers/mod.rs` | Trait default method |
| `src/gif/providers/tenor.rs` | No-op `set_max_upload_size` |
| `src/gif/providers/giphy.rs` | No-op `set_max_upload_size` |
| `src/gif/providers/imgur.rs` | No-op `set_max_upload_size` |
| `src/gif/resolver.rs` | New constructor logic |
| `src/main.rs` | Updated resolver init |

## Gotchas

- **Provider name matching**: `GifResolver::new` matches string keys ("klipy", "giphy", "tenor", "imgur") to concrete types — must match exactly
- **Domain derivation**: Fallback whitelist built from enabled providers' `supports_domain` logic — ensure provider domain strings match
- **Size conversion**: `max_size_mb` × 1,048,576 = bytes for `set_max_upload_size`
- **Breaking change**: Old config format will fail — no backward compat

## Test Commands

```bash
# Format and lint
cargo fmt && cargo clippy --all-targets --all-features

# Quick start test
cp config_example.yaml data/config.yaml
cargo run
```

## References

- PRD: `.scratch/media-config-refactor/PRD.md`
- Decisions: `.scratch/media-config-refactor/DECISIONS.md`
- Issues: `.scratch/media-config-refactor/issues/01-*.md` through `05-*.md`