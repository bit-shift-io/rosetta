# Integration Tests for Rosetta

## Problem Statement

The Rosetta chat bridge has no automated tests to verify that:
1. Configured services (Matrix, WhatsApp, Discord) can successfully connect and authenticate
2. GIF resolvers correctly resolve provider URLs to direct media files respecting size limits

Without these tests, regressions in service connectivity or GIF resolution go undetected until manual testing.

## Solution

Add integration tests that run against real configured services and real GIF provider APIs. Tests run sequentially via `cargo test`, skip gracefully with a warning if credentials are missing, and require no cleanup.

## User Stories

1. As a developer, I want `cargo test` to verify all configured services connect successfully, so I catch authentication/config regressions early.
2. As a developer, I want each GIF provider (Tenor, Giphy, Imgur, Kliqy) tested against a known URL, so I know the resolver chain works end-to-end.
3. As a developer, I want GIF resolution to respect the `max_size_mb` config limit, so oversized media isn't sent to services with upload limits.
4. As a CI maintainer, I want tests to skip with a warning when credentials are missing, so CI passes without secrets.

## Implementation Decisions

- **Test organization**: Single integration test file at `tests/integration_tests.rs` (standard Rust location for integration tests)
- **Execution**: Sequential (`--test-threads=1`) to avoid service connection conflicts
- **Config loaded
- **Config**: Uses existing `data/config.yaml` (production config)
- **Services**: Test `connect()` + `start()` + `wait_until_ready()` for each configured service
- **GIF providers**: Test each enabled provider in `media.gif_providers` config with a known test URL per provider
- **Size validation**: Verify resolved media URL's `Content-Length` <= `max_size_mb * 1024 * 1024`
- **Skipping**: Use `#[ignore]` or conditional skip with warning log when credentials missing
- **No cleanup**: Services disconnect on process exit; DB is append-only

## Testing Decisions

- **Modules tested**: `services::matrix`, `services::whatsapp`, `services::discord`, `gif::resolver`, `gif::providers`
- **Test type**: Integration tests (external dependencies)
- **File naming**: `tests/integration_tests.rs` (Rust convention for integration tests)
- **Prior art**: None — this is the first test suite in the project
- **Test isolation**: Sequential execution; each service test independent

## Out of Scope

- Unit tests for individual modules (config parsing, persistence, bridge logic)
- Message routing/bridging integration tests
- Reaction/edit/delete event tests
- Reconnection/failure recovery tests
- Mock-based tests
- CI/CD pipeline setup
- Test fixtures/recorded responses

## File Structure

```
tests/
└── integration_tests.rs
```

## Acceptance Criteria

- [ ] `cargo test --test integration_tests` runs all integration tests sequentially
- [ ] Each configured service (Matrix, WhatsApp, Discord) has a test verifying `connect()` + `start()` + `wait_until_ready()` succeed
- [ ] Each enabled GIF provider has a test resolving a known URL to a direct media URL
- [ ] GIF resolution test verifies `Content-Length` <= configured `max_size_mb`
- [ ] Tests skip with a warning log (not failure) if service credentials missing
- [ ] Tests skip with a warning log if GIF provider API key missing
- [ ] No test cleanup required (process exit handles disconnect)

## References

- AGENTS.md: Technology stack, service traits, GIF resolver architecture
- src/services/mod.rs: `Service` trait (`connect`, `start`, `wait_until_ready`)
- src/gif/resolver.rs: `GifResolver` and provider chain
- data/config.yaml: Service and media configuration