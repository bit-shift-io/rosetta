Status: done

# 04-service-traits-split

## What to build
Split the monolithic `Service` trait into fine-grained traits (no facade):
- `Connectable`: connect, disconnect, wait_until_ready, is_connected
- `MessageSender`: send_message
- `MessageEditor`: edit_message (optional)
- `ReactionSender`: react_to_message (optional)
- `MemberLister`: get_room_members (optional)
- `ServiceInfo`: service_name, as_any, as_any_mut

Update all three service implementations to implement the fine-grained traits. Bridge code uses trait objects directly for mandatory traits (`MessageSender`, `Connectable`, `ReactionSender`, `ServiceInfo`), downcasts only for optional traits (`MessageEditor`, `MemberLister`).

## Files to create/modify
- src/services/traits.rs (new - fine-grained traits only, no facade)
- src/services/mod.rs (modify - re-export new traits)
- src/services/matrix.rs (modify - impl fine-grained traits)
- src/services/discord.rs (modify - impl fine-grained traits)
- src/services/whatsapp.rs (modify - impl fine-grained traits; edit/members as stubs)

## Test approach
- Compile test: WhatsApp doesn't implement MessageEditor/MemberLister (stub Err(unimplemented) / Ok(vec![]))
- Integration tests verify all features still work

## Acceptance criteria
- [ ] Six fine-grained traits defined with correct method signatures
- [ ] No facade trait
- [ ] MatrixService implements all six traits
- [ ] DiscordService implements all six traits
- [ ] WhatsAppService implements Connectable, MessageSender, ReactionSender, ServiceInfo; MessageEditor/MemberLister return Err(unimplemented) / Ok(vec![])
- [ ] BridgeCoordinator uses trait objects directly for mandatory traits, downcasts only for optional traits
- [ ] All integration tests pass

## Blocked by
03-message-formatter-trait (services modified in both)