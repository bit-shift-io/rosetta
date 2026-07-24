# Glossary

### Bridge
**Definition:** A configured connection between two or more chat channels across different protocols (Matrix, WhatsApp, Discord). Messages, edits, reactions, and media are forwarded bidirectionally between channels in the same bridge.

**Boundary:** A bridge is a logical grouping in `config.yaml`; it does not imply a single process or thread. Multiple bridges can run in one Rosetta instance.

---

### Service
**Definition:** A concrete type implementing the fine-grained service traits (`Connectable`, `MessageSender`, `MessageEditor`, `ReactionSender`, `MemberLister`, `ServiceInfo`) for a specific chat protocol (Matrix, WhatsApp, Discord). Manages authentication, event streaming, and outbound API calls.

**Boundary:** Not a "bridge" — a bridge *contains* channels from multiple services. A service may participate in multiple bridges. No single `Service` trait exists; code uses the fine-grained traits directly.

---

### ChannelConfig
**Definition:** A single entry in a bridge's channel list, specifying `service` name, `channel` identifier, `room_name` (human-readable display name populated from service API on connect), and bridging options (`display_names`, `enable_media`, `bridge_own_messages`, `aliases`).

**Boundary:** Protocol-specific identifier format (Matrix room ID, WhatsApp chat JID, Discord channel ID). Not a service instance.

---

### room_name
**Definition:** The human-readable name of a chat room/channel, fetched from the service API when the bot connects to the room. Stored in `ChannelConfig.room_name` (replaces former `display_name` field). Used in `.status` output and log messages.

**Boundary:** Distinct from `channel` (the protocol-specific ID). May be empty if not yet populated or if API call fails. Updated on connect/reconnect.

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

---

### BridgeMatcher
**Definition:** Type that resolves a (service, channel) pair to the set of ChannelConfigs in the same bridge, identifying the source channel config and all target channel configs.

**Boundary:** Pure function of Config; no side effects. Used by message dispatcher, edit handler, reaction handler.

---

### EventRouter
**Definition:** Type that receives ServiceEvent from the mpsc channel and delegates to MessageDispatcher, EditHandler, ReactionHandler, or StatusHandler based on event type.

**Boundary:** Owns no state except handler references. Single event loop; graceful shutdown on channel close.

---

### MessageDispatcher
**Definition:** Type that forwards a ServiceMessage to all target channels in a bridge, applying formatting (MessageFormatter), media policy (MediaHandler), and aliases (AliasResolver) per target channel config.

**Boundary:** Does not handle edits, reactions, or status — only NewMessage events.

---

### ServiceBuilder
**Definition:** Type that constructs service instances from ServiceConfig, injecting dependencies (e.g., GifResolver for Discord). Returns concrete types or trait objects for the fine-grained traits.

**Boundary:** Pure construction; no connection or lifecycle management.

---

### ServiceRegistry
**Definition:** Type that manages the lifecycle (connect, start, wait_ready, shutdown) of a collection of service instances keyed by name. Stores services as trait objects for the fine-grained traits.

**Boundary:** Does not construct services; receives them from ServiceBuilder.

---

### MessageFormatter
**Definition:** Trait for protocol-specific message rendering (text + attachments). Matrix uses markdown+HTML, Discord uses markdown, WhatsApp uses WhatsApp markdown.

**Boundary:** Stateless; format(text, attachments, sender, config) → formatted message. Implemented per service.

---

### AliasResolver
**Definition:** Type that applies ChannelConfig.aliases (HashMap<String, String>) to a sender_id, returning the alias or the original sender_id.

**Boundary:** Pure function; no side effects. Handles missing/empty alias map gracefully.

---

### MediaHandler
**Definition:** Type that applies per-channel media policy: if ChannelConfig.enable_media=false, strips attachments from outgoing message.

**Boundary:** Pure transformation; no I/O.

---

### Deduplicator
**Definition:** Type that wraps MessageStore.exists to check if a (service, channel, message_id) has been processed, with optional mark-as-seen.

**Boundary:** Delegates to MessageStore; no independent state.

---

### EditHandler
**Definition:** Type that processes ServiceUpdate by looking up destination mappings via MessageStore, downcasting target services to MessageEditor, and calling edit_message.

**Boundary:** Only handles UpdateMessage events. Respects bridge_own_messages config.

---

### ReactionHandler
**Definition:** Type that processes ServiceReaction by looking up destination mappings via MessageStore, downcasting target services to ReactionSender, and calling react_to_message.

**Boundary:** Only handles NewReaction events. Respects bridge_own_messages config.

---

### StatusHandler
**Definition:** Type that handles .status command: queries is_connected() on all services in the bridge, calls get_room_members() via MemberLister on all channels, formats markdown response, sends to requesting channel.

**Boundary:** Only handles .status command messages. Uses ServiceInfo for display names.

---

### Connectable
**Definition:** Trait for service connection lifecycle: connect, disconnect, wait_until_ready, is_connected.

**Boundary:** Mandatory capability — all services must implement.

---

### MessageSender
**Definition:** Trait for sending messages: send_message(channel, message) → message_id.

**Boundary:** Mandatory capability — all services must implement.

---

### MessageEditor
**Definition:** Trait for editing messages: edit_message(channel, message_id, new_content). Optional — not all protocols support edits.

**Boundary:** Matrix and Discord implement; WhatsApp stubs with Err(unimplemented).

---

### ReactionSender
**Definition:** Trait for sending reactions: react_to_message(channel, message_id, emoji). Optional capability.

**Boundary:** All three services implement.

---

### MemberLister
**Definition:** Trait for listing channel members: get_room_members(channel) → Vec<String>. Optional capability.

**Boundary:** Matrix and Discord implement; WhatsApp stubs with Ok(vec![]).

---

### ServiceInfo
**Definition:** Trait for service metadata: service_name() → &str, as_any() for downcasting.

**Boundary:** Mandatory capability — all services must implement.