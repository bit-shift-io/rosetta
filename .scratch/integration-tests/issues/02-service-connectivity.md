Status: pending

# 02: Service Connectivity Tests

## What to build

One integration test per configured service type (Matrix, WhatsApp, Discord) that:
1. Creates the service from config
2. Calls `connect().await` — asserts `Ok(())`
3. Calls `start(sender).await` — asserts `Ok(())`
4. Calls `wait_until_ready().await` — asserts `Ok(())`
4. Logs success

Skips with warning log if service config missing or credentials incomplete.

## Files to create/modify

- `tests/integration_tests.rs` (add tests)

## Test approach

- Use `Service` trait methods directly
- Run sequentially (shared config, potential token conflicts)
- Each test independent — can run subset

## Acceptance criteria

- [ ] `test_matrix_connectivity` passes if Matrix configured with valid creds
- [ ] `test_whatsapp_connectivity` passes if WhatsApp configured with valid session
- [ ] `test_discord_connectivity` passes if Discord configured with valid token
- [ ] Each test skips with `eprintln!("WARN: Skipping {service} — missing/invalid config")` if not configured
- [ ] No test fails due to missing config

## Blocked by

01-test-infrastructure.md