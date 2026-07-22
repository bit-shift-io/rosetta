# Handoff: Integration Tests

## Summary

Add the project's first test suite: integration tests in `tests/integration_tests.rs` that verify:
1. **Service connectivity** — Each configured service (Matrix, WhatsApp, Discord) can `connect()` + `start()` + `wait_until_ready()` successfully
2. **GIF resolver** — Each enabled GIF provider resolves a known test URL to a direct media URL with `Content-Length` ≤ `media.max_size_mb`

Tests run sequentially via `cargo test --test integration_tests` against `data/config.yaml`. Missing credentials = skip with warning (not failure).

## Implementation Order

1. **Issue 01: Test Infrastructure** — Create `tests/integration_tests.rs`, load config, shared setup
2. **Issue 02: Service Connectivity Tests** — One test per service type using `Service` trait methods
3. **Issue 03: GIF Provider Tests** — One test per enabled provider resolving known URL + size check
4. **Issue 04: Test Runner Config** — Ensure `Cargo.toml` has `[[test]]` if needed; verify `cargo test --test integration_tests` works

## Key Files

- **New:** `tests/integration_tests.rs`
- **Read:** `src/config.rs`, `src/services/mod.rs`, `src/services/*.rs`, `src/gif/resolver.rs`, `src/gif/providers/*.rs`, `data/config.yaml`

## Gotchas

- **Config loading:** Reuse `Config::load("data/config.yaml")` from `main.rs`
- **Service trait:** Methods are async on `&mut self` — need `Arc<Mutex<Box<dyn Service>>>`
- **GIF resolver:** `GifResolver::new()` takes `&MediaConfig` and `bool` (debug); call `resolve(url).await`
- **Size check:** Use `reqwest::Client::head(url).send().await?.content_length()`
- **Skipping:** Use `if cfg.missing { eprintln!("WARN: skipping..."); return; }` not `#[ignore]` (dynamic)
- **Sequential:** Add `#[serial]` from `serial_test` crate or run with `--test-threads=1`

## Dependencies

- Add `serial_test = "3"` to dev-dependencies if using `#[serial]`
- Or document `--test-threads=1` requirement

## Links

- PRD: `.scratch/integration-tests/PRD.md`
- Decisions: `.scratch/integration-tests/DECISIONS.md`
- Issues: `.scratch/integration-tests/issues/`