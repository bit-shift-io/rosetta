Status: pending

# 02-deduplicator-media-handler

## What to build
Extract deduplication and media policy logic from `BridgeCoordinator.route_message`:
- `Deduplicator`: Wraps `MessageStore.exists` with a clean API `check_and_mark(source_service, source_channel, source_id) -> Result<bool>` returning true if already processed
- `MediaHandler`: Given a mutable `ServiceMessage` and target `ChannelConfig`, strips attachments if `!enable_media`, returns processed message

Both are pure logic with no async dependencies, easily unit testable.

## Files to create/modify
- src/bridge/deduplicator.rs (new)
- src/bridge/media.rs (new)
- src/bridge/mod.rs (re-exports)
- src/bridge.rs (modify - use new types)

## Test approach
- Unit tests for `Deduplicator`: first call returns false (not duplicate), second returns true; different source_ids don't collide
- Unit tests for `MediaHandler`: enable_media=true keeps attachments, enable_media=false strips them; display_names=false clears sender
- Mock MessageStore for deduplicator tests

## Acceptance criteria
- [ ] Deduplicator.check_and_mark returns Ok(false) on first call, Ok(true) on subsequent calls for same triple
- [ ] Deduplicator returns Err on MessageStore error
- [ ] MediaHandler.process(msg, config) strips attachments when enable_media=false
- [ ] MediaHandler.process(msg, config) clears sender when display_names=false
- [ ] BridgeCoordinator uses both (verify by inspection)
- [ ] Unit tests pass

## Blocked by
None — can start immediately