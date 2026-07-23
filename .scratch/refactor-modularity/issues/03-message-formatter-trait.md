Status: done

# 03-message-formatter-trait

## What to build
Define `MessageFormatter` trait in `bridge/formatter.rs` and extract formatting logic from each service's `send_message`:
- `format_text(sender, content, is_own) -> String`
- `format_caption(sender, content, is_own) -> String` (for attachments)
- Implementations: `MatrixFormatter` (markdown→HTML), `DiscordFormatter` (markdown), `WhatsAppFormatter` (markdown with *bold*)

This eliminates duplication and makes formatting testable in isolation.

## Files to create/modify
- src/bridge/formatter.rs (new - trait + common helpers)
- src/services/matrix/formatter.rs (new)
- src/services/discord/formatter.rs (new)
- src/services/whatsapp/formatter.rs (new)
- src/services/matrix.rs (modify - use MatrixFormatter)
- src/services/discord.rs (modify - use DiscordFormatter)
- src/services/whatsapp.rs (modify - use WhatsAppFormatter)

## Test approach
- Unit tests for each formatter: empty sender, empty content, both present, markdown special chars
- Snapshot tests for HTML output (Matrix)
- Property tests: formatter output doesn't contain sender when sender.is_empty()

## Acceptance criteria
- [ ] MessageFormatter trait defined with format_text and format_caption
- [ ] MatrixFormatter produces HTML body + plain fallback for text messages
- [ ] DiscordFormatter produces markdown with **sender**: content
- [ ] WhatsAppFormatter produces *sender*: content with *bold*
- [ ] All three services delegate to their formatter in send_message
- [ ] Unit tests cover all formatter implementations
- [ ] No behavior change in integration tests

## Blocked by
None — can start immediately