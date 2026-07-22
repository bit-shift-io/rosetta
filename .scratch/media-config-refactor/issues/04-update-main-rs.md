Status: done

# 04-update-main-rs

## What to build

Update `src/main.rs` to:
1. Pass the new `config.media` to `GifResolver::new`
2. Remove references to old `config.gif_providers` and `config.media_whitelist`
3. Ensure `max_upload_size` from Matrix service is still propagated to `GifResolver`

## Files to create/modify

- src/main.rs

## Test approach

- Build and run with `config_example.yaml` - verify GIF resolver initializes correctly
- Verify Matrix service still sets max upload size after connection

## Acceptance criteria

- [ ] `main.rs` compiles with updated config and resolver
- [ ] `GifResolver::new` called with `config.media`
- [ ] Matrix service `max_upload_size()` still propagated to resolver
- [ ] Application starts successfully with example config

## Blocked by

03-update-gif-resolver