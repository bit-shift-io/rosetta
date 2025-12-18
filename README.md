# Rosetta

Rosetta is a modular, multi-protocol chat bridge written in Rust. It connects different chat services (Matrix, WhatsApp, Discord) together, allowing seamless communication across platforms.

## Features

*   **Multi-Service Support**: Connects Matrix, WhatsApp, and Discord.
*   **Flexible Bridging**: Define multiple bridges to link specific channels/rooms across services.
*   **Aliases**: Map user IDs to readable display names per channel.
*   **Secure**: Uses `matrix-sdk` for E2EE support and runs locally.
*   **Loop Prevention**: Intelligent bridging logic prevents infinite message loops between bots.

## Supported Services

*   **Matrix**: Full generic client support (via `matrix-sdk`).
*   **WhatsApp**: Web client emulation (via `whatsapp-rust`).
*   **Discord**: Bot integration (via `serenity`).

## Prerequisites

*   **Rust**: Nightly toolchain is required (due to `whatsapp-rust` dependencies).
*   **Matrix Account**: A dedicated bot account is recommended.
*   **WhatsApp Account**: You will need to scan a QR code to link the session.
*   **Discord Bot**: You need a Bot Token and **Privileged Gateway Intents** (Message Content, Server Members, Presence) enabled in the Discord Developer Portal.

## Configuration

Rosetta uses a `config.yaml` file for all settings.

1.  **Copy the example**:
    ```bash
    cp config_example.yaml config.yaml
    ```
2.  **Edit `config.yaml`**:

    The configuration has two main sections:
    *   **`services`**: Define your bot accounts (Matrix login, Discord token, etc.).
    *   **`bridges`**: Define which channels talk to each other.

    **Example Snippet:**
    ```yaml
    services:
      my_matrix_bot:
        protocol: matrix
        homeserver_url: "https://matrix.org"
        username: "startrek_bridge_bot"
        password: "secure_password"
        bridge_own_messages: false # Always false for bots

      my_discord_bot:
        protocol: discord
        bot_token: "YOUR_DISCORD_TOKEN"
        bridge_own_messages: false

    bridges:
      general_chat:
        - service: my_matrix_bot
          channel: "!roomid:matrix.org"
          display_names: true
        - service: my_discord_bot
          channel: "1234567890" # Channel ID
          display_names: true
    ```

    > **Tip**: See `config_example.yaml` for full options including Aliases.

## Commands

Rosetta supports built-in commands that can be sent in any bridged channel:

*   **`.status`**
    *   **Usage**: Type `.status` in a chat.
    *   **Effect**: The bot checks the connection health of all services **in that specific bridge** and posts a report (e.g., "Bridge Status: Connected").
    *   **Note**: The command itself is also forwarded to other bridged channels so everyone knows a status check was performed.

## Running

1.  **Build and Run**:
    ```bash
    cargo run
    ```
2.  **First Run**:
    *   If using WhatsApp, a QR code will appear in the terminal. Scan it with your phone (Linked Devices).

## Development

*   **Dependencies**: managed in `Cargo.toml`.
*   **Architecture**:
    *   `src/main.rs`: Entry point and service initialization.
    *   `src/bridge.rs`: Core bridging logic and message routing.
    *   `src/services/`: Protocol-specific implementations (Service trait).
    
## Docker / Podman
Login: `podman login docker.io`
Build: `podman build -t docker.io/gibbz/rosetta:latest .`
Push: `podman push docker.io/gibbz/rosetta:latest`
