Status: done

# 12-bridge-module-structure

## What to build
Reorganize `src/bridge.rs` into a `src/bridge/` module directory:
```
src/bridge/
├── mod.rs              # Re-exports, BridgeCoordinator (thin facade)
├── router.rs           # EventRouter (issue 11)
├── matcher.rs          # BridgeMatcher (issue 01)
├── alias_resolver.rs   # AliasResolver (issue 01)
├── dispatcher.rs       # MessageDispatcher (issue 07)
├── edit_handler.rs     # EditHandler (issue 08)
├── reaction_handler.rs # ReactionHandler (issue 09)
├── status_handler.rs   # StatusHandler (issue 10)
├── deduplicator.rs     # Deduplicator (issue 02)
├── media_handler.rs    # MediaHandler (issue 02)
├── formatter.rs        # MessageFormatter trait (issue 03)
```

Update `src/lib.rs` to `pub mod bridge;` (already done) and ensure all submodules are accessible.

## Files to create/modify
- src/bridge/mod.rs (modify - new structure)
- src/bridge/*.rs (all new files from issues 01-11)
- src/lib.rs (verify re-exports)
- src/bridge.rs (delete or keep as thin re-export shim)

## Test approach
- Cargo check passes
- All unit tests in bridge/ submodules pass
- Integration tests pass

## Acceptance criteria
- [ ] src/bridge/ directory exists with all submodules
- [ ] Each submodule has clear single responsibility
- [ ] BridgeCoordinator in mod.rs is <100 lines (thin facade)
- [ ] All internal types are pub(crate) or pub where needed
- [ ] No circular dependencies between bridge submodules
- [ ] lib.rs re-exports bridge types correctly
- [ ] cargo test --lib passes
- [ ] cargo test --test integration_tests passes

## Blocked by
01-11 (all bridge submodules)