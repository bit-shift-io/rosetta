Status: done

# 07-message-dispatcher

## What to build
Extract message dispatching from `BridgeCoordinator::route_message` into `MessageDispatcher`:
- Input: `ServiceMessage`, `Vec<(bridge_name, source_config, target_configs)>`, services map with `MessageSender` trait objects, `MessageStore`, `MessageFormatter` per service
- For each target: applies MediaHandler (from issue 02), applies AliasResolver (from issue 01), formats with target service's MessageFormatter, sends via MessageSender trait object
- Records mapping in MessageStore on success
- Returns results per target

## Files to create/modify
- src/bridge/dispatcher.rs (new - MessageDispatcher)
- src/bridge/mod.rs (re-export)
- src/bridge.rs (modify - use MessageDispatcher)

## Test approach
- Unit test with mock services: verify correct service called, message formatted, mapping saved
- Test media stripping per target config
- Test alias application per target config
- Test sender name suppression (display_names=false)

## Acceptance criteria
- [ ] MessageDispatcher.dispatch(msg, matches, services, store, formatters) sends to all targets
- [ ] Uses target service's MessageFormatter for text + caption
- [ ] Applies MediaHandler per target channel config
- [ ] Applies AliasResolver per source channel config
- [ ] Saves mapping on successful send
- [ ] Logs error on send failure but continues to other targets
- [ ] BridgeCoordinator::route_message delegates to dispatcher
- [ ] Integration tests pass (message bridging works end-to-end)

## Blocked by
01-bridge-matcher-alias-resolver, 02-deduplicator-media-handler, 03-message-formatter-trait, 04-service-traits-split