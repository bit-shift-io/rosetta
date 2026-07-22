Status: done

# 05-verify-config-example-and-docs

## What to build

Verify and update documentation:
1. Confirm `config_example.yaml` has the new media format (already done)
2. Update any inline docs/comments in config.rs referencing old format
3. Check for any other config examples or docs referencing old format

## Files to create/modify

- config_example.yaml (verify only)
- src/config.rs (update doc comments)
- Any .md files in docs/ or root referencing old config

## Test approach

- Verify `cargo run` works with `cp config_example.yaml data/config.yaml && cargo run` (quick start test)
- Check no references to `gif_providers:` (old flat format) or `media_whitelist:` remain in code/docs

## Acceptance criteria

- [ ] `config_example.yaml` uses new `media:` format correctly
- [ ] No references to old `GifProviderConfig` struct in doc comments
- [ ] No references to old `media_whitelist` in doc comments
- [ ] Application starts with example config

## Blocked by

04-update-main-rs