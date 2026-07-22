### Q1: Should all providers (Tenor, Giphy, Klipy, Imgur) use the unified map structure?

**Decision:** Yes, unify all providers under `media.gif_providers` map with `enabled` + `api_key`

- **Why:** Current config has inconsistent field names (`api_key` vs `client_id`). Unifying makes config extensible and discoverable. Adding a new provider only requires config changes, not code changes.
- **Implication:** `GifProviderConfig` struct replaced with `HashMap<String, GifProviderEntry>`. All provider initialization logic in `GifResolver::new()` iterates the map.
- **Alternatives considered:** Keep Tenor/Imgur as separate fields - rejected because it perpetuates inconsistency and makes adding providers harder.

---

### Q2: Typo in config_example.yaml - `apit_key` vs `api_key`

**Decision:** Fix the typo - use `api_key` consistently for all providers

- **Why:** Typo in example config would cause confusion. The code should use the correct field name.
- **Implication:** Config struct field must be `api_key` (not `apit_key`).

---

### Q3: Should `max_size_mb` be global or per-provider override?

**Decision:** Global only (at `media.max_size_mb`)

- **Why:** Simpler configuration, single source of truth for size limits. Per-provider override adds complexity without clear use case.
- **Implication:** Single `max_upload_size` (bytes) passed to all providers. Providers that support size filtering (Klipy) use it; others receive it via `set_max_upload_size` but may ignore.

---

### Q4: Backward compatibility with old config format?

**Decision:** Breaking change - old config format will fail to load

- **Why:** User confirmed breaking change is acceptable. Maintaining dual parsing adds complexity and technical debt.
- **Implication:** Users must migrate config.yaml to new format. Document migration in README/CHANGELOG.

---

### Q5: How should fallback scraper whitelist work with new config?

**Decision:** Derive whitelist from enabled providers' known domains

- **Why:** Eliminates separate `media_whitelist` maintenance. If tenor is enabled, tenor.com is auto-whitelisted for fallback scraping.
- **Implication:** `GifResolver` builds domain list from enabled providers in the map. Provider domain knowledge lives in each provider's `supports_domain()` method.

---

### Q6: Should all providers implement `set_max_upload_size`?

**Decision:** Yes, add to `GifProvider` trait (default no-op implementation)

- **Why:** Consistent interface. Klipy already implements size filtering. Tenor/Giphy/Imgur APIs may support size params in future. No-op default avoids breaking existing providers.
- **Implication:** Update `GifProvider` trait in `providers/mod.rs` with default impl. Update Tenor, Giphy, Imgur providers.

---

### Key Assumptions

- `config_example.yaml` is the source of truth for new format
- `max_size_mb` of 10 = 10 * 1024 * 1024 = 10,485,760 bytes
- Breaking change means no migration tool needed
- Fallback scraper domains: tenor.com, giphy.com, klipy.com, imgur.com (from provider implementations)

---

### Updated CONTEXT.md Entries

**MediaConfig** - Top-level media configuration section containing `max_size_mb` and `gif_providers` map

**GifProviderEntry** - Single provider configuration with `enabled: bool` and `api_key: String` fields

**max_size_mb** - Global maximum media size in megabytes, converted to bytes for provider APIs