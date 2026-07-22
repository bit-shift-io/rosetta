# Glossary

### Bridge
**Definition:** A configured connection between two or more chat channels across different protocols (Matrix, WhatsApp, Discord). Messages, edits, reactions, and media are forwarded bidirectionally between channels in the same bridge.

**Boundary:** A bridge is a logical grouping in `config.yaml`; it does not imply a single process or thread. Multiple bridges can run in one Rosetta instance.

---

### Service
**Definition:** An implementation of the `Service` trait representing a connection to one chat protocol (Matrix, WhatsApp, Discord). Manages authentication, event streaming, and outbound API calls.

**Boundary:** Not a "bridge" — a bridge *contains* channels from multiple services. A service may participate in multiple bridges.

---

### ChannelConfig
**Definition:** A single entry in a bridge's channel list, specifying `service` name, `channel` identifier, and bridging options (`display_names`, `enable_media`, `bridge_own_messages`, `aliases`).

**Boundary:** Protocol-specific identifier format (Matrix room ID, WhatsApp chat JID, Discord channel ID). Not a service instance.

---

### GIF Provider
**Definition:** An external API (Tenor, Giphy, Imgur, Kliqy) that resolves a GIF page URL to a direct media file URL (MP4/GIF/WebM). Configured in `media.gif_providers` with API keys.

**Boundary:** Only handles URL resolution. Does not download or proxy media — the resolver selects the best URL under size limits.

---

### Integration Test
**Definition:** Tests in `tests/` directory that exercise the full application against real external services (Matrix, WhatsApp, Discord, GIF APIs). Require network, credentials, and `data/config.yaml`.

**Boundary:** Distinct from unit tests (which would live in `src/` and mock dependencies). Integration tests are the only test type in this project currently.

---

### Max Size MB
**Definition:** Configuration value (`media.max_size_mb`) limiting uploaded media size. GIF resolver filters candidate URLs by `Content-Length` header to respect this limit (primarily for Matrix upload restrictions).

**Boundary:** `0` means no limit (config default). The resolver treats 0 as "no limit check."

---

### MessageStore
**Definition:** SQLite-backed persistence (`data/message_history.db`) mapping source message IDs to destination message IDs across services. Enables edit forwarding, reaction sync, and deduplication.

**Boundary:** Does not store message content — only ID mappings. Append-only; no cleanup logic.