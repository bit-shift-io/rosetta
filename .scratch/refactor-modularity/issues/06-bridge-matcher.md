Status: pending

# 06-bridge-matcher

## What to build
Extract bridge/channel matching logic from `BridgeCoordinator::route_message` into `BridgeMatcher`:
- Input: source_service, source_channel, Config
- Output: Vec<(bridge_name, source_channel_config, target_channel_configs)>
- Handles: finding matching bridge, finding source channel config, filtering target channels (skip source)

Also extracts alias resolution: `AliasResolver` applies source config aliases to sender_id → display_name

## Files to create/modify
- src/bridge/matcher.rs (new - BridgeMatcher)
- src/bridge/alias_resolver.rs (new - AliasResolver)
- src/bridge/mod.rs (modify - re-export)
- src/bridge.rs (modify - use BridgeMatcher + AliasResolver)

## Test approach
- Property-based tests: random bridge configs, verify matcher finds correct channels
- Unit tests: alias resolution with/without aliases, empty sender_id handling
- Edge cases: multiple bridges with same service, no matching bridge, self-channel skip

## Acceptance criteria
- [ ] BridgeMatcher::match_channels(source_service, source_channel, config) returns matching targets
- [ ] Returns empty vec when no bridge matches (message dropped with warning)
- [ ] Correctly skips target channel when same service+channel as source
- [ ] AliasResolver::resolve(sender_id, aliases) returns alias or sender_id
- [ ] AliasResolver handles missing alias map (empty) gracefully
- [ ] BridgeCoordinator::route_message delegates to matcher + resolver
- [ ] All routing integration tests pass
- [ ] No behavior change in message delivery

## Blocked by
03-message-formatter-trait