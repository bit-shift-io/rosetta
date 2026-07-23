Status: done

# 10-status-handler

## What to build
Extract `.status` command handling from `BridgeCoordinator::route_message` into `StatusHandler`:
- Input: triggering `ServiceMessage`, `HashMap<String, Arc<Mutex<Box<dyn Service>>>>`, `Config`
- For each service in the bridge: checks is_connected()
- For each channel in the bridge: calls get_room_members() via MemberLister
- Formats response as markdown, sends back to source channel via MessageSender
- Returns result

## Files to create/modify
- src/bridge/status_handler.rs (new - StatusHandler)
- src/bridge/mod.rs (re-export)
- src/bridge.rs (modify - use StatusHandler)

## Test approach
- Unit test with mock services: verify status includes all services/channels
- Test disconnected service shows ❌
- Test member listing per channel
- Test response sent only to requesting channel

## Acceptance criteria
- [ ] StatusHandler.handle(msg, services, config) builds and sends status
- [ ] Lists all services in the bridge with connection status
- [ ] Lists all channels with member lists (via MemberLister)
- [ ] Only responds in the channel that sent .status
- [ ] Uses ServiceInfo.service_name for display
- [ ] BridgeCoordinator::route_message delegates .status to StatusHandler
- [ ] .status command still works in integration tests

## Blocked by
04-service-traits-split (MemberLister, ServiceInfo, MessageSender traits)