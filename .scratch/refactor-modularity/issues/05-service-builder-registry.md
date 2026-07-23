Status: done

# 05-service-builder-registry

## What to build
Extract service construction and lifecycle from `main.rs` into reusable types:
- `ServiceBuilder`: builds concrete service types from `ServiceConfig` + `GifResolver` (for Discord). Returns concrete types or trait objects for fine-grained traits.
- `ServiceRegistry`: owns a map of service names to trait objects for the fine-grained traits (Connectable, MessageSender, ReactionSender, ServiceInfo, and optionally MessageEditor, MemberLister). Provides connect/start/wait_ready/shutdown.

`main.rs` becomes: load config → create builder → builder.build_all(config) → registry.connect_all() → registry.start_all(tx) → registry.wait_all_ready() → coordinator.start()

## Files to create/modify
- src/services/builder.rs (new - ServiceBuilder)
- src/services/registry.rs (new - ServiceRegistry)
- src/main.rs (modify - thin entry point)

## Test approach
- Unit test ServiceBuilder with each config variant (Matrix, WhatsApp, Discord)
- Unit test ServiceRegistry connect/start/ready/shutdown sequencing
- Mock services for registry tests

## Acceptance criteria
- [ ] ServiceBuilder::build(config, gif_resolver) returns concrete service types or fine-grained trait objects
- [ ] ServiceRegistry::new() creates empty registry
- [ ] ServiceRegistry::register(name, service) stores service
- [ ] ServiceRegistry::connect_all() → calls connect on all, handles Discord skip-on-fail
- [ ] ServiceRegistry::start_all(tx) → calls start on all with cloned tx
- [ ] ServiceRegistry::wait_all_ready() → calls wait_until_ready on all
- [ ] ServiceRegistry::shutdown_all() → calls disconnect on all
- [ ] main.rs reduced to ~40 lines (logging, config, builder, registry, coordinator, signals)
- [ ] Matrix max_upload_size still propagated to GifResolver

## Blocked by
04-service-traits-split (builder returns fine-grained trait objects)