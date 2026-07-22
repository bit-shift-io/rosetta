Status: pending

# 04: Test Runner Config

## What to build

Ensure integration tests run correctly:
- Verify `Cargo.toml` has `[[test]]` entry or standard `tests/` auto-discovery works
- Add `serial_test = "3"` to `[dev-dependencies]` if using `#[serial]`
- Document run command: `cargo test --test integration_tests -- --test-threads=1`
- Verify all tests pass locally with valid config

## Files to create/modify

- `Cargo.toml` (dev-dependencies if needed)
- `README.md` or `AGENTS.md` (document test command)

## Test approach

- Run full suite: `cargo test --test integration_tests -- --test-threads=1`
- Verify no compilation errors
- Verify skip behavior works (remove creds temporarily)

## Acceptance criteria

- [ ] `cargo test --test integration_tests` runs all tests sequentially
- [ ] Dev dependencies include `serial_test` if used
- [ ] Test command documented in `AGENTS.md` or `README.md`

## Blocked by

01-test-infrastructure.md, 02-service-connectivity.md, 03-gif-providers.md