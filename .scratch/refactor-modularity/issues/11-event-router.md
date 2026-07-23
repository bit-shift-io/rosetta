Status: done

# 11-event-router

## What to build
Create `EventRouter` as the central coordinator that replaces `BridgeCoordinator::start` event loop:
- Owns: `MessageDispatcher`, `EditHandler`, `ReactionHandler`, `StatusHandler`, `BridgeMatcher`, `Deduplicator`
- Event loop: recv `ServiceEvent` → match:
  - `NewMessage` → deduplicate → match → dispatch
  - `UpdateMessage` → handle edit
  - `NewReaction` → handle reaction
- Graceful shutdown

`BridgeCoordinator` becomes a thin facade holding config + services + store, delegating to EventRouter.

## Files to create/modify
- src/bridge/router.rs (new - EventRouter)
- src/bridge/mod.rs (re-export)
- src/bridge.rs (modify - thin facade, delegates to EventRouter)

## Test approach
- Unit test: mock handlers, verify correct handler called per event type
- Test deduplication short-circuits NewMessage
- Test shutdown signal stops loop
- Integration tests verify full routing

## Acceptance criteria
- [ ] EventRouter::new(config, services, store, handlers...) constructs router
- [ ] EventRouter::run() processes events until channel closed
- [ ] Routes NewMessage → deduplicate → match → dispatch
- [ ] Routes UpdateMessage → edit handler
- [ ] Routes NewReaction → reaction handler
- [ ] BridgeCoordinator::start() delegates to EventRouter::run()
- [ ] BridgeCoordinator::shutdown() signals router shutdown
- [ ] All integration tests pass (message, edit, reaction, status)

## Blocked by
01-bridge-matcher-alias-resolver, 02-deduplicator-media-handler, 07-message-dispatcher, 08-edit-handler, 09-reaction-handler, 10-status-handler