# Rosetta

A Rust-based bridge that connects a Matrix room with a WhatsApp chat (process or group). This bot listens for messages on both platforms and forwards them to the other, enabling seamless communication.

## Features

*   **Bidirectional Bridging**: Messages flow from Matrix to WhatsApp and vice versa.
*   **Media Support**: Currently focuses on text messages (extensible for media).
*   **Secure**: Uses `matrix-sdk` for end-to-end encryption support (if configured) and `whatsapp-rust` which implements the WhatsApp protocol.

## Dependencies

This project relies on the following Rust crates:

*   **`anyhow`**: Flexible error handling for easy result propagation.
*   **`dotenvy`**: Loads environment variables from a `config.yaml` file for configuration.
*   **`env_logger`**: Configures logging via environment variables.
*   **`log`**: Lightweight logging facade.
*   **`matrix-sdk`**: The official Matrix Client-Server SDK for Rust (v0.16.0), handling Matrix protocol interactions.
*   **`tokio`**: An asynchronous runtime for Rust, essential for handling concurrent tasks (connecting to both services simultaneously).
*   **`whatsapp-rust`** (and related crates): A Rust implementation of the WhatsApp protocol (sourced via git, branch `main`).
    *   `wacore`: Core types and functional logic.
    *   `wacore-binary`: Binary data handling.
    *   `waproto`: Protocol buffer definitions for WhatsApp.
    *   `whatsapp-rust-tokio-transport`: WebSocket transport layer using Tokio.
    *   `whatsapp-rust-ureq-http-client`: HTTP client modules using ureq.


## Setup & Running

### Prerequisites

*   Rust and Cargo installed (Nightly toolchain required).
*   A Matrix account and a room to bridge.
*   A WhatsApp account (linked via QR code).

### 1. Clone & Configure

1.  Clone this repository.
2.  Create a `config.yaml` file in the root directory (you can copy `config_example.yaml`):

    ```bash
    cp config_example.yaml config.yaml
    ```

3.  Edit `config.yaml` with your credentials:

    ```yaml
    matrix:
      homeserver_url: "https://matrix.org"
      username: "your_user"
      password: "your_password"
      room_id: "!your_room_id:matrix.org"
      debug: true
      aliases:
        "@user:matrix.org": "Me"

    whatsapp:
      target_id: "1234567890@s.whatsapp.net"
      debug: true
      bridge_own_messages: true
      aliases:
        "1234567890@s.whatsapp.net": "Alice"
        "00000000@lid": "Bob"
    ```

    *   **Aliases**: Map IDs (JIDs or UserIDs) to display names.
        *   Keys must be quoted if they contain special characters (like `@`).
        *   Values are the names that will appear in the bridged message.
    *   **Debugging**: Set `debug: true` for detailed logs.
    
### How to find the WhatsApp JID

If you don't know the `WHATSAPP_ID_TO_BRIDGE`, follow these steps:

1.  Set a dummy value in `config.yaml` (e.g., `WHATSAPP_ID_TO_BRIDGE=dummy`).
2.  Run the bot (`cargo run`) and link your WhatsApp account via QR code.
3.  Send a message to the person or group you want to bridge from your phone.
4.  Check the bot's terminal output. You will see a log line like:
    `INFO: Received WhatsApp message from JID: 1234567890@s.whatsapp.net`
5.  Copy this JID and update your `config.yaml` file.
6.  Restart the bot.

### 2. Run the Bot

Execute the project using Cargo:

```bash
cargo run
```

### 3. Authenticate WhatsApp

On the first run, the bot will display a QR code in the terminal.
1.  Open WhatsApp on your mobile device.
2.  Go to **Linked Devices** > **Link a Device**.
3.  Scan the QR code.

Once connected, the bot will start forwarding messages between the configured Matrix room and WhatsApp chat.
