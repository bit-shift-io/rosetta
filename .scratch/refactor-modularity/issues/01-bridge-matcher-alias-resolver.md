Status: done

# 01-bridge-matcher-alias-resolver

## What to build
Extract bridge matching and alias resolution logic from `BridgeCoordinator.route_message` into two independent, testable modules:
- `BridgeMatcher`: Given a source (service, channel), find all `ChannelConfig` entries in the same bridge(s), excluding the source channel
- `AliasResolver`: Given a `ChannelConfig` and a sender_id, return the display name (applying aliases if configured)

After this slice, the matcher and resolver can be unit tested with various bridge/channel/alias configurations without any service connections.

## Files to create/modify
- src/bridge/matcher.rs (new)
- src/bridge/alias_resolver.rs (new)
- src/bridge/mod.rs (new - re-exports)
- src/bridge.rs (modify - use new types)

## Test approach
- Unit tests for `BridgeMatcher` with Config fixtures: single bridge, multiple bridges, no matching bridge, source channel not in bridge
- Unit tests for `AliasResolver`: alias present, alias absent, empty aliases map
- Property-based tests for matcher with generated bridge configs

## Acceptance criteria
- [ ] BridgeMatcher.find_targets(source_service, source_channel, &config) returns Vec<&ChannelConfig> for all target channels in matching bridges
- [ ] BridgeMatcher returns empty vec when no bridge matches
- [ ] AliasResolver.resolve(sender_id, &channel_config) returns alias when present, otherwise sender_id
- [ ] BridgeCoordinator.route_message uses both types (can verify by inspection)
- [ ] Unit tests pass with `cargo test bridge::matcher bridge::alias_resolver`

## Blocked by
None — can start immediately