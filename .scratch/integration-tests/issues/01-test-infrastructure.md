Status: pending

# 01: Test Infrastructure

## What to build

Create `tests/integration_tests.rs` with shared test infrastructure:
- Load `Config` from `data/config.yaml`
- Helper to create services from config (reusing logic from `main.rs`)
- Optional: `serial_test` crate for sequential execution

## Files to create/modify

- `tests/integration_tests.rs` (new)
- `Cargo.toml` (add `serial_test = "3"` to `[dev-dependencies]` if using)

## Test approach

- Verify config loads without panic
- Verify services can be instantiated from config
- No external calls yet — pure setup

## Acceptance criteria

- [ ] `cargo test --test integration_tests` compiles and runs empty test suite
- [ ] Config loads from `data/config.yaml`
- [ ] Services instantiate without `connect()`/`start()`

## Blocked by

None — can start immediately