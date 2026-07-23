Status: done

# 08-edit-handler

## What to build
Extract edit handling from `BridgeCoordinator::handle_edit` into `EditHandler`:
- Input: `ServiceUpdate`, `HashMap<String, Arc<Mutex<Box<dyn Service>>>>`, `Config`, `MessageStore`
- Checks bridge_own_messages per source channel config
- Looks up all destination mappings via MessageStore
- Downcasts each target service to MessageEditor, calls edit_message
- Returns results per target

## Files to create/modify
- src/bridge/edit_handler.rs (new - EditHandler)
- src/bridge/mod.rs (re-export)
- src/bridge.rs (modify - use EditHandler)

## Test approach
- Unit test with mock MessageEditor services
- Test bridge_own_messages filtering
- Test mapping lookup with multiple destinations
- Test error handling when service doesn't implement MessageEditor

## Acceptance criteria
- [ ] EditHandler.handle(update, services, config, store) processes edits
- [ ] Respects bridge_own_messages per source channel
- [ ] Finds all destination mappings via MessageStore
- [ ] Downcasts to MessageEditor (skips services without impl)
- [ ] Calls edit_message on each target
- [ ] BridgeCoordinator::handle_edit delegates to EditHandler
- [ ] Edit integration tests pass (Matrix↔Discord edit sync)

## Blocked by
04-service-traits-split (MessageEditor trait), 01-bridge-matcher-alias-resolver (config lookup)