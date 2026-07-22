Status: pending

# 03: GIF Provider Tests

## What to build

One integration test per *enabled* GIF provider in `media.gif_providers` that:
1. Creates `GifResolver` from config
2. Calls `resolve(test_url).await` with a known test URL for that provider
3. Asserts result is `Some(direct_media_url)`
4. Sends `HEAD` request to `direct_media_url`, asserts `Content-Length <= max_size_mb * 1024 * 1024`
5. Logs resolved URL and size

Skips with warning if provider disabled, API key missing, or no test URL known.

## Files to create/modify

- `tests/integration_tests.rs` (add tests)

## Test approach

- Known test URLs (to be provided/verified):
  - Tenor: `https://tenor.com/view/...` 
  - Giphy: `https://giphy.com/gifs/...`
  - Imgur: `https://imgur.com/...`
  - Kliqy: `https://kliqy.com/...`
- Use `reqwest::Client::head()` for size check (no download)
- Sequential execution

## Acceptance criteria

- [ ] `test_tenor_resolver` passes if Tenor enabled + API key present
- [ ] `test_giphy_resolver` passes if Giphy enabled + API key present
- [ ] `test_imgur_resolver` passes if Imgur enabled + API key present
- [ ] `test_klipy_resolver` passes if Kliqy enabled + API key present
- [ ] Each test skips with warning if provider disabled or API key missing
- [ ] Size validation uses `config.media.max_size_mb` (default 0 = no limit check)

## Blocked by

01-test-infrastructure.md