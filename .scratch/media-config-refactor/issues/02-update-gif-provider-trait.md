Status: done

# 02-update-gif-provider-trait

## What to build

Update the `GifProvider` trait in `src/gif/providers/mod.rs` to include a default `set_max_upload_size` method. Update all four provider implementations (Tenor, Giphy, Klipy, Imgur) to implement this method (Klipy already has it, others need no-op impl).

## Files to create/modify

- src/gif/providers/mod.rs
- src/gif/providers/tenor.rs
- src/gif/providers/giphy.rs
- src/gif/providers/klipy.rs (verify existing impl)
- src/gif/providers/imgur.rs

## Test approach

- Unit test that each provider implements the trait method
- Verify Klipy's existing implementation still works with size filtering
- Verify no-op implementations compile

## Acceptance criteria

- [ ] `GifProvider` trait has `fn set_max_upload_size(&mut self, max_bytes: u64) { }` default impl
- [ ] All four providers compile with the trait
- [ ] Klipy's size filtering still works via `set_max_upload_size`
- [ ] No breaking changes to `resolve()` signatures

## Blocked by

01-update-config-structs (providers created in GifResolver.new which uses config)