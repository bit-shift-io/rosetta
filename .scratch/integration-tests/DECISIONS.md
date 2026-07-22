# Decisions: Integration Tests

### Q1: What types of testing to prioritize?
**Decision:** Integration tests only — service connectivity + GIF resolver validation
- **Why:** User explicitly requested only these two test categories; no unit tests needed
- **Implication:** Test file lives in `tests/` (not `src/`); tests real external dependencies
- **Alternatives considered:** Unit tests, mock-based tests — rejected per user direction

### Q2: How to handle external dependencies?
**Decision:** Real services with real credentials from `data/config.yaml`
- **Why:** User has services already configured; wants real validation
- **Implication:** Tests require network + valid credentials; skip if missing
- **Alternatives considered:** Mock servers, recorded fixtures, mock traits — rejected

### Q3: Specific test scenarios
**Decision:** 
- Services: `connect()` + `start()` + `wait_until_ready()` success
- GIF: Each enabled provider resolves known URL; verify `Content-Length` <= `max_size_mb`
- **Why:** Minimal viable connectivity + size validation for Matrix upload limits
- **Alternatives considered:** Full message routing, reconnection tests — out of scope

### Q4: Execution and structure
**Decision:** 
- Standard `tests/integration_tests.rs`
- Run via `cargo test --test integration_tests`
- Sequential execution (`--test-threads=1`)
- Use production `data/config.yaml`
- **Why:** Rust conventions; avoids service connection conflicts
- **Alternatives considered:** Separate binary, feature-gated — standard approach sufficient

### Q5: Other constraints
**Decision:**
- Skip with warning (not failure) if credentials missing
- Known GIF URLs provided later / searched online
- No cleanup required
- **Why:** CI-friendly; user will provide URLs; process exit handles resources
- **Alternatives considered:** Fail CI, fixtures, explicit disconnect — rejected

---

## Key Assumptions

1. `data/config.yaml` exists and contains valid service + media config
2. At least one service is configured (Matrix, WhatsApp, or Discord)
3. `media.gif_providers` has at least one enabled provider with API key
4. `media.max_size_mb` is set (default 0 = no limit in config, but resolver should handle)
5. Services don't conflict when connecting sequentially (different accounts/tokens)
6. GIF provider test URLs remain stable (or user will update)

## Trade-offs Explicitly Considered

| Decision | Trade-off |
|----------|-----------|
| Real services | Flaky if network/auth issues; but catches real problems |
| Sequential only | Slower; but avoids token/connection conflicts |
| Skip on missing creds | CI passes without secrets; but gaps in coverage if creds missing |
| No cleanup | Simpler; but leaves DB entries (append-only, harmless) |
| Single test file | Simple; but all tests in one binary (standard for Rust) |

---

## CONTEXT.md Updates

### `Integration Test`
**Definition:** Tests in `tests/` directory that exercise the full application against real external services (Matrix, WhatsApp, Discord, GIF APIs). Unlike unit tests, they require network, credentials, and config.

### `GIF Provider`
**Definition:** External service (Tenor, Giphy, Imgur, Kliqy) that resolves a GIF page URL to a direct media file (MP4/GIF/WebM). Configured in `media.gif_providers` with API keys.

### `Max Size MB`
**Definition:** Configuration value (`media.max_size_mb`) limiting uploaded media size. GIF resolver must filter/respect this when selecting media URLs for Matrix (which enforces upload limits).