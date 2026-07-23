# Refactor: Modularity & Code Organization

## Problem Statement
The Rosetta codebase has grown organically with several large modules that mix multiple responsibilities. `bridge.rs` (426 lines) handles message routing, edit forwarding, reaction sync, status commands, deduplication, and bridge matching all in one type. Each service implementation (Matrix 554 lines, Discord 560 lines, WhatsApp 404 lines) combines connection logic, event handling, message formatting, and protocol-specific operations. `main.rs` (146 lines) mixes logging config, service construction, connection orchestration, and signal handling. This makes the code hard to navigate, test in isolation, and extend with new protocols or features.

## Solution
Refactor the codebase into smaller, single-responsibility modules with clear boundaries:
- Decompose `BridgeCoordinator` into focused types: `EventRouter`, `MessageDispatcher`, `EditHandler`, `ReactionHandler`, `StatusHandler`
- Extract service-agnostic utilities: `MessageFormatter`, `MediaHandler`, `AliasResolver`, `BridgeMatcher`
- Split `Service` trait into fine-grained traits (`Connectable`, `MessageSender`, `MessageEditor`, `ReactionSender`, `MemberLister`, `ServiceInfo`) — no facade, internal code uses trait objects directly
- Introduce `ServiceRegistry` / `ServiceBuilder` to encapsulate service lifecycle from `main.rs`
- Create a `bridge` module directory with sub-modules for each routing concern
- Add unit tests for all new extracted modules

## User Stories
1. As a **maintainer**, I want `BridgeCoordinator` split into focused types so I can understand and modify routing logic without reading 400+ lines.
2. As a **contributor**, I want service implementations to share common message formatting and media handling so I don't duplicate code when adding a new protocol.
3. As a **tester**, I want to unit-test message routing, edit handling, and reaction sync independently without spinning up real service connections.
4. As a **developer**, I want a clear `ServiceRegistry` so adding a new protocol only requires implementing focused traits, not touching `main.rs`.
5. As a **reviewer**, I want each module to have a single reason to change so PRs are smaller and easier to evaluate.
6. As a **newcomer**, I want the project structure to reflect the domain (bridges, services, persistence, GIF resolution) so I can find code quickly.
7. As a **maintainer**, I want the `Service` trait split so optional features (edits, reactions, members) don't force stub implementations on every protocol.
8. As a **developer**, I want message formatting (markdown, mentions, media captions) extracted so protocol-specific rendering is isolated and testable.

## Implementation Decisions
- **Module structure**: Create `src/bridge/` directory with `router.rs`, `dispatcher.rs`, `edit_handler.rs`, `reaction_handler.rs`, `status_handler.rs`, `matcher.rs`, `formatter.rs`, `alias_resolver.rs`
- **Service traits**: Split into `Connectable`, `MessageSender`, `MessageEditor`, `ReactionSender`, `MemberLister`, `ServiceInfo`; no facade trait — internal code uses fine-grained trait objects directly
- **Service lifecycle**: Extract `ServiceBuilder` (constructs services from config) and `ServiceRegistry` (manages `Arc<Mutex<Box<dyn Service>>>` map) from `main.rs`
- **Message formatting**: Create `MessageFormatter` trait with `format_text`, `format_attachment_caption` methods; each service provides implementation
- **Bridge matching**: Extract `BridgeMatcher` that finds matching bridges and channel configs for a source message
- **Deduplication**: Keep in `MessageStore` but add `Deduplicator` wrapper for cleaner API
- **Configuration**: No changes to `config.rs`; new types consume existing `Config` and `ChannelConfig`
- **Error handling**: Use `anyhow::Result` throughout; new modules return domain-specific error types where helpful

