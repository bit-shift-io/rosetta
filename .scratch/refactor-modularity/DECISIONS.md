# Refactor Modularity - Decisions Log

### Q1: How to decompose BridgeCoordinator?
**Decision:** Split into 8 focused types: EventRouter, MessageDispatcher, EditHandler, ReactionHandler, StatusHandler, BridgeMatcher, AliasResolver, Deduplicator
- **Why:** Current 426-line file mixes routing, dispatching, editing, reactions, status, matching, aliasing, deduplication. Each has different reasons to change.
- **Implication:** BridgeCoordinator becomes a thin facade (~80 lines) delegating to these types. Easier to test each in isolation.
- **Alternatives considered:** 
  - Keep as-is with private methods — rejected: doesn't improve testability or navigation
  - Split into 2-3 larger modules — rejected: still mixes concerns (e.g., routing + dispatching)

### Q2: Should Service trait be split?
**Decision:** Yes, split into 6 fine-grained traits (no facade):
- `Connectable` (connect, disconnect, wait_until_ready, is_connected)
- `MessageSender` (send_message)
- `MessageEditor` (edit_message) — optional
- `ReactionSender` (react_to_message) — optional
- `MemberLister` (get_room_members) — optional
- `ServiceInfo` (service_name, as_any)
- **Why:** WhatsApp doesn't support edits/members; forcing stub implementations violates ISP. New protocols only implement what they support.
- **Implication:** Service implementations can opt into features. Bridge code uses trait objects with feature detection (e.g., `if let Some(editor) = svc.as_any().downcast_ref::<&dyn MessageEditor>()`).
- **Alternatives considered:**
  - Keep single trait with default no-op impls — rejected: hides missing capabilities, no compile-time feedback
  - Feature flags on Service — rejected: complicates trait, still forces stubs

### Q3: Where to put message formatting logic?
**Decision:** Extract `MessageFormatter` trait into `bridge/formatter.rs` with per-service implementations in each service module
- **Why:** Matrix uses markdown+HTML, Discord uses markdown, WhatsApp uses markdown-like. Currently duplicated in each service's `send_message`.
- **Implication:** Services delegate formatting to their formatter. Bridge dispatcher uses formatter when building outgoing messages. Testable in isolation.
- **Alternatives considered:**
  - Keep in services — rejected: duplication, hard to test formatting independently
  - Single formatter with config — rejected: protocols differ too much

### Q4: How to handle service construction in main.rs?
**Decision:** Extract `ServiceBuilder` (constructs Box<dyn Service> from config) and `ServiceRegistry` (manages HashMap<String, Arc<Mutex<...>>> lifecycle)
- **Why:** main.rs mixes config parsing, service instantiation, connection, GIF resolver wiring, and signal handling. Hard to test or reuse.
- **Implication:** main.rs becomes: load config → builder.build_all(config) → registry.start_all(tx) → coordinator.run()
- **Alternatives considered:**
  - Keep in main.rs with helper functions — rejected: doesn't improve modularity
  - DI container — rejected: overkill for this codebase

### Q5: Bridge matching logic location?
**Decision:** Extract `BridgeMatcher` into `bridge/matcher.rs` — takes Config, returns matching ChannelConfigs for a source service/channel
- **Why:** Currently inline in route_message (lines 109-127). Used by route_message, handle_edit, handle_reaction. Duplicated logic.
- **Implication:** Single source of truth for "which bridge/channel does this message belong to?"
- **Alternatives considered:**
  - Keep as method on Config — rejected: Config should be data only

### Q6: Alias resolution location?
**Decision:** Extract `AliasResolver` into `bridge/alias_resolver.rs` — applies ChannelConfig.aliases to sender_id
- **Why:** Currently inline in route_message (lines 212-223). Reusable, testable independently.
- **Implication:** Clean separation: matcher finds config → resolver applies aliases → dispatcher sends

### Q7: Media/attachment handling per channel config?
**Decision:** Extract `MediaHandler` into `bridge/media.rs` — strips attachments if `!enable_media`, handles caption formatting
- **Why:** Currently inline in route_message (lines 240-246). Policy varies per target channel.
- **Implication:** Dispatcher calls media_handler.process(outgoing_msg, target_channel_config)

### Q8: Deduplication wrapper?
**Decision:** Add `Deduplicator` in `bridge/deduplicator.rs` wrapping MessageStore.exists
- **Why:** Currently inline in route_message (lines 90-104). Cleaner API: `deduplicator.check_and_mark(source_service, channel, id)`
- **Implication:** Single place for deduplication policy (could add TTL, metrics later)

### Q9: Breaking changes to public API?
**Decision:** Yes, drop the `Service` facade entirely. Internal code uses fine-grained traits directly.
- **Why:** User confirmed breaking changes OK. No external consumers of the `Service` trait.
- **Implication:** `ServiceBuilder` returns concrete service types or a `ServiceImpl` trait combining what that service supports. Bridge code downcasts only for optional traits (`MessageEditor`, `MemberLister`). `MessageSender`/`Connectable`/`ReactionSender`/`ServiceInfo` are always available.
- **Alternatives considered:**
  - Facade trait with blanket impl — rejected: unnecessary complexity when no external impls exist

