# EVENT_REFACTOR.md

## Objective
Refactor the realtime event system to reduce accidental complexity while preserving or improving security boundaries, protocol compatibility, and operational robustness.

## Scope
- `apps/filament-server` gateway ingress, event construction, fanout, and observability.
- `apps/filament-client-web` gateway envelope parsing, domain dispatch, and payload decoding.
- `crates/filament-protocol` event contract ownership and envelope validation.
- Event contract docs and contract-consistency tests.

## Non-Goals
- No federation work.
- No E2EE or cryptography changes.
- No relaxation of payload limits, queue limits, timeouts, or rate limits.
- No introduction of unsafe Rust.

## Security and Architecture Constraints
- Keep strict envelope `{ v, t, d }` and explicit event versioning.
- Fail closed at every network boundary.
- Enforce hard caps for ingress and outbound payloads.
- Prefer typed domain conversion over stringly routing.
- Preserve bounded queues and slow-consumer eviction behavior.

## Code-State Snapshot (2026-02-25)
- Ingress validation is strict and mostly correct.
- Fanout backpressure behavior is strong.
- Contract drift exists for `workspace_channel_override_update` (two payload shapes under one event name).
- Guild fanout currently scans all subscription keys by prefix.
- Event construction has fallback behavior that can silently mask emit defects.
- Server/client gateway logic is heavily split into small modules, increasing maintenance overhead.

## Success Metrics
- Zero known contract drift between emitted events, client decoders, and docs.
- Outbound payload cap enforced for all event paths.
- Guild fanout no longer requires prefix-scanning all subscriptions.
- Realtime module count and call-chain depth reduced without reducing test coverage.
- Existing gateway network flow and security-limits tests remain green.

## Status Legend
- `NOT STARTED`
- `IN PROGRESS`
- `DONE`
- `BLOCKED`

---

## Phase 0 - Contract Freeze and Drift Elimination
### Goal
Make event contracts unambiguous and eliminate current payload-shape drift.

### Completion Status
`DONE`

### Tasks
- [x] Split `workspace_channel_override_update` into explicit contracts:
  - role override event
  - principal/target override event
- [x] Keep backward compatibility strategy explicit:
  - temporary dual-emit window or controlled migration flag
  - client accepts both during migration window
- [x] Update `docs/GATEWAY_EVENTS.md` and protocol contract notes.
- [x] Add cross-contract tests proving server emit set, docs set, and client decode set stay aligned.

### Tentative File Touch List
- `apps/filament-server/src/server/gateway_events/workspace.rs`
- `apps/filament-server/src/server/gateway_events.rs`
- `apps/filament-server/src/server/handlers/guilds.rs`
- `apps/filament-server/src/server/tests/tests/contract.rs`
- `apps/filament-server/tests/gateway_network_flow.rs`
- `apps/filament-client-web/src/lib/gateway-workspace-*.ts`
- `apps/filament-client-web/tests/gateway-workspace-*.test.ts`
- `docs/GATEWAY_EVENTS.md`
- `docs/PROTOCOL.md`

### Tests
- [x] Server integration test for both override event variants.
- [x] Web decoder tests for each variant and fail-closed invalid payloads.
- [x] Contract manifest parity tests (server/doc/client).

### Progress Notes
- 2026-02-25 (Slice 1): Added explicit server event type `workspace_channel_permission_override_update` for principal/target override payload shape, kept temporary dual-emit from the permission-override route (`workspace_channel_override_update` legacy + explicit event), and updated server/docs event manifests plus gateway event builder tests.
- 2026-02-25 (Slice 2): Updated web workspace gateway decoding/dispatch to accept migration dual-emit payloads under both event names, normalize principal/target payloads to `workspace_channel_permission_override_update`, and add targeted dispatch tests for legacy role payload, legacy permission payload, and explicit permission event type.
- 2026-02-25 (Slice 3): Added gateway network integration coverage for the permission-override REST route to assert dual-emit behavior (`workspace_channel_override_update` legacy + `workspace_channel_permission_override_update` explicit) and payload shape consistency for target-based overrides.
- 2026-02-25 (Slice 4): Added explicit server event type `workspace_channel_role_override_update` for role override payload shape, kept temporary dual-emit from the role-override route (`workspace_channel_override_update` legacy + explicit role event), and extended builder/network tests for role override migration behavior.
- 2026-02-25 (Slice 5): Updated gateway/protocol docs to explicitly codify the override migration split (`workspace_channel_override_update` legacy role shape, plus explicit role and permission override event names) and added a server contract test to fail if docs drift on this migration contract.
- 2026-02-25 (Slice 6): Added a focused cross-contract parity test for the override migration event trio to enforce alignment across server emitted manifest, `docs/GATEWAY_EVENTS.md`, and client decoder event-type acceptance; also fixed drift by including `workspace_channel_role_override_update` in server emitted manifest and client override type guard.
- 2026-02-25 (Slice 7): Added focused web decoder tests for all override migration event variants (`workspace_channel_override_update`, `workspace_channel_role_override_update`, `workspace_channel_permission_override_update`) and explicit fail-closed cases for malformed role and permission payloads.
- 2026-02-25 (Slice 8): Added a server contract test enforcing cross-contract parity for all emitted domain event names across `EMITTED_EVENT_TYPES`, `docs/GATEWAY_EVENTS.md`, and web gateway source literals (`apps/filament-client-web/src/lib/gateway-*.ts`), intentionally excluding connection-only `ready`/`subscribed` pending a dedicated client subscribe-ack contract slice.
- 2026-02-25 (Slice 9): Added explicit client decode support for `subscribed` connection events (fail-closed payload validation), then tightened server manifest parity assertions to include connection events so emitted server/docs/client event sets are now enforced without exclusions.