## Testing Decisions
- Unit test each new module in isolation: `router`, `dispatcher`, `edit_handler`, `reaction_handler`, `status_handler`, `matcher`, `formatter`, `alias_resolver`, `service_builder`, `service_registry`
- Use mock implementations of service traits for routing tests (no external dependencies)
- Integration tests remain in `tests/integration_tests.rs` (run with `--test-threads=1`)
- Test file naming: `mod_name.unit.test.rs` co-located in each module directory
- Property-based tests for `BridgeMatcher` (various bridge/channel configurations)

## Out of Scope
- Adding new protocols (Signal, Telegram, Slack, etc.)
- Changing the SQLite schema or persistence logic
- Modifying GIF resolution providers
- Changing the YAML config format
- Runtime hot-reload of bridges/services
- Metrics/observability instrumentation

## File Structure
```
src/
├── main.rs                    # Thin entry: logging, config, registry, coordinator, signals
├── lib.rs                     # Re-exports
├── config.rs                  # Unchanged
├── persistence.rs             # Unchanged (MessageStore)
├── gif/                       # Unchanged
├── bridge/
│   ├── mod.rs                 # BridgeCoordinator (thin facade)
│   ├── router.rs              # EventRouter - routes ServiceEvent to handlers
│   ├── dispatcher.rs          # MessageDispatcher - forwards messages to target channels
│   ├── edit_handler.rs        # EditHandler - processes ServiceUpdate
│   ├── reaction_handler.rs    # ReactionHandler - processes ServiceReaction
│   ├── status_handler.rs      # StatusHandler - handles .status command
│   ├── matcher.rs             # BridgeMatcher - finds bridge/channel for source
│   ├── formatter.rs           # MessageFormatter trait + implementations
│   ├── alias_resolver.rs      # AliasResolver - applies channel aliases
│   ├── deduplicator.rs        # Deduplicator - wraps MessageStore.exists
│   └── media.rs               # MediaHandler - strips/transforms attachments per channel config
├── services/
│   ├── mod.rs                 # Service traits (split) + re-exports
│   ├── builder.rs             # ServiceBuilder - constructs services from config
│   ├── registry.rs            # ServiceRegistry - manages service map lifecycle
│   ├── matrix/
│   │   ├── mod.rs
│   │   ├── client.rs          # MatrixClient wrapper
│   │   ├── events.rs          # Event handlers
│   │   ├── sender.rs          # MessageSender impl
│   │   ├── editor.rs          # MessageEditor impl
│   │   ├── reactor.rs         # ReactionSender impl
│   │   ├── members.rs         # MemberLister impl
│   │   └── formatter.rs       # MatrixMessageFormatter
│   ├── discord/
│   │   ├── mod.rs
│   │   ├── client.rs
│   │   ├── events.rs
│   │   ├── sender.rs
│   │   ├── editor.rs
│   │   ├── reactor.rs
│   │   ├── members.rs
│   │   └── formatter.rs
│   └── whatsapp/
│       ├── mod.rs
│       ├── client.rs
│       ├── events.rs
│       ├── sender.rs
│       ├── editor.rs (stub)
│       ├── reactor.rs
│       ├── members.rs (stub)
│       └── formatter.rs
```

## Acceptance Criteria
- [ ] `bridge.rs` reduced to <100 lines (thin facade over new modules)
- [ ] Each service implementation split into ≤5 files, each <200 lines
- [ ] `Service` trait split into 6 focused traits (no facade)
- [ ] `main.rs` reduced to <80 lines (delegates to `ServiceBuilder`/`ServiceRegistry`)
- [ ] Unit tests exist for all new bridge modules (≥80% coverage on routing logic)
- [ ] Unit tests exist for `ServiceBuilder` and `ServiceRegistry`
- [ ] All existing integration tests pass without modification
- [ ] `cargo fmt` and `cargo clippy --all-targets --all-features` pass

## References
- Current codebase: `src/bridge.rs`, `src/services/*.rs`, `src/main.rs`
- ADR (to be created): Module decomposition strategy
- CONTEXT.md: Domain terms (Bridge, Service, ChannelConfig, ServiceEvent)