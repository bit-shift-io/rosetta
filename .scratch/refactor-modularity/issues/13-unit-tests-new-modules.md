Status: done

# 13-unit-tests-new-modules

## What to build
Add unit tests for all newly extracted modules. Each module gets a `__tests__/` subdirectory with `*.unit.test.rs` files.

Modules needing tests:
1. `bridge/matcher.rs` → `bridge/__tests__/matcher.unit.test.rs`
2. `bridge/alias_resolver.rs` → `bridge/__tests__/alias_resolver.unit.test.rs`
3. `bridge/dispatcher.rs` → `bridge/__tests__/dispatcher.unit.test.rs`
4. `bridge/edit_handler.rs` → `bridge/__tests__/edit_handler.unit.test.rs`
5. `bridge/reaction_handler.rs` → `bridge/__tests__/reaction_handler.unit.test.rs`
6. `bridge/status_handler.rs` → `bridge/__tests__/status_handler.unit.test.rs`
7. `bridge/deduplicator.rs` → `bridge/__tests__/deduplicator.unit.test.rs`
8. `bridge/media_handler.rs` → `bridge/__tests__/media_handler.unit.test.rs`
9. `bridge/formatter.rs` → `bridge/__tests__/formatter.unit.test.rs`
10. `bridge/router.rs` → `bridge/__tests__/router.unit.test.rs`
11. `services/builder.rs` → `services/__tests__/builder.unit.test.rs`
12. `services/registry.rs` → `services/__tests__/registry.unit.test.rs`
13. `services/traits.rs` → `services/__tests__/traits.unit.test.rs` (blanket impl test)

Use mock implementations of service traits for bridge tests (no external deps).

## Files to create
- bridge/__tests__/*.unit.test.rs (10 files)
- services/__tests__/*.unit.test.rs (3 files)

## Test approach
- Use `mockall` or manual mock structs for service traits
- Property-based tests for BridgeMatcher (proptest)
- Each test file focuses on one module's public API
- Test happy path + error cases + edge cases

## Acceptance criteria
- [ ] All 13 test files created
- [ ] Each module has ≥5 unit tests covering core behavior
- [ ] cargo test --lib passes
- [ ] Tests run in <5 seconds
- [ ] No external dependencies (network, DB) in unit tests
- [ ] Mock services implement fine-grained traits from issue 04

## Blocked by
01-12 (all modules must exist first)