Status: done

# 09-reaction-handler

## What to build
Extract reaction handling from `BridgeCoordinator::handle_reaction` into `ReactionHandler`:
- Input: `ServiceReaction`, `HashMap<String, Arc<Mutex<Box<dyn Service>>>>`, `Config`, `MessageStore`
- Checks bridge_own_messages per source channel config
- Looks up all destination mappings via MessageStore
- Downcasts each target service to ReactionSender, calls react_to_message
- Returns results per target

## Files to create/modify
- src/bridge/reaction_handler.rs (new - ReactionHandler)
- src/bridge/mod.rs (re-export)
- src/bridge.rs (modify - use ReactionHandler)

## Test approach
- Unit test with mock ReactionSender services
- Test bridge_own_messages filtering
- Test mapping lookup
- Test error handling when service doesn't implement ReactionSender

## Acceptance criteria
- [ ] ReactionHandler.handle(reaction, services, config, store) processes reactions
- [ ] Respects bridge_own_messages per source channel
- [ ] Finds all destination mappings via MessageStore
- [ ] Downcasts to ReactionSender (skips services without impl)
- [ ] Calls react_to_message on each target
- [ ] BridgeCoordinator::handle_reaction delegates to ReactionHandler
- [ ] Reaction integration tests pass (Matrix↔Discord reaction sync)

## Blocked by
04-service-traits-split (ReactionSender trait), 01-bridge-matcher-alias-resolver (config lookup)