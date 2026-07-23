# Refactor Modularity - Handoff Document

## Summary
This refactor decomposes Rosetta's large modules into focused, single-responsibility types to improve maintainability, testability, and extensibility. The 426-line `BridgeCoordinator` becomes a thin facade over 8 specialized handlers. The monolithic `Service` trait splits into 6 fine-grained traits (no facade). Each service implementation (Matrix, Discord, WhatsApp) gets its own subdirectory with capability-focused files. `main.rs` delegates to `ServiceBuilder` and `ServiceRegistry`.

## Implementation Order (Vertical Slices)

### Phase 1: Bridge Core (Independent, Foundation)
1. **01-bridge-matcher-alias-resolver** — Extract `BridgeMatcher` and `AliasResolver` from `route_message`. No dependencies on other new modules. **Start here.**
2. **02-deduplicator-media-handler** — Extract `Deduplicator` and `MediaHandler`. Pure logic, easily testable.

### Phase 2: Formatters & Handlers (Depend on Phase 1)
3. **03-message-formatter-trait** — Define `MessageFormatter` trait + implementations. Services updated to use it.
4. **04-service-traits-split** — Split `Service` trait into 6 traits (no facade). All services updated. **Blocks 05-07.**
5. **05-message-dispatcher** — Build `MessageDispatcher` using `BridgeMatcher`, `AliasResolver`, `MediaHandler`, formatters.
6. **06-edit-reaction-status-handlers** — Build `EditHandler`, `ReactionHandler`, `StatusHandler` using `BridgeMatcher`, `MessageStore`.
7. **07-event-router** — Build `EventRouter` delegating to dispatcher/edit/reaction/status handlers. `BridgeCoordinator` becomes facade.

### Phase 3: Service Lifecycle (Independent of Phase 2)
8. **08-service-builder** — Extract `ServiceBuilder` from `main.rs`. Constructs services from config.
9. **09-service-registry** — Extract `ServiceRegistry` managing service map lifecycle.
10. **10-main-rs-simplify** — Slim `main.rs` to: config → builder → registry → coordinator → signals.

### Phase 4: Service Internals (Can parallelize across services)
11. **11-matrix-service-split** — Split `matrix.rs` into subdirectory with capability files.
12. **12-discord-service-split** — Split `discord.rs` similarly.
13. **13-whatsapp-service-split** — Split `whatsapp.rs` similarly.

### Phase 5: Verification
14. **14-unit-tests-new-modules** — Unit tests for all new modules (blocked by 1-13).

## Key Files to Modify
| File | Change |
|------|--------|
| `src/bridge.rs` | Reduce to ~80-line facade delegating to new modules |
| `src/services/mod.rs` | Replace `Service` trait with 6 traits (no facade); re-export |
| `src/main.rs` | Delegate to `ServiceBuilder`/`ServiceRegistry` |
| `src/services/matrix.rs` | → `src/services/matrix/` directory |
| `src/services/discord.rs` | → `src/services/discord/` directory |
| `src/services/whatsapp.rs` | → `src/services/whatsapp/` directory |

## New Files to Create (Estimated 35+)
```
src/bridge/
├── mod.rs, matcher.rs, alias_resolver.rs, dispatcher.rs
├── edit_handler.rs, reaction_handler.rs, status_handler.rs
├── router.rs, formatter.rs, deduplicator.rs, media.rs
src/services/
├── builder.rs, registry.rs (new)
├── matrix/mod.rs, client.rs, events.rs, sender.rs, editor.rs, reactor.rs, members.rs, formatter.rs
├── discord/mod.rs, client.rs, events.rs, sender.rs, editor.rs, reactor.rs, members.rs, formatter.rs
└── whatsapp/mod.rs, client.rs, events.rs, sender.rs, editor.rs, reactor.rs, members.rs, formatter.rs
src/bridge/__tests__/*.unit.test.rs (10 files)
src/services/__tests__/*.unit.test.rs (2 files)
src/services/matrix/__tests__/*.unit.test.rs
src/services/discord/__tests__/*.unit.test.rs
src/services/whatsapp/__tests__/*.unit.test.rs
```

## Gotchas & Notes
- **Downcasting**: Bridge code uses `as_any().downcast_ref::<&dyn MessageEditor>()` for optional capabilities (`MessageEditor`, `MemberLister`). Mandatory traits (`MessageSender`, `Connectable`, `ReactionSender`, `ServiceInfo`) are always implemented — no downcast needed. Handle `None` gracefully for optional traits.
- **WhatsApp stubs**: `editor.rs` and `members.rs` for WhatsApp return `Err(anyhow!("not supported"))` or `Ok(vec![])` — don't panic.
- **MessageStore**: Pass `Arc<MessageStore>` to handlers; don't clone the store.
- **GIF Resolver**: Unchanged. Pass `Arc<GifResolver>` to Discord service via builder.
- **Config**: No changes. New types consume existing `Config` and `ChannelConfig`.
- **Integration tests**: Must pass without modification. Run `cargo test --test integration_tests -- --test-threads=1` after each phase.
- **Clippy/Fmt**: Run `cargo fmt && cargo clippy --all-targets --all-features` before committing each slice.

## Dependencies
- `mockall` (dev-dependency) recommended for service trait mocks in unit tests
- `proptest` (dev-dependency) for `BridgeMatcher` property tests
- No new runtime dependencies

## References
- PRD: `.scratch/refactor-modularity/PRD.md`
- Decisions: `.scratch/refactor-modularity/DECISIONS.md`
- Issues: `.scratch/refactor-modularity/issues/01-*.md` through `14-*.md`

## Start Command
In a new chat, run:
```
/implement-feature .scratch/refactor-modularity/
```