### Exit Criteria
- One logical event name maps to one payload shape.
- Docs and tests reflect actual emitted behavior.

---

## Phase 1 - Outbound Event Hardening
### Goal
Make event emission fail-fast and enforce outbound size limits universally.

### Completion Status
`IN PROGRESS`

### Tasks
- [ ] Replace fallback event serialization paths with explicit `Result` handling.
- [ ] Add outbound payload size guard before enqueue/fanout.
- [ ] Add metric labels for outbound rejection reasons (`oversized_outbound`, `serialize_error`).
- [ ] Ensure dropped/rejected emits are observable but never panic the server.

### Tentative File Touch List
- `apps/filament-server/src/server/auth.rs` (remove fallback-to-`ready` behavior for outbound event building)
- `apps/filament-server/src/server/gateway_events/envelope.rs`
- `apps/filament-server/src/server/realtime/fanout_dispatch.rs`
- `apps/filament-server/src/server/realtime/presence_sync_dispatch.rs`
- `apps/filament-server/src/server/realtime/voice_sync_dispatch.rs`
- `apps/filament-server/src/server/realtime/subscribe_ack.rs`
- `apps/filament-server/src/server/metrics.rs`

### Tests
- [ ] Unit tests for outbound size rejection.
- [ ] Integration test that oversized outbound payload is dropped and counted.
- [ ] Regression test proving normal payloads still fan out.

### Progress Notes
- 2026-02-25 (Slice 1): Removed silent fallback-to-`ready` behavior from `outbound_event` by switching to explicit `Result` errors for invalid event types and serialization/encoding failures; added unit tests proving valid envelope output and fail-closed rejection for invalid outbound event names. Gateway event envelope builder now fails loudly on build errors instead of silently emitting fallback payloads.
- 2026-02-25 (Slice 2): Removed panic-based connection-event wrappers (`ready`/`subscribed`) from the server gateway API and switched websocket connection + subscribe-ack paths to explicit `try_ready`/`try_subscribed` `Result` handling. Serialization failures are now observed via `filament_gateway_events_dropped_total{scope=\"connection\",reason=\"serialize_error\"}` and fail-closed disconnect/error paths, with targeted tests updated to use explicit builders.
- 2026-02-25 (Slice 3): Added non-panicking `try_build_event` envelope builder coverage (including explicit serialization-failure tests), then migrated only the realtime message-create emit path to explicit `Result` handling via `try_message_create`. `emit_message_create_and_index` now drops failed outbound serialization with `filament_gateway_events_dropped_total{scope=\"channel\",event_type=\"message_create\",reason=\"serialize_error\"}` instead of panicking, while other message/channel emitters remain unchanged for follow-up slices.
- 2026-02-25 (Slice 4): Migrated profile outbound gateway builders from panic-based wrappers to explicit `try_profile_update`/`try_profile_avatar_update` `Result` APIs. Profile broadcast handlers now fail closed on serialization errors, recording `filament_gateway_events_dropped_total{scope=\"user\",event_type in {\"profile_update\",\"profile_avatar_update\"},reason=\"serialize_error\"}` and skipping fanout instead of panicking.
- 2026-02-25 (Slice 5): Migrated message edit outbound emission from panic-capable builder usage to explicit `Result` handling via `try_message_update`. The message-update broadcast path now records `filament_gateway_events_dropped_total{scope=\"channel\",event_type=\"message_update\",reason=\"serialize_error\"}` and logs a warning before skipping fanout when serialization fails; gateway event tests were updated to use explicit `try_message_update`.
- 2026-02-25 (Slice 6): Migrated presence-subscribe event construction (snapshot + online update) to explicit `Result` builders via `try_presence_sync`/`try_presence_update` in the runtime path. `handle_presence_subscribe` now fail-closes serialization errors by logging and recording `filament_gateway_events_dropped_total{reason=\"serialize_error\"}` with scope-aware labels (`connection` for `presence_sync`, `guild` for `presence_update`) instead of relying on panic-capable builders.
- 2026-02-25 (Slice 7): Migrated disconnect-time offline presence event construction to explicit `Result` handling by switching `presence_disconnect_events` and disconnect followup planning to fallible builders using `try_presence_update`. `remove_connection` now logs and records `filament_gateway_events_dropped_total{scope=\"guild\",event_type=\"presence_update\",reason=\"serialize_error\"}` when followup event construction fails, and the remaining panic wrapper (`presence_update`) was removed in favor of `try_presence_update` across tests/runtime call sites.
- 2026-02-25 (Slice 8): Migrated message-delete outbound emission to explicit `Result` handling with new `try_message_delete` builder, removing the panic-capable `message_delete` wrapper. The message-delete broadcast path now fail-closes serialization failures by recording `filament_gateway_events_dropped_total{scope=\"channel\",event_type=\"message_delete\",reason=\"serialize_error\"}` and logging a warning before skipping fanout; added targeted gateway event builder coverage for `try_message_delete`.
- 2026-02-25 (Slice 9): Migrated voice subscribe snapshot emission to explicit `Result` handling by adding `try_voice_participant_sync` and switching `handle_voice_subscribe` to use a fallible `try_build_voice_subscribe_sync_event` path. Voice sync serialization failures now fail closed with `gateway.voice_subscribe.serialize_failed` warning logs and `filament_gateway_events_dropped_total{scope=\"connection\",event_type=\"voice_participant_sync\",reason=\"serialize_error\"}` instead of relying on a panic-capable runtime builder path.
- 2026-02-25 (Slice 10): Added fallible `try_voice_participant_update` gateway event builder and migrated only `update_voice_participant_audio_state_for_channel` to explicit `Result` handling. Audio-state voice update serialization failures now fail closed with `gateway.voice_participant_update.serialize_failed` warning logs and `filament_gateway_events_dropped_total{scope=\"channel\",event_type=\"voice_participant_update\",reason=\"serialize_error\"}` before skipping fanout; added focused builder payload coverage for the new `try_` event path.
- 2026-02-25 (Slice 11): Migrated voice registration transition planning to use fallible `try_voice_participant_update` instead of panic-capable `voice_participant_update` wrappers. `register_voice_participant_from_token` now handles planning serialization errors explicitly with warning log `gateway.voice_registration.serialize_failed`, records `filament_gateway_events_dropped_total{scope=\"channel\",event_type=\"voice_participant_update\",reason=\"serialize_error\"}`, and fails closed by skipping broadcast for the faulty registration transition.

### Exit Criteria
- Outbound and inbound both enforce size caps.
- No silent fallback to unrelated event types.

---

## Phase 2 - Fanout Data Model Refactor
### Goal
Remove O(N) guild prefix scans and use explicit indices for routing.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Introduce indexed subscription registry:
  - channel key -> listeners
  - guild id -> connection ids
  - user id -> connection ids (verify this remains authoritative)
- [ ] Update subscription insert/remove paths to maintain all indexes atomically.
- [ ] Refactor guild broadcast path to direct index lookup (no string prefix scans).
- [ ] Keep dedup semantics and slow-consumer handling unchanged.

### Tentative File Touch List
- `apps/filament-server/src/server/core.rs` (registry types)
- `apps/filament-server/src/server/realtime/subscription_insert.rs`
- `apps/filament-server/src/server/realtime/connection_subscriptions.rs`
- `apps/filament-server/src/server/realtime/connection_registry.rs`
- `apps/filament-server/src/server/realtime/fanout_guild.rs`
- `apps/filament-server/src/server/realtime/connection_runtime.rs`
- `apps/filament-server/src/server/tests/tests/gateway.rs`

### Tests
- [ ] Unit tests for index maintenance on subscribe/disconnect.
- [ ] Existing dedup test still passes.
- [ ] Add stress-oriented test for large subscription maps to validate non-scan path.

### Exit Criteria
- Guild broadcast complexity is index-based.
- Behavior parity for delivery, dedup, and slow-consumer close.

---

## Phase 3 - Ingress Domain Typing and Boundary Cleanup
### Goal
Reduce stringly-typed command handling at ingress boundary.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Introduce gateway ingress domain types (validated IDs, bounded fields) from DTO conversion.
- [ ] Keep DTO structs at transport boundary with `deny_unknown_fields`.
- [ ] Move all ID/shape validation into `TryFrom` conversions before handler execution.
- [ ] Ensure ingress parse and unknown-event metrics stay intact.