### Q10: Module organization for services?
**Decision:** Each service gets a subdirectory with focused files: client.rs, events.rs, sender.rs, editor.rs, reactor.rs, members.rs, formatter.rs
- **Why:** Current 400-560 line files mix connection, event handling, and each Service trait method. Splitting by capability improves navigation.
- **Implication:** MatrixService becomes a struct composing these capabilities. Easier to swap implementations.

### Key Assumptions
- No changes to persistence layer or SQLite schema
- No changes to GIF resolution module
- Config.yaml format stays the same
- Integration tests continue to work without modification
- tokio async runtime remains unchanged

### Trade-offs Explicitly Considered
| Decision | Trade-off | Chosen Because |
|----------|-----------|----------------|
| Split Service trait (no facade) | More trait objects, downcasting in bridge | Compile-time feature detection, ISP compliance |
| Per-service subdirectories | More files to navigate | Clear separation, easier to find capability code |
| Extracted formatters | Indirection in send_message | Testability, eliminates duplication |
| BridgeMatcher separate | Extra allocation for match results | Single source of truth, reusable by edit/reaction handlers |

### Q11: How to structure EventRouter?
**Decision:** Single `EventRouter` in `bridge/router.rs` with `route(event, services, config, store)` that matches on ServiceEvent enum and delegates to handler types
- **Why:** Centralizes event dispatch logic, keeps BridgeCoordinator thin. Each handler (dispatcher, edit, reaction, status) is a separate type.
- **Implication:** BridgeCoordinator.start() creates router and runs `while let Some(event) = rx.recv().await { router.route(event, ...).await }`
- **Alternatives considered:** 
  - Direct match in BridgeCoordinator — rejected: doesn't decompose the type
  - Channel per event type — rejected: over-engineering for 4 event types

### Q12: Service subdirectory structure?
**Decision:** Each service in `services/{matrix,discord,whatsapp}/` with: mod.rs (struct + trait impls), client.rs (protocol client wrapper), events.rs (incoming event handlers), sender.rs (MessageSender impl), editor.rs (MessageEditor impl or stub), reactor.rs (ReactionSender impl), members.rs (MemberLister impl or stub), formatter.rs (MessageFormatter impl)
- **Why:** Current files mix all concerns. Splitting by capability (sending, editing, reacting, members) makes it obvious what each protocol supports.
- **Implication:** WhatsApp editor.rs and members.rs contain stub implementations (return Ok(()) / Err(unimplemented)). Matrix/Discord have real impls.
- **Alternatives considered:**
  - Keep single file per service — rejected: 500+ lines hard to navigate
  - Split by layer (connection, events, sending) — rejected: capabilities are better boundaries

### Q13: Unit test organization?
**Decision:** Co-locate unit tests in `__tests__/` subdirectory within each module directory, named `module.unit.test.rs`
- **Why:** Keeps tests next to code, follows Rust conventions. Integration tests stay in top-level `tests/`.
- **Implication:** `bridge/__tests__/matcher.unit.test.rs`, `services/matrix/__tests__/sender.unit.test.rs`, etc.
- **Alternatives considered:**
  - Top-level tests/ directory — rejected: too far from code under test
  - #[cfg(test)] modules in same file — rejected: clutters implementation

### CONTEXT.md Updates
- **BridgeMatcher**: Type that resolves a (service, channel) pair to the set of ChannelConfigs in the same bridge
- **EventRouter**: Type that receives ServiceEvent and delegates to MessageDispatcher, EditHandler, ReactionHandler, StatusHandler
- **MessageDispatcher**: Type that forwards a ServiceMessage to all target channels in a bridge, applying formatting, media policy, aliases
- **ServiceBuilder**: Type that constructs Service instances from Config
- **ServiceRegistry**: Type that manages the lifecycle (connect, start, shutdown) of a collection of services
- **MessageFormatter**: Trait for protocol-specific message rendering (text + attachments)
- **AliasResolver**: Type that applies ChannelConfig.aliases to sender IDs
- **MediaHandler**: Type that applies per-channel media policy (strip/keep attachments)
- **Deduplicator**: Type that wraps MessageStore.exists for deduplication checks
- **EditHandler**: Type that processes ServiceUpdate and forwards edits to mapped destination messages
- **ReactionHandler**: Type that processes ServiceReaction and forwards reactions to mapped destination messages
- **StatusHandler**: Type that handles .status command and returns bridge/service/room status
- **Connectable**: Trait for service connection lifecycle (connect, disconnect, wait_until_ready, is_connected)
- **MessageSender**: Trait for sending messages (send_message)
- **MessageEditor**: Trait for editing messages (edit_message) — optional capability
- **ReactionSender**: Trait for sending reactions (react_to_message) — optional capability
- **MemberLister**: Trait for listing channel members (get_room_members) — optional capability
- **ServiceInfo**: Trait for service metadata (service_name, as_any)