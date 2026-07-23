# Rosetta Agent Guide

This document provides context, architecture notes, commands, and coding guidelines for AI coding agents collaborating on the **Rosetta** project.

Rosetta is a modular, multi-protocol chat bridge written in Rust. It connects chat channels/rooms across Matrix, WhatsApp, and Discord, allowing seamless multi-service messaging.

---

## ⚡ API Usage & Tool Execution Rules

To minimize API rate limits and token usage during automated agent loops, adhere to the following operational guardrails:

* **Batching Tool Calls:** Group tool actions together in a single turn whenever possible. If you need to inspect multiple files or make sequential edits, execute them in one turn rather than reading/editing files one-by-one across multiple back-and-forth turns.
* **Concise Planning First:** Output a short, high-level execution plan before triggering heavy tool chains.
* **Context Efficiency:** Keep reasoning steps, summaries, and thought outputs brief. Avoid re-reading large files or repeating directory listings unnecessarily.
* **Scope Isolation:** Stay focused on the exact files needed for the task to avoid ballooning the prompt context size.

---

## 📁 Project Knowledge & Resources

* **Agent Skills (`.agents/skills/`):** Contains specialized agent skills and reusable tool/workflow instructions. Check this folder when performing specific automated tasks.
* **Feature Roadmap (`.scratch/`):** Contains feature specifications, architecture drafts, and planned roadmap items. Consult this directory before implementing or proposing major structural updates.

---

## 🛠️ Technology Stack

| Technology | Purpose / Details |
| :--- | :--- |
| **Language** | Rust (stable toolchain) |
| **Async Runtime** | `tokio` (v1.53.0, full features) |
| **Matrix Protocol** | `matrix-sdk` (v0.18.0) |
| **Discord Protocol** | `serenity` (v0.12.4) |
| **WhatsApp Protocol** | `wa-rs` |
| **Database** | SQLite via `rusqlite` (v0.37) |
| **Configuration** | YAML via `serde` (v1.0.228) and `serde_yaml` (v0.9.34) |
| **Logging** | `log` (v0.4) and `env_logger` (v0.11) |

---

## 📂 Core Directory Structure & Key Symbols

