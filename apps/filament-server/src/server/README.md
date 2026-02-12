# filament-server module map

This directory contains the backend split from the former monolithic `src/lib.rs`.

- `core.rs`: global constants, config/state structs, in-memory records, and app initialization.
- `db.rs`: schema bootstrap and database enum/bitmask conversion helpers.
- `metrics.rs`: in-process metrics state and Prometheus text rendering.
- `router.rs`: router assembly and transport-layer middleware wiring.
- `types.rs`: API DTOs, path/query structs, and lightweight transport enums.
- `handlers.rs`: HTTP endpoint handlers (auth, friends, guilds, channels, messages, media).
- `realtime.rs`: websocket gateway handling, fanout, and search worker orchestration.
- `domain.rs`: permission checks, attachment/reaction helpers, and audit-log persistence.
- `auth.rs`: credential/token helpers, auth context extraction, and per-surface rate guards.
- `errors.rs`: unified API error mapping and tracing initialization.
- `tests.rs`: internal unit/integration-style tests that require crate-private access.

`src/lib.rs` now serves as the composition entrypoint and includes these files in order.