### Tentative File Touch List
- `apps/filament-server/src/server/types.rs`
- `apps/filament-server/src/server/realtime/ingress_command.rs`
- `apps/filament-server/src/server/realtime/ingress_parse.rs`
- `apps/filament-server/src/server/realtime/ingress_subscribe.rs`
- `apps/filament-server/src/server/realtime/ingress_message_create.rs`
- `apps/filament-server/src/server/domain.rs` (if newtypes/helpers live here)

### Tests
- [ ] Unit tests for newtype invariants and `TryFrom` conversions.
- [ ] Ingress parse tests for invalid IDs and malformed payloads.
- [ ] Gateway network tests still pass for subscribe/message_create.

### Exit Criteria
- Handlers execute only with validated domain input.
- No behavior regressions in disconnect semantics.

---

## Phase 4 - Module Consolidation Without Behavior Changes
### Goal
Reduce fragmentation and simplify navigation while preserving testable seams.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Consolidate tiny wrapper modules into cohesive components:
  - `realtime/ingress/*`
  - `realtime/fanout/*`
  - `realtime/presence/*`
  - `realtime/voice/*`
- [ ] Keep pure helper functions and tests, but reduce one-function files.
- [ ] Preserve public/internal function signatures where practical to minimize churn.

### Tentative File Touch List
- `apps/filament-server/src/server/realtime.rs`
- `apps/filament-server/src/server/realtime/*` (module moves/merges)
- `apps/filament-server/src/server/README.md`

### Tests
- [ ] Full server test suite plus gateway network flow.
- [ ] Clippy and rustdoc clean for moved modules.

### Exit Criteria
- Lower module count and shallower call graph.
- No functional diffs beyond import/move cleanup.

---

## Phase 5 - Client Dispatcher Simplification
### Goal
Keep fail-closed parsing while reducing repetitive dispatch boilerplate.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Replace long `if` chains in dispatchers with table-driven handler maps.
- [ ] Keep domain decoders explicit and strict.
- [ ] Centralize event-type registry used by dispatcher + tests.
- [ ] Preserve unknown-event ignore behavior.

### Tentative File Touch List
- `apps/filament-client-web/src/lib/gateway.ts`
- `apps/filament-client-web/src/lib/gateway-domain-dispatch.ts`
- `apps/filament-client-web/src/lib/gateway-*-dispatch.ts`
- `apps/filament-client-web/src/lib/gateway-*-events.ts`
- `apps/filament-client-web/tests/gateway-*.test.ts`

### Tests
- [ ] Existing gateway parser/dispatch tests remain green.
- [ ] Add tests for registry completeness and duplicate-type detection.

### Exit Criteria
- Client dispatch is data-driven and easier to extend.
- Strict payload parsing behavior unchanged.

---

## Phase 6 - Contract Source-of-Truth Enforcement
### Goal
Prevent future drift between server emitters, docs, and web decoders.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Introduce machine-readable event manifest (event type + schema version + scope).
- [ ] Add CI check that compares:
  - emitted server event list
  - documented event list
  - client-supported event list
- [ ] Require explicit migration entry for additive/deprecated events.

### Tentative File Touch List
- `crates/filament-protocol/src/lib.rs` (or new `events` module)
- `apps/filament-server/src/server/tests/tests/contract.rs`
- `apps/filament-client-web/tests/gateway-contract-manifest.test.ts` (new)
- `docs/GATEWAY_EVENTS.md`
- `.github/workflows/ci.yml`

### Tests
- [ ] Contract parity tests fail if any event appears in only one surface.
- [ ] Backward-compat tests for additive field changes.

### Exit Criteria
- Contract drift becomes CI-blocked by default.

---

## Phase 7 - Rollout, Telemetry, and Cleanup
### Goal
Ship safely with progressive rollout and remove migration compatibility code.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add temporary compatibility counters for dual-decoder/dual-emitter paths.
- [ ] Define sunset criteria for deprecated payloads/event names.
- [ ] Remove compatibility code after stability window and update docs.
- [ ] Capture post-refactor benchmark snapshots for fanout hot paths.

### Tests
- [ ] Regression pass of gateway network flow, security limits, and websocket lifecycle.
- [ ] Verify telemetry counters for dropped/rejected events in staging.

### Exit Criteria
- Deprecated event compatibility paths removed.
- Observability confirms stable behavior.

---

## Recommended PR Sequence (Small Safe Increments)
1. Phase 0 contract split for override events + docs + tests.
2. Phase 1 outbound hardening and metrics.
3. Phase 2 guild fanout index.
4. Phase 3 ingress domain typing.
5. Phase 4 server module consolidation.
6. Phase 5 client dispatcher simplification.
7. Phase 6 manifest/CI parity gates.
8. Phase 7 cleanup/removal of transition code.

## Required Validation Per Phase
- `cargo fmt`
- `cargo clippy`
- `cargo test`
- `cargo audit` (where available in environment)
- `cargo deny` (where available in environment)
- `npm test` for `apps/filament-client-web` gateway-related suites