* **[Cargo.toml](file:///home/bronson/Projects/rosetta/Cargo.toml)**: Defines the crate dependencies, features, and manifest settings.
* **[src/main.rs](file:///home/bronson/Projects/rosetta/src/main.rs)**: Entry point. Sets up the logger, loads configuration, initializes the configured services, runs connection routines, and spins up the coordinator.
* **[src/config.rs](file:///home/bronson/Projects/rosetta/src/config.rs)**: Contains deserialization structs for parsing `data/config.yaml`.
  * `Config`: Overall app configuration.
  * `ServiceConfig`: Tagged union/enum for protocol configs (Matrix, WhatsApp, Discord).
  * `ChannelConfig`: Configurations for channels in bridges (includes alias mapping, own-message bridging flags, and media-bridging toggle).
* **[src/bridge.rs](file:///home/bronson/Projects/rosetta/src/bridge.rs)**:
  * `BridgeCoordinator`: Runs the main event routing loop, coordinates message forwarding, message edits, reaction sync, and answers the `.status` command.
* **[src/bridge/dispatcher.rs](file:///home/bronson/Projects/rosetta/src/bridge/dispatcher.rs)**:
  * `MessageDispatcher`: Routes messages between services, applies alias resolution, media policy, and formatters.
* **[src/bridge/alias_resolver.rs](file:///home/bronson/Projects/rosetta/src/bridge/alias_resolver.rs)**:
  * `AliasResolver`: Resolves sender IDs to display names using configured aliases; falls back to the source service's provided display name.
* **[src/persistence.rs](file:///home/bronson/Projects/rosetta/src/persistence.rs)**:
  * `MessageStore`: Core helper interacting with `data/message_history.db`. Maps incoming source message IDs to the generated destination message IDs on other services.
* **[src/services/mod.rs](file:///home/bronson/Projects/rosetta/src/services/mod.rs)**:
  * `Service` Trait: Represents a protocol integration backend (methods like `connect`, `start`, `send_message`, `edit_message`, `react_to_message`, `get_room_members`).
  * `ServiceMessage`, `ServiceUpdate`, `ServiceReaction`, `ServiceEvent`, and `Attachment` structs.
* **[src/services/matrix.rs](file:///home/bronson/Projects/rosetta/src/services/matrix.rs)**: `MatrixService` implementation.
* **[src/services/whatsapp.rs](file:///home/bronson/Projects/rosetta/src/services/whatsapp.rs)**: `WhatsAppService` implementation.
* **[src/services/discord.rs](file:///home/bronson/Projects/rosetta/src/services/discord.rs)**: `DiscordService` implementation. Includes parsing/link-scraping for media links (`og:image` support).

---

## 💾 SQLite Persistence Schema

To support features like edit forwarding, message reactions, and duplicate message prevention, Rosetta maintains a table in `data/message_history.db`:

```sql
CREATE TABLE IF NOT EXISTS message_map (
    source_service TEXT NOT NULL,
    source_channel TEXT NOT NULL,
    source_id TEXT NOT NULL,
    dest_service TEXT NOT NULL,
    dest_channel TEXT NOT NULL,
    dest_id TEXT NOT NULL,
    timestamp INTEGER DEFAULT (strftime('%s', 'now')),
    PRIMARY KEY (source_service, source_channel, source_id, dest_service)
);

CREATE INDEX IF NOT EXISTS idx_source 
ON message_map(source_service, source_channel, source_id);
```

---

## ⚙️ Development Workflows

### Setup Configuration
Before running, copy the example configuration:
```bash
mkdir -p data
cp config_example.yaml data/config.yaml
```

### Running Locally
```bash
cargo run
```

### Formatting and Linting
Please format and lint all changes before committing:
```bash
cargo fmt
cargo clippy --all-targets --all-features
```

### Running Integration Tests
Integration tests require a valid `data/config.yaml` with service credentials and GIF provider API keys. Tests skip gracefully with a warning if credentials are missing.

```bash
# Run all integration tests sequentially (required to avoid service connection conflicts)
cargo test --test integration_tests -- --test-threads=1
```

### Podman / Docker Deployment
```bash
# Build the container
podman build -t gibbz/rosetta:latest .

# Run the container (bind-mount the config/db directory)
podman run -d --name rosetta -v ./data:/app/data:Z gibbz/rosetta:latest
```

---

## ⚠️ Coding Guardrails & Rules

> [!IMPORTANT]
> **Toolchain Requirement**: Use the Rust **stable** toolchain. Keep configurations aligned with stable compiler features.

> [!WARNING]
> **Async Integrity**: The application runs entirely on `tokio`. Do not use blocking database calls or blocking network requests inside async handlers. Use `tokio::task::spawn_blocking` if necessary, or rely on async APIs (e.g., async features in `whatsapp-rust`).

> [!NOTE]
> **Deduplication / Loop Prevention**:
> - Always query `MessageStore::exists` prior to routing.
> - Ensure bot client instances ignore messages sent by themselves unless `bridge_own_messages` is explicitly set to true.
> - Filter out events from other bots where possible (e.g. `msg.author.bot` check in Discord).

* **Error Handling**: Utilize `anyhow` for app-level error propagates.
* **Documentation**: Maintain existing docstrings and comments. If modifying traits, ensure the corresponding implementations are fully documented.
* **Logging**: Use `log` crate macros (`info!`, `warn!`, `error!`, `debug!`). Keep log lines clean and filter out noisy dependency outputs as initialized in `[src/main.rs](file:///home/bronson/Projects/rosetta/src/main.rs)`.
