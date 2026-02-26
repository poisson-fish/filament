# EVENT_REFACTOR.md

## Objective
Refactor the realtime event system to reduce accidental complexity while preserving or improving security boundaries and operational robustness.

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

## Pre-Deploy Compatibility Policy (2026-02-25)
- Protocol-breaking refactor changes are allowed because the app is not officially deployed.
- Backward-compatibility shims (dual emit/dual decode/migration flags) are optional, not required.
- When a protocol/event contract is changed, server emitters, client decoders, docs, and contract tests must be updated in the same slice/PR.
- Security controls remain non-negotiable: no relaxation of limits, timeouts, rate caps, fail-closed behavior, or input validation.

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
`DONE`

### Tasks
- [x] Replace fallback event serialization paths with explicit `Result` handling.
- [x] Add outbound payload size guard before enqueue/fanout.
- [x] Add metric labels for outbound rejection reasons (`oversized_outbound`, `serialize_error`).
- [x] Ensure dropped/rejected emits are observable but never panic the server.

### Tentative File Touch List
- `apps/filament-server/src/server/auth.rs` (remove fallback-to-`ready` behavior for outbound event building)
- `apps/filament-server/src/server/gateway_events/envelope.rs`
- `apps/filament-server/src/server/realtime/fanout_dispatch.rs`
- `apps/filament-server/src/server/realtime/presence_sync_dispatch.rs`
- `apps/filament-server/src/server/realtime/voice_sync_dispatch.rs`
- `apps/filament-server/src/server/realtime/subscribe_ack.rs`
- `apps/filament-server/src/server/metrics.rs`

### Tests
- [x] Unit tests for outbound size rejection.
- [x] Integration test that oversized outbound payload is dropped and counted.
- [x] Regression test proving normal payloads still fan out.

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
- 2026-02-25 (Slice 12): Added outbound size-cap enforcement in the shared channel fanout dispatcher before enqueue, bounded by `max_gateway_event_bytes` and recorded as `filament_gateway_events_dropped_total{scope=\"channel\",event_type=\"...\",reason=\"oversized_outbound\"}`. This slice intentionally covers channel fanout only (via `dispatch_channel_payload`) and adds a focused unit test proving oversized payloads are rejected without queueing or listener eviction.
- 2026-02-25 (Slice 13): Expanded voice-registration planning to use fallible gateway event builders for join/leave/publish/unpublish (`try_voice_participant_join`, `try_voice_participant_leave`, `try_voice_stream_publish`, `try_voice_stream_unpublish`) instead of panic-capable wrappers. `plan_voice_registration_events` now propagates typed `VoiceRegistrationEventBuildError` for all serialization paths so connection runtime can log/metric-drop with existing `gateway.voice_registration.serialize_failed` handling rather than risking panic.
- 2026-02-25 (Slice 14): Migrated voice cleanup removal planning (expired, disconnected-user, and channel-user removal paths) from panic-capable builders to explicit fallible `try_voice_participant_leave`/`try_voice_stream_unpublish` handling. Added typed `VoiceCleanupEventBuildError` propagation through cleanup planners/registry and fail-closed runtime handling in dispatch/runtime (`gateway.voice_cleanup.serialize_failed` warning + `filament_gateway_events_dropped_total{scope=\"channel\",event_type=\"...\",reason=\"serialize_error\"}`), and kept non-`try` voice leave/unpublish builders test-only.
- 2026-02-25 (Slice 15): Expanded outbound size-cap enforcement to guild fanout by adding a pre-dispatch payload-byte guard in `dispatch_guild_payload`, wired `broadcast_guild_event` to pass `max_gateway_event_bytes`, and added a focused guild fanout unit test to verify oversized payloads are dropped with `filament_gateway_events_dropped_total{scope=\"guild\",event_type=\"...\",reason=\"oversized_outbound\"}` before queueing/scanning listeners.
- 2026-02-25 (Slice 16): Migrated only friend-request-create runtime emission from panic-capable `friend_request_create` to fallible `try_friend_request_create` handling in the create-friend-request handler. Serialization/build failure now fail-closes broadcast for that emit path with warning log `gateway.friend_request_create.serialize_failed` and `filament_gateway_events_dropped_total{scope=\"user\",event_type=\"friend_request_create\",reason=\"serialize_error\"}` while preserving the HTTP success response.
- 2026-02-25 (Slice 17): Migrated friend-request-accept outbound emission from panic-capable `friend_request_update` wrappers to fallible `try_friend_request_update` handling in both DB and in-memory accept flows. Serialization/build failures now fail-close per emit with warning log `gateway.friend_request_update.serialize_failed` and `filament_gateway_events_dropped_total{scope=\"user\",event_type=\"friend_request_update\",reason=\"serialize_error\"}` while preserving successful accept responses and friendship persistence.
- 2026-02-25 (Slice 18): Migrated friend-remove outbound emission from panic-capable `friend_remove` wrappers to fallible `try_friend_remove` handling in `remove_friend`. Per-recipient serialization failures now fail closed with warning log `gateway.friend_remove.serialize_failed` and `filament_gateway_events_dropped_total{scope=\"user\",event_type=\"friend_remove\",reason=\"serialize_error\"}`, while successful friendship deletion and HTTP `204` response behavior remain unchanged.
- 2026-02-25 (Slice 19): Migrated friend-request-delete outbound emission from panic-capable `friend_request_delete` wrapper to fallible `try_friend_request_delete` handling in both DB and in-memory delete flows. Serialization/build failures now fail closed with warning log `gateway.friend_request_delete.serialize_failed` and `filament_gateway_events_dropped_total{scope=\"user\",event_type=\"friend_request_delete\",reason=\"serialize_error\"}` while preserving successful request deletion and HTTP `204` behavior.
- 2026-02-25 (Slice 20): Migrated guild workspace-update broadcast emission from panic-capable `workspace_update` construction to fallible `try_workspace_update` handling in `update_guild`. Serialization/build failures now fail closed with warning log `gateway.workspace_update.serialize_failed` and `filament_gateway_events_dropped_total{scope=\"guild\",event_type=\"workspace_update\",reason=\"serialize_error\"}`, while preserving successful guild update HTTP responses and normal fanout on valid payloads.
- 2026-02-25 (Slice 21): Migrated guild role-create broadcast emission from panic-capable `workspace_role_create` construction to fallible `try_workspace_role_create` handling in `create_guild_role`. Serialization/build failures now fail closed with warning log `gateway.workspace_role_create.serialize_failed` and `filament_gateway_events_dropped_total{scope=\"guild\",event_type=\"workspace_role_create\",reason=\"serialize_error\"}`, while preserving successful role creation HTTP responses and normal fanout on valid payloads.
- 2026-02-25 (Slice 22): Migrated guild role-update broadcast emission from panic-capable `workspace_role_update` construction to fallible `try_workspace_role_update` handling in `update_guild_role`. Serialization/build failures now fail closed with warning log `gateway.workspace_role_update.serialize_failed` and `filament_gateway_events_dropped_total{scope=\"guild\",event_type=\"workspace_role_update\",reason=\"serialize_error\"}`, while preserving successful role update HTTP responses and normal fanout on valid payloads.
- 2026-02-25 (Slice 23): Migrated guild role-delete broadcast emission from panic-capable `workspace_role_delete` construction to fallible `try_workspace_role_delete` handling in `delete_guild_role`. Serialization/build failures now fail closed with warning log `gateway.workspace_role_delete.serialize_failed` and `filament_gateway_events_dropped_total{scope=\"guild\",event_type=\"workspace_role_delete\",reason=\"serialize_error\"}`, while preserving successful role deletion and HTTP response behavior.
- 2026-02-25 (Slice 24): Migrated guild channel-create broadcast emission from panic-capable `channel_create` construction to fallible `try_channel_create` handling in `create_channel`. Serialization/build failures now fail closed with warning log `gateway.channel_create.serialize_failed` and `filament_gateway_events_dropped_total{scope=\"guild\",event_type=\"channel_create\",reason=\"serialize_error\"}`, while preserving successful channel creation HTTP responses and normal fanout on valid payloads; added focused builder coverage for `try_channel_create`.
- 2026-02-25 (Slice 25): Migrated message-reaction outbound emission from panic-capable `message_reaction` construction to fallible `try_message_reaction` handling in `broadcast_message_reaction_event`. Serialization/build failures now fail closed with warning log `dropped message_reaction outbound event because serialization failed` and `filament_gateway_events_dropped_total{scope=\"channel\",event_type=\"message_reaction\",reason=\"serialize_error\"}` while preserving reaction mutation behavior and channel fanout for valid payloads; added focused gateway event builder coverage for `try_message_reaction` success fields and invalid event-type rejection.
- 2026-02-25 (Slice 26): Expanded outbound size-cap enforcement to user fanout by adding a pre-dispatch payload-byte guard in `dispatch_user_payload`, wired `broadcast_user_event` to pass `max_gateway_event_bytes`, and added a focused user fanout unit test to verify oversized payloads are dropped with `filament_gateway_events_dropped_total{scope=\"user\",event_type=\"...\",reason=\"oversized_outbound\"}` before queueing or slow-consumer eviction.
- 2026-02-25 (Slice 27): Migrated guild role-reorder outbound emission from panic-capable `workspace_role_reorder` construction to fallible `try_workspace_role_reorder` handling in `reorder_guild_roles`. Serialization/build failures now fail closed with warning log `gateway.workspace_role_reorder.serialize_failed` and `filament_gateway_events_dropped_total{scope=\"guild\",event_type=\"workspace_role_reorder\",reason=\"serialize_error\"}`, while preserving successful role-reorder persistence/audit behavior and HTTP response semantics.
- 2026-02-25 (Slice 28): Migrated guild role-assignment add/remove outbound emission from panic-capable `workspace_role_assignment_add`/`workspace_role_assignment_remove` construction to fallible `try_workspace_role_assignment_add`/`try_workspace_role_assignment_remove` handling in `assign_guild_role` and `unassign_guild_role`. Serialization/build failures now fail closed with warning logs (`gateway.workspace_role_assignment_add.serialize_failed`, `gateway.workspace_role_assignment_remove.serialize_failed`) and `filament_gateway_events_dropped_total{scope=\"guild\",event_type in {\"workspace_role_assignment_add\",\"workspace_role_assignment_remove\"},reason=\"serialize_error\"}`, while preserving successful assignment/unassignment persistence, audit behavior, and HTTP response semantics.
- 2026-02-25 (Slice 29): Migrated member-role update outbound emission from panic-capable `workspace_member_update` construction to fallible `try_workspace_member_update` handling in `set_member_role`. Serialization/build failures now fail closed with warning log `gateway.workspace_member_update.serialize_failed` and `filament_gateway_events_dropped_total{scope=\"guild\",event_type=\"workspace_member_update\",reason=\"serialize_error\"}`, while preserving successful role mutation, audit logging, and HTTP `accepted` response semantics.
- 2026-02-25 (Slice 30): Migrated workspace membership transition emits from panic-capable `workspace_member_add`/`workspace_member_remove`/`workspace_member_ban` builders to fallible `try_workspace_member_add`/`try_workspace_member_remove`/`try_workspace_member_ban` handling in `join_public_guild`, `add_member`, `kick_member`, and `ban_member`. Serialization/build failures now fail closed with warning logs (`gateway.workspace_member_add.serialize_failed`, `gateway.workspace_member_remove.serialize_failed`, `gateway.workspace_member_ban.serialize_failed`) and `filament_gateway_events_dropped_total{scope=\"guild\",event_type in {\"workspace_member_add\",\"workspace_member_remove\",\"workspace_member_ban\"},reason=\"serialize_error\"}` while preserving successful membership mutation, moderation persistence, and HTTP response semantics.
- 2026-02-25 (Slice 31): Added outbound size-cap enforcement for connection-scope `subscribed` acknowledgements by guarding payload bytes before enqueue in `try_enqueue_subscribed_event`. Subscribe ack runtime handling now records explicit drop reasons (`full_queue`, `closed`, `oversized_outbound`) and fail-closes with explicit disconnect reasons (`outbound_queue_full`, `outbound_queue_closed`, `outbound_payload_too_large`) instead of treating all enqueue failures as a single queue-full case; added focused unit tests for enqueue outcome classification and reason mapping.
- 2026-02-25 (Slice 32): Migrated guild IP-ban sync outbound emission from panic-capable `workspace_ip_ban_sync` to fallible `try_workspace_ip_ban_sync` handling in `apply_guild_ip_ban_by_user` and `remove_guild_ip_ban`. Serialization/build failures now fail closed per emit with warning log `gateway.workspace_ip_ban_sync.serialize_failed` and `filament_gateway_events_dropped_total{scope=\"guild\",event_type=\"workspace_ip_ban_sync\",reason=\"serialize_error\"}` while preserving successful IP-ban mutation, audit log writes, and HTTP response semantics; added focused gateway builder coverage for `try_workspace_ip_ban_sync`.
- 2026-02-25 (Slice 33): Migrated remaining channel override migration emits in guild handlers from panic-capable builders to fallible `try_*` paths (`try_workspace_channel_override_update`, `try_workspace_channel_role_override_update`, `try_workspace_channel_permission_override_update_legacy`, `try_workspace_channel_permission_override_update`). Serialization/build failures now fail closed per emit with warning logs and `filament_gateway_events_dropped_total{scope=\"guild\",event_type in {\"workspace_channel_override_update\",\"workspace_channel_role_override_update\",\"workspace_channel_permission_override_update\"},reason=\"serialize_error\"}` while preserving dual-emit migration compatibility and successful HTTP response behavior; updated gateway builder contract tests to assert channel override payloads through the fallible builders.
- 2026-02-25 (Slice 34): Hardened connection bootstrap `ready` emission enqueue path by adding explicit enqueue classification (`Enqueued`/`Full`/`Closed`/`Oversized`) with max outbound payload byte enforcement before queueing. `handle_gateway_connection` now records `filament_gateway_events_dropped_total{scope=\"connection\",event_type=\"ready\",reason in {\"full_queue\",\"closed\",\"oversized_outbound\"}}` on rejected enqueue attempts, records matching disconnect reasons (`outbound_queue_full`, `outbound_queue_closed`, `outbound_payload_too_large`), and fail-closes the connection instead of ignoring send failures; added focused unit coverage for ready enqueue/reason mapping.
- 2026-02-25 (Slice 35): Removed the last non-fallible gateway event builder callsites in server realtime tests (`presence_sync_dispatch`, `voice_sync_dispatch`, `voice_cleanup_dispatch`) by switching to `try_presence_sync`, `try_voice_participant_sync`, `try_voice_participant_leave`, and `try_voice_participant_update` with explicit `expect` checks. This completes the explicit `Result`-handling migration for server-side event construction callsites under `src/server` and closes Phase 1 Task 1.
- 2026-02-25 (Slice 36): Added outbound payload size-cap enforcement for connection-scope `presence_sync` snapshot enqueue by extending presence enqueue classification with `Oversized` and wiring `dispatch_presence_sync_event` to use `max_gateway_event_bytes`. Presence subscribe runtime now fail-closes oversized snapshot enqueue at dispatch time with `filament_gateway_events_dropped_total{scope=\"connection\",event_type=\"presence_sync\",reason=\"oversized_outbound\"}` and no queue mutation; added focused unit tests for oversized enqueue and dispatch outcome mapping.
- 2026-02-25 (Slice 37): Added outbound payload size-cap enforcement for connection-scope `voice_participant_sync` snapshot enqueue by extending voice enqueue classification with `Oversized` and wiring `dispatch_voice_sync_event` to use `max_gateway_event_bytes` from runtime config. Voice subscribe snapshot dispatch now records `filament_gateway_events_dropped_total{scope=\"connection\",event_type=\"voice_participant_sync\",reason=\"oversized_outbound\"}` and skips enqueue on oversized payloads; added focused unit tests for oversized enqueue classification and dispatch outcome mapping.
- 2026-02-25 (Slice 38): Canonicalized outbound rejection reason labels in metrics by introducing shared constants/helpers for `serialize_error` and `oversized_outbound` (`record_gateway_event_serialize_error`, `record_gateway_event_oversized_outbound`), migrated representative runtime callsites (`ready` serialize drop, message-create serialize drop, and shared fanout oversized drop), and added focused unit tests asserting those exact labels are recorded in `filament_gateway_events_dropped_total`.
- 2026-02-25 (Slice 39): Added network integration coverage for channel-scope oversized outbound rejection by introducing `oversized_outbound_channel_event_is_dropped_and_counted` in `gateway_network_flow`. The test constrains `max_gateway_event_bytes`, validates REST message create still succeeds, asserts no websocket `message_create` fanout is emitted for oversized outbound payloads, and verifies drop observability via `filament_gateway_events_dropped_total{scope=\"channel\",event_type=\"message_create\",reason=\"oversized_outbound\"}`.
- 2026-02-25 (Slice 40): Added complementary network regression coverage for normal channel fanout under the same outbound size cap by introducing `normal_sized_outbound_channel_event_still_fans_out_under_size_cap` in `gateway_network_flow`. The test constrains `max_gateway_event_bytes`, emits a small REST message payload, and asserts websocket `message_create` delivery still succeeds (proving size-guard hardening does not regress normal fanout behavior).
- 2026-02-25 (Slice 41): Hardened profile outbound serialization-drop observability by adding structured warning logs for `profile_update` and `profile_avatar_update` emit build failures before fail-closed skip behavior. Profile handlers now use canonical `record_gateway_event_serialize_error("user", event_type)` labels, and a focused unit test verifies serialize-drop metric recording for this profile emit path without introducing panic behavior.
- 2026-02-25 (Slice 42): Hardened connection-scope snapshot reject observability by wiring `handle_presence_subscribe` and `handle_voice_subscribe` to log explicit enqueue rejection outcomes (`closed`, `full_queue`, `oversized_outbound`) after `dispatch_presence_sync_event`/`dispatch_voice_sync_event`, while keeping existing fail-closed behavior and drop metrics unchanged. Added focused unit tests for new reason-mapping helpers in both dispatch modules to guarantee stable reject reason labels.
- 2026-02-25 (Slice 43): Hardened shared channel fanout reject observability by adding structured warning logs in `dispatch_gateway_payload` for `oversized_outbound`, `closed`, and `full_queue` drop paths while preserving existing fail-closed prune/slow-consumer behavior and metric labels. Added focused unit coverage asserting closed/full rejection counters are recorded under stable reason labels in `filament_gateway_events_dropped_total`.
- 2026-02-25 (Slice 44): Hardened remaining non-shared fanout reject observability by adding structured warning logs to user and guild dispatch paths for `oversized_outbound`, `closed`, and `full_queue`, while preserving existing bounded-queue pruning and slow-consumer eviction behavior. Added focused unit tests in `fanout_user` and `fanout_guild` asserting drop reason metrics increment correctly under concurrent test execution (delta-based assertions with unique event types).
- 2026-02-25 (Slice 45): Hardened subscribe-ack reject observability by adding structured warning logs for connection-scope `subscribed` enqueue rejections (`full_queue`, `closed`, `oversized_outbound`) in `execute_subscribe_command`, while preserving existing fail-closed disconnect/error behavior and drop metrics. Added focused unit tests for stable subscribe-ack reject reason mapping used by logging.
- 2026-02-25 (Slice 46): Added focused unit observability coverage for connection-scope oversized snapshot rejects by asserting `filament_gateway_events_dropped_total{scope=\"connection\",event_type in {\"presence_sync\",\"voice_participant_sync\"},reason=\"oversized_outbound\"}` increments in `dispatch_presence_sync_event` and `dispatch_voice_sync_event` oversized paths. This closes the remaining outbound-size unit-test checkbox and completes Phase 1’s final observability/non-panic hardening task.

### Exit Criteria
- Outbound and inbound both enforce size caps.
- No silent fallback to unrelated event types.

---

## Phase 2 - Fanout Data Model Refactor
### Goal
Remove O(N) guild prefix scans and use explicit indices for routing.

### Completion Status
`DONE`

### Tasks
- [x] Introduce indexed subscription registry:
  - channel key -> listeners
  - guild id -> connection ids
  - user id -> connection ids (verify this remains authoritative)
- [x] Update subscription insert/remove paths to maintain all indexes atomically.
- [x] Refactor guild broadcast path to direct index lookup (no string prefix scans).
- [x] Keep dedup semantics and slow-consumer handling unchanged.

### Tentative File Touch List
- `apps/filament-server/src/server/core.rs` (registry types)
- `apps/filament-server/src/server/realtime/subscription_insert.rs`
- `apps/filament-server/src/server/realtime/connection_subscriptions.rs`
- `apps/filament-server/src/server/realtime/connection_registry.rs`
- `apps/filament-server/src/server/realtime/fanout_guild.rs`
- `apps/filament-server/src/server/realtime/connection_runtime.rs`
- `apps/filament-server/src/server/tests/tests/gateway.rs`

### Tests
- [x] Unit tests for index maintenance on subscribe/disconnect.
- [x] Existing dedup test still passes.
- [x] Add stress-oriented test for large subscription maps to validate non-scan path.

### Progress Notes
- 2026-02-25 (Slice 1): Introduced explicit realtime subscription index storage in `RealtimeRegistry` (`guild_connections` and `user_connections`) while preserving the existing channel-key subscription map and current fanout behavior. Wired index maintenance into connection lifecycle/subscription paths: user index now records each connection at gateway connect, guild index now records connection ids on subscribe key insert (parsing `guild_id` from `guild:channel` keys), and both indexes are pruned on disconnect alongside existing subscription cleanup. Added focused unit coverage for index access, subscription key parsing, and index prune semantics without changing protocol/event shapes or existing limits/rate caps.
- 2026-02-25 (Slice 2): Refactored guild fanout dispatch to use direct `guild_id -> connection_ids` index lookup plus `connection_senders`, removing the O(N) subscription-key prefix scan path. `broadcast_guild_event` now dispatches through guild/user indices under existing outbound size caps and drop metrics; closed/full/missing senders are pruned from the guild index while preserving slow-consumer signaling and reject observability labels. Added focused guild fanout unit coverage for delivery parity, closed/full pruning, oversized rejection, and a large-index regression (`2,048` non-target guild entries) validating targeted non-scan behavior.
- 2026-02-25 (Slice 3): Made user fanout target resolution index-based by switching `broadcast_user_event` from `connection_presence` scans to authoritative `user_connections` lookups via `fanout_user_targets::connection_ids_for_user`. Updated target-resolution unit tests to validate `UserConnectionIndex` behavior directly, preserving existing user fanout dispatch semantics (dedup via set membership, slow-consumer close signaling, and outbound size/drop handling) with no protocol changes.
- 2026-02-25 (Slice 4): Made subscribe/disconnect index maintenance atomic at the mutation boundary by updating realtime subscription insert/remove paths to mutate `subscriptions`, `guild_connections`, and `user_connections` within single coordinated write sections. `add_subscription` now acquires subscription + guild index locks together and updates both via a unified insert helper; `remove_connection` now prunes all three index maps in one lock scope via a unified removal helper. Added focused unit tests covering subscribe index insertion (valid/invalid key shapes) and disconnect prune behavior across all indexes, while preserving existing fanout/dedup behavior, queue limits, and drop handling.
- 2026-02-25 (Slice 5): Aligned gateway runtime regression tests with index-based fanout sources so parity assertions exercise the real post-refactor path (`guild_connections` + `connection_senders`, `user_connections` + `connection_senders`) instead of legacy `connection_presence` assumptions. Added focused slow-consumer close tests for guild and user fanout full-queue conditions while preserving existing queue bounds, reject handling, and dedup delivery semantics (single guild delivery per connection despite multiple channel subscriptions).

### Exit Criteria
- Guild broadcast complexity is index-based.
- Behavior parity for delivery, dedup, and slow-consumer close.

---

## Phase 3 - Ingress Domain Typing and Boundary Cleanup
### Goal
Reduce stringly-typed command handling at ingress boundary.

### Completion Status
`DONE`

### Tasks
- [x] Introduce gateway ingress domain types (validated IDs, bounded fields) from DTO conversion.
- [x] Keep DTO structs at transport boundary with `deny_unknown_fields`.
- [x] Move all ID/shape validation into `TryFrom` conversions before handler execution.
- [x] Ensure ingress parse and unknown-event metrics stay intact.

### Tentative File Touch List
- `apps/filament-server/src/server/types.rs`
- `apps/filament-server/src/server/realtime/ingress_command.rs`
- `apps/filament-server/src/server/realtime/ingress_parse.rs`
- `apps/filament-server/src/server/realtime/ingress_subscribe.rs`
- `apps/filament-server/src/server/realtime/ingress_message_create.rs`
- `apps/filament-server/src/server/domain.rs` (if newtypes/helpers live here)

### Tests
- [x] Unit tests for newtype invariants and `TryFrom` conversions.
- [x] Ingress parse tests for invalid IDs and malformed payloads.
- [x] Gateway network tests still pass for subscribe/message_create.

### Progress Notes
- 2026-02-25 (Slice 1): Introduced ingress subscribe domain types with invariant constructors (`GatewayGuildId`, `GatewayChannelId`, `GatewaySubscribeCommand`) and moved subscribe ID validation into `TryFrom<GatewaySubscribe>` during ingress parsing. `execute_subscribe_command` now accepts only validated domain input, preserving fail-closed behavior (`invalid_subscribe_payload`) and existing ingress parse/unknown-event metric classification; added focused parser tests for valid ULID subscribe payloads and malformed/invalid-ID rejection.
- 2026-02-25 (Slice 2): Introduced a typed ingress message-create domain command (`GatewayMessageCreateCommand`) and moved message-create guild/channel ULID validation into `TryFrom<GatewayMessageCreate>` during ingress parsing, so handler execution now receives validated IDs only. `execute_message_create_command` now accepts typed IDs while preserving existing fail-closed parse behavior (`invalid_message_create_payload`) and ingress parse/unknown-event metric classification; added focused parser coverage for valid typed message-create payloads plus invalid-ID rejection.
- 2026-02-25 (Slice 3): Continued ingress domain typing for `message_create` transport conversion by introducing typed attachment-id domain wrapping (`GatewayAttachmentIds`) and normalizing DTO `attachment_ids: Option<Vec<String>>` to a non-optional ingress command field at parse time. `execute_message_create_command` now consumes attachment IDs from typed command state (removing transport-shape fallback logic), and focused parser coverage was added for omitted `attachment_ids` to preserve fail-closed parsing/metric behavior while reducing handler-side stringly DTO handling.
- 2026-02-25 (Slice 4): Strengthened `message_create` ingress domain typing by adding validated domain wrappers for bounded content (`GatewayMessageContent`) and attachment IDs (`GatewayAttachmentIds`) in DTO `TryFrom` conversion. Parsing now fail-closes invalid attachment-id sets (ULID/cap/dedupe validation via shared domain parser) and empty content without attachments before handler execution, while still allowing attachment-only messages and preserving existing `invalid_message_create_payload` parse classification; added focused ingress parser tests for these valid/invalid paths.
- 2026-02-25 (Slice 5): Localized gateway transport DTOs to `realtime/ingress_command.rs` (`GatewaySubscribeDto`, `GatewayMessageCreateDto`) with explicit `#[serde(deny_unknown_fields)]` at the websocket ingress boundary, and removed the generic `types.rs` ingress DTO copies to reduce drift risk. Added focused ingress parser tests proving unknown payload fields fail closed for both `subscribe` and `message_create` with unchanged disconnect reasons/parse classification (`invalid_subscribe_payload`, `invalid_message_create_payload`).
- 2026-02-25 (Slice 6): Moved websocket `message_create` payload shape reliance fully to ingress `TryFrom` output by adding an ingress-only prevalidated create path (`create_message_internal_from_ingress_validated`) that consumes `GatewayMessageContent` and `GatewayAttachmentIds` directly, instead of re-running attachment/content boundary validation inside the handler path. REST `create_message_internal` keeps its existing validation logic unchanged, preserving fail-closed behavior for non-websocket boundaries; added focused unit coverage for prevalidated message body preparation/tokenization behavior.
- 2026-02-25 (Slice 7): Moved subscribe routing-key shape construction (`guild_id:channel_id`) from handler execution into ingress DTO conversion by introducing typed `GatewaySubscriptionKey` in `TryFrom<GatewaySubscribeDto>`. `execute_subscribe_command` now consumes only prevalidated domain input (`GatewayGuildId`, `GatewayChannelId`, `GatewaySubscriptionKey`) and no longer builds subscription keys from raw strings at execution time; updated ingress parser tests to assert typed key derivation while preserving existing parse-failure metrics/disconnect behavior.
- 2026-02-25 (Slice 8): Moved gateway command event-shape validation into domain conversion by implementing `TryFrom<Envelope<Value>> for GatewayIngressCommand` and making `parse_gateway_ingress_command` a thin wrapper over that conversion. Added focused `TryFrom` coverage for invalid subscribe ULIDs and invalid message attachment IDs to prove ID/payload-shape rejection happens before handler execution while preserving existing parse error kinds (`invalid_subscribe_payload`, `invalid_message_create_payload`, `unknown_event`).
- 2026-02-25 (Slice 9): Extended gateway network ingress metrics coverage to assert parse-rejected reason labels for invalid `subscribe` and invalid `message_create` payloads in addition to existing invalid-envelope and unknown-event checks. Re-ran targeted websocket network flow tests (`gateway_ingress_rejections_and_unknown_events_are_counted_in_metrics`, `websocket_handshake_and_message_flow_work_over_network`) to confirm ingress metrics classification and subscribe/message-create runtime behavior remain intact after the ingress typing refactor.

### Exit Criteria
- Handlers execute only with validated domain input.
- No behavior regressions in disconnect semantics.

---

## Phase 4 - Module Consolidation Without Behavior Changes
### Goal
Reduce fragmentation and simplify navigation while preserving testable seams.

### Completion Status
`DONE`

### Tasks
- [x] Consolidate tiny wrapper modules into cohesive components:
  - `realtime/ingress/*`
  - `realtime/fanout/*`
  - `realtime/presence/*`
  - `realtime/voice/*`
- [x] Keep pure helper functions and tests, but reduce one-function files.
- [x] Preserve public/internal function signatures where practical to minimize churn.

### Tentative File Touch List
- `apps/filament-server/src/server/realtime.rs`
- `apps/filament-server/src/server/realtime/*` (module moves/merges)
- `apps/filament-server/src/server/README.md`

### Tests
- [x] Full server test suite plus gateway network flow.
- [x] Clippy and rustdoc clean for moved modules.

### Progress Notes
- 2026-02-25 (Slice 1): Started Phase 4 module consolidation by collapsing the tiny ingress parse-classification wrapper module into `realtime/ingress_command.rs` and removing `realtime/ingress_parse.rs`. `realtime.rs` now imports ingress parse classification directly from `ingress_command`, preserving parse-rejected/unknown-event classification behavior and disconnect/metric semantics while reducing ingress module count by one.
- 2026-02-25 (Slice 2): Continued Phase 4 wrapper consolidation by folding `realtime/presence_disconnect_events.rs` into `realtime/connection_disconnect_followups.rs` and removing the standalone module from `realtime.rs`. Disconnect followup planning still builds offline `presence_update` events via the same fallible `try_presence_update` path (same fail-closed serialize-error propagation and followup semantics), while reducing one-function presence module fragmentation and keeping focused followup/offline event tests in place.
- 2026-02-25 (Slice 3): Continued Phase 4 wrapper consolidation by folding `realtime/presence_subscribe_events.rs` into `realtime/presence_subscribe.rs` and removing the standalone module import from `realtime.rs`. Presence subscribe event construction remains on the same fallible `try_presence_sync`/`try_presence_update` builders with unchanged fail-closed serialize-error handling in `handle_presence_subscribe`, while reducing presence module fragmentation by one file and keeping focused snapshot/online-update builder tests in the consolidated module.
- 2026-02-26 (Slice 4): Continued Phase 4 fanout wrapper consolidation by folding `realtime/fanout_user_targets.rs` into `realtime/fanout_user.rs` and removing the standalone module import from `realtime.rs`. User target resolution remains an index lookup over `user_connections` (`connection_ids_for_user`) with unchanged delivery/dedup/slow-consumer behavior in `broadcast_user_event`; moved lookup tests into the consolidated fanout module to preserve focused coverage while reducing one-function fanout module count by one.
- 2026-02-26 (Slice 5): Continued Phase 4 voice wrapper consolidation by folding `realtime/voice_subscribe_sync.rs` into `realtime/voice_sync_dispatch.rs` and removing the standalone module import from `realtime.rs`. Voice subscribe snapshot event construction remains on the same fallible `try_voice_participant_sync` builder (`try_build_voice_subscribe_sync_event`) used by `handle_voice_subscribe`, with unchanged fail-closed serialize-error/drop behavior and metrics labels; migrated the wrapper’s focused payload-shape tests into the consolidated voice sync module.
- 2026-02-26 (Slice 6): Continued Phase 4 presence wrapper consolidation by folding `realtime/presence_disconnect.rs` into `realtime/connection_disconnect_followups.rs` and removing the standalone module import from `realtime.rs`. Disconnect presence outcome computation (`compute_disconnect_presence_outcome`) and followup planning (`plan_disconnect_followups`) now live in one cohesive disconnect component with unchanged offline-guild detection, voice-cleanup decision semantics, and fallible `presence_update` planning behavior; migrated focused presence-disconnect unit coverage into the consolidated module.
- 2026-02-26 (Slice 7): Continued Phase 4 voice wrapper consolidation by folding `realtime/voice_cleanup_registry.rs` into `realtime/voice_cleanup_dispatch.rs` and removing the standalone module import from `realtime.rs`. Voice cleanup planning and dispatch now live in one cohesive component with unchanged fail-closed serialize-error handling (`gateway.voice_cleanup.serialize_failed` + `filament_gateway_events_dropped_total{scope=\"channel\",reason=\"serialize_error\"}`), while `connection_runtime` now consumes `channel_user_voice_removal_broadcasts` from the consolidated module; migrated focused registry-removal planning tests into `voice_cleanup_dispatch` to preserve cleanup-path coverage.
- 2026-02-26 (Slice 8): Continued Phase 4 presence wrapper consolidation by folding `realtime/presence_sync_dispatch.rs` into `realtime/presence_subscribe.rs` and removing the standalone module import from `realtime.rs`. Presence sync snapshot enqueue dispatch and reject-reason mapping (`dispatch_presence_sync_event`, `presence_sync_reject_reason`) now live in one cohesive presence component with unchanged fail-closed queue/size handling and metric labels (`closed`, `full_queue`, `oversized_outbound`); migrated focused dispatch/reason/oversized-metric tests into the consolidated presence module.
- 2026-02-26 (Slice 9): Continued Phase 4 voice wrapper consolidation by folding `realtime/voice_cleanup_events.rs` into `realtime/voice_cleanup_dispatch.rs` and removing the standalone module import from `realtime.rs`. Voice cleanup event-build/planning helpers (`build_voice_removal_events`, `plan_voice_removal_broadcasts`) now live in the same dispatch component with unchanged fail-closed serialize-error propagation and channel fanout behavior; migrated focused removal-planning tests into the consolidated module to preserve event ordering and channel-key coverage.
- 2026-02-26 (Slice 10): Continued Phase 4 voice wrapper consolidation by folding `realtime/voice_registration_events.rs` into `realtime/voice_registration.rs` and removing the standalone module import from `realtime.rs`. Voice registration transition planning and event-build helpers (`plan_voice_registration_events`) now live in one cohesive voice registration component with unchanged fail-closed serialize-error handling in `register_voice_participant_from_token` (`gateway.voice_registration.serialize_failed` + `filament_gateway_events_dropped_total{scope="channel",reason="serialize_error"}`); migrated focused event-planning tests into the consolidated module to preserve registration transition and malformed old-key coverage.
- 2026-02-26 (Slice 11): Continued Phase 4 search wrapper consolidation by folding `realtime/search_bootstrap.rs` into `realtime/search_runtime.rs` and removing the standalone module import from `realtime.rs`. Search bootstrap rebuild operation construction remains unchanged (`SearchOperation::Rebuild { docs }`) within `ensure_search_bootstrapped`, while reducing one-function search module fragmentation and preserving focused rebuild-construction tests in the consolidated runtime module.
- 2026-02-26 (Slice 12): Continued Phase 4 wrapper consolidation by folding `realtime/connection_control.rs` and `realtime/emit_metrics.rs` into `realtime/connection_runtime.rs`, then removing both standalone module imports from `realtime.rs`. Slow-consumer close signaling (`signal_slow_connections_close`) and delivery metric emission (`emit_gateway_delivery_metrics`) now live alongside channel/guild/user broadcast orchestration with unchanged queue-close signaling, per-delivery emitted metric semantics, and fail-closed runtime behavior; migrated focused unit tests from both deleted wrappers into `connection_runtime` to preserve coverage while reducing module fragmentation by two files.
- 2026-02-26 (Slice 13): Continued Phase 4 fanout wrapper consolidation by folding `realtime/fanout_channel.rs` into `realtime/fanout_dispatch.rs` and removing the standalone module import from `realtime.rs`. Channel fanout key lookup + empty-key pruning (`dispatch_channel_payload`) now live in the shared dispatch component with unchanged outbound size-cap checks, closed/full queue drop handling, and slow-consumer signaling semantics; migrated focused channel fanout wrapper tests into the consolidated dispatch module.
- 2026-02-26 (Slice 14): Continued Phase 4 ingress wrapper consolidation by folding `realtime/ingress_message.rs` into `realtime/ingress_command.rs` and removing the standalone module import from `realtime.rs`. WebSocket ingress message decoding (`decode_gateway_ingress_message`) and its `GatewayIngressMessageDecode` mapping now live in the same ingress boundary component as envelope DTO/domain parsing with unchanged size-cap enforcement and disconnect mappings (`event_too_large`, `client_close`, ping/pong continue); migrated focused decode tests into the consolidated ingress command module.
- 2026-02-26 (Slice 15): Continued Phase 4 ingress wrapper consolidation by folding `realtime/ingress_message_create.rs` into `realtime/ingress_command.rs` and removing the standalone module import from `realtime.rs`. Message-create ingress execution (`execute_message_create_command`) now lives alongside ingress DTO/domain parsing with unchanged fail-closed behavior (`ip_banned`, `message_rejected`) and the same prevalidated message-create path; added focused ingress parser coverage to assert attachment-id dedup normalization remains intact in the consolidated module.
- 2026-02-26 (Slice 16): Continued Phase 4 voice wrapper consolidation by moving `realtime/voice_presence.rs` helper logic (voice channel-key/snapshot collection plus sync-enqueue/outcome mapping) into `realtime/voice_sync_dispatch.rs` with focused tests now colocated in that module. A temporary `voice_presence` compatibility shim currently re-exports the moved helpers to keep this slice low-churn (full module removal planned next), while outbound size-cap enforcement, reject reason labels (`closed`, `full_queue`, `oversized_outbound`), and fail-closed voice subscribe behavior remain unchanged.
- 2026-02-26 (Slice 17): Completed the planned follow-up from Slice 16 by removing the temporary `realtime/voice_presence.rs` compatibility shim and routing remaining call sites directly to `realtime/voice_sync_dispatch` helpers (`voice_channel_key`, `collect_voice_snapshots`, `voice_snapshot_from_record`). This reduces voice-module fragmentation by one file without behavior changes: voice subscribe snapshot dispatch, registration event planning, queue-size caps, reject-reason labels, and fail-closed handling remain unchanged; added focused registration planning assertions to keep join-event payload coverage in the consolidated voice component path.
- 2026-02-26 (Slice 18): Continued Phase 4 search wrapper consolidation by folding `realtime/search_indexed_message.rs` into `realtime/search_runtime.rs` and removing the standalone module import from `realtime.rs`. Indexed message mapping (`indexed_message_from_response`) now lives with search runtime orchestration with unchanged field mapping semantics and no protocol/limit behavior changes; migrated focused mapping coverage into `search_runtime` tests to preserve payload-to-index document parity while reducing one-function search module fragmentation by one file.
- 2026-02-26 (Slice 19): Continued Phase 4 search wrapper consolidation by folding `realtime/search_schema.rs` into `realtime/search_runtime.rs` and removing the standalone `search_schema` module from `realtime.rs`. Search schema construction remains unchanged (`message_id`, `guild_id`, `channel_id`, `author_id`, `created_at_unix`, `content` with default tokenizer), with schema field/type assertions migrated into `search_runtime` tests; updated `search_apply_batch` tests to use `realtime::build_search_schema` so call paths now go through the consolidated runtime module.
- 2026-02-26 (Slice 20): Continued Phase 4 search wrapper consolidation by folding `realtime/search_query_input.rs` into `realtime/search_runtime.rs` and removing the standalone module import from `realtime.rs`. Search query normalization and limit-default validation helpers (`normalize_search_query`, `effective_search_limit`, and `validate_search_query_with_limits`) now live in one cohesive runtime component with unchanged fail-closed query-limit enforcement, while `search_query_run` now consumes the consolidated normalization helper; migrated focused normalization/default-limit tests into `search_runtime` to preserve coverage.
- 2026-02-26 (Slice 21): Continued Phase 4 ingress wrapper consolidation by folding `realtime/ingress_rate_limit.rs` into `realtime/ingress_command.rs` and removing the standalone module import from `realtime.rs`. Gateway ingress rate-limit window checks (`allow_gateway_ingress`) now live alongside ingress message decode and command parse/validation, with unchanged bounded-window semantics (expired-entry eviction, limit gate, and push-on-allow); migrated focused rate-limit unit tests into `ingress_command` to preserve fail-closed `ingress_rate_limited` behavior without altering limits or disconnect handling.
- 2026-02-26 (Slice 22): Continued Phase 4 ingress wrapper consolidation by folding `realtime/ingress_subscribe.rs` into `realtime/ingress_command.rs` and removing the standalone module import/file from `realtime.rs`. Subscribe command execution plus subscribe-ack reject-reason helpers now live in the same ingress boundary component as DTO/domain parsing with unchanged fail-closed behavior (`ip_banned`, `forbidden_channel`, `outbound_queue_full`, `outbound_queue_closed`, `outbound_payload_too_large`, `outbound_serialize_error`), unchanged outbound size-cap enforcement via `try_enqueue_subscribed_event`, and unchanged drop/emitted metrics labels; migrated focused subscribe-ack reason-mapping tests into `ingress_command` to preserve coverage.
- 2026-02-26 (Slice 23): Continued Phase 4 fanout wrapper consolidation by folding `realtime/fanout_guild.rs` into `realtime/fanout_dispatch.rs` and removing the standalone module import/file from `realtime.rs`. Guild index fanout dispatch (`dispatch_guild_payload`) now lives with shared/channel fanout dispatch helpers with unchanged outbound size-cap enforcement, closed/full queue drop metrics, stale-listener pruning, and slow-consumer signaling semantics; migrated focused guild fanout unit tests into the consolidated dispatch module.
- 2026-02-26 (Slice 24): Continued Phase 4 fanout wrapper consolidation by folding `realtime/fanout_user.rs` into `realtime/fanout_dispatch.rs` and removing the standalone module import/file from `realtime.rs`. User index target lookup (`connection_ids_for_user`) and user dispatch (`dispatch_user_payload`) now live with shared/channel/guild fanout helpers, preserving unchanged fail-closed outbound size-cap checks, queue rejection reason labels (`closed`, `full_queue`, `oversized_outbound`), sender pruning, and slow-consumer signaling semantics; migrated focused user fanout unit coverage into the consolidated dispatch module.
- 2026-02-26 (Slice 25): Continued Phase 4 ingress wrapper consolidation by folding `realtime/subscribe_ack.rs` into `realtime/ingress_command.rs` and removing the standalone module import/file from `realtime.rs`. Subscribe-ack enqueue classification (`SubscribeAckEnqueueResult`) and bounded enqueue helper (`try_enqueue_subscribed_event`) now live with ingress subscribe execution, preserving unchanged fail-closed queue/size rejection mappings and drop metric labels (`full_queue`, `closed`, `oversized_outbound`); migrated focused enqueue outcome tests into the consolidated ingress module.
- 2026-02-26 (Slice 26): Continued Phase 4 wrapper consolidation by folding `realtime/ready_enqueue.rs` into `realtime.rs` and removing the standalone module import/file. Ready-event enqueue classification and reason mapping (`ReadyEnqueueResult`, `try_enqueue_ready_event`, `ready_error_reason`, `ready_drop_metric_reason`) now live with gateway connection bootstrap handling, preserving unchanged fail-closed queue/size rejection semantics and metric/drop reason labels; migrated focused enqueue/reason unit tests into `realtime.rs`.
- 2026-02-26 (Slice 27): Continued Phase 4 search wrapper consolidation by folding `realtime/search_blocking.rs` into `realtime/search_query_run.rs` and removing the standalone module import/file from `realtime.rs`. Blocking-search timeout execution (`run_search_blocking_with_timeout`) now lives with query runtime orchestration while preserving unchanged fail-closed timeout and panic-to-internal-error mapping for search tasks; migrated focused timeout/panic unit tests into `search_query_run` and kept existing search index lookup/query runtime behavior unchanged.
- 2026-02-26 (Slice 28): Continued Phase 4 search wrapper consolidation by folding `realtime/search_query_exec.rs` into `realtime/search_query_run.rs` and removing the standalone module import/file from `realtime.rs`. Tantivy query execution (`run_search_query_against_index`) now lives with query input normalization + blocking runtime execution in one cohesive search-query component with unchanged fail-closed parse/internal error handling; migrated focused guild/channel query filtering tests into the consolidated module.
- 2026-02-26 (Slice 29): Continued Phase 4 search wrapper consolidation by folding `realtime/search_index_lookup.rs` into `realtime/search_reconciliation_plan.rs` and removing the standalone module import/file from `realtime.rs`. Index-id lookup input shaping and timeout-bounded blocking execution now live with reconciliation planning (`plan_search_reconciliation`) while preserving unchanged `max_docs` cap behavior and fail-closed timeout/validation semantics; migrated focused lookup input-builder tests into the consolidated reconciliation module.
- 2026-02-26 (Slice 30): Continued Phase 4 search wrapper consolidation by folding `realtime/search_collect_runtime.rs` into `realtime/search_runtime.rs` and removing the standalone module import/file from `realtime.rs`. Search collect runtime branching (DB vs in-memory) now lives with schema/bootstrap/runtime orchestration in one cohesive search component, preserving unchanged fail-closed DB error handling (`AuthFailure::Internal`), guild doc-cap enforcement (`enforce_guild_collect_doc_cap`), and existing query/result limits; migrated focused row-mapping cap tests into `search_runtime` to preserve coverage while reducing one-function wrapper fragmentation by one file.
- 2026-02-26 (Slice 31): Continued Phase 4 message wrapper consolidation by folding `realtime/message_create_response.rs` into `realtime/message_record.rs` and removing the standalone module import/file from `realtime.rs`. DB-created message response mapping (`build_db_created_message_response`) now lives with other message record/response mapping helpers, preserving unchanged `message_create` response payload semantics (including empty `reactions` initialization) while reducing one-function message module fragmentation by one file; migrated focused response-mapping coverage into `message_record` tests.
- 2026-02-26 (Slice 32): Continued Phase 4 search wrapper consolidation by folding `realtime/search_validation.rs` into `realtime/search_runtime.rs` and removing the standalone module import/file from `realtime.rs`. Search query limit enforcement helper (`validate_search_query_limits`) now lives with search runtime normalization/validation orchestration, preserving unchanged fail-closed limits (query chars, result limit bounds, term cap, wildcard/fuzzy caps, and field-query rejection); migrated focused validation tests into `search_runtime` tests to preserve strict parsing coverage.
- 2026-02-26 (Slice 33): Continued Phase 4 search wrapper consolidation by folding `realtime/search_reconcile.rs` into `realtime/search_reconciliation_plan.rs` and removing the standalone module import/file from `realtime.rs`. Reconciliation diff computation (`compute_reconciliation`) now lives with reconciliation planning (`build_search_reconciliation_plan`) in one cohesive search component, preserving unchanged sorted upsert/delete semantics and max-doc constrained lookup behavior; migrated focused reconciliation sorting tests into the consolidated planning module.
- 2026-02-26 (Slice 34): Continued Phase 4 hydration wrapper consolidation by folding `realtime/hydration_order.rs` into `realtime/hydration_runtime.rs` and removing the standalone module import/file from `realtime.rs`. Ordered hydration assembly (`collect_hydrated_in_request_order`) now lives in the runtime hydration component with unchanged fail-closed missing-id behavior and request-order semantics; migrated focused ordering tests into `hydration_runtime`.
- 2026-02-26 (Slice 35): Continued Phase 4 hydration wrapper consolidation by folding `realtime/hydration_merge.rs` into `realtime/hydration_runtime.rs` and removing the standalone module import/file from `realtime.rs`. Attachment/reaction merge helper (`merge_hydration_maps`) now lives with hydration runtime orchestration, preserving unchanged per-message merge semantics and fail-closed defaulting for missing map entries; migrated focused merge tests into the consolidated module.
- 2026-02-26 (Slice 36): Continued Phase 4 hydration wrapper consolidation by folding `realtime/hydration_in_memory_attachments.rs` into `realtime/hydration_runtime.rs` and removing the standalone module import/file from `realtime.rs`. In-memory attachment hydration helper (`apply_hydration_attachments`) now lives with hydration runtime orchestration, preserving unchanged attachment overwrite/clear semantics for message ids and keeping focused attachment mapping tests in the consolidated module.
- 2026-02-26 (Slice 37): Continued Phase 4 search wrapper consolidation by folding `realtime/search_batch_drain.rs`, `realtime/search_collect_all.rs`, and `realtime/search_collect_guild.rs` into `realtime/search_runtime.rs` and removing standalone module imports/files from `realtime.rs`. Search worker batch-drain behavior and in-memory collect paths remain unchanged (same fail-closed guild cap enforcement and `NotFound`/`InvalidRequest` semantics), with focused unit tests migrated into `search_runtime` to preserve coverage while reducing one-function search wrapper fragmentation.
- 2026-02-26 (Slice 38): Continued Phase 4 message wrapper consolidation by folding `realtime/message_emit.rs` into `realtime.rs` and removing the standalone module import/file. Message-create outbound emit + search-index enqueue behavior remains unchanged (same fail-closed serialize-drop metric label `serialize_error`, warning log, and channel fanout/search upsert flow), with the focused `message_upsert_operation` mapping test migrated into `realtime` tests and no external signature changes required.
- 2026-02-26 (Status correction): Left the Phase 4 wrapper-reduction/signature-preservation tasks unchecked because wrapper reduction is still in progress; additional one-function modules remain and full closure criteria has not been met yet.
- 2026-02-26 (Slice 39): Continued Phase 4 connection wrapper consolidation by folding `realtime/connection_registry.rs`, `realtime/connection_subscriptions.rs`, and `realtime/subscription_insert.rs` into `realtime/connection_runtime.rs`, then removing all three standalone module imports/files from `realtime.rs`. Connection add/remove index maintenance behavior remains unchanged: `add_subscription` still atomically updates `subscriptions` + `guild_connections`, `remove_connection` still prunes `subscriptions` + `guild_connections` + `user_connections`, and slow-consumer/delivery/disconnect semantics are preserved; migrated focused unit tests for connection state pruning, index pruning, and subscription-key guild indexing into `connection_runtime` tests.
- 2026-02-26 (Slice 40): Continued Phase 4 search wrapper consolidation by folding `realtime/search_apply_batch.rs` into `realtime/search_runtime.rs` and removing the standalone module import/file from `realtime.rs`. Search batch apply + ack semantics remain unchanged (same writer/commit/reload flow and fail-closed ack mapping to `AuthFailure::Internal` on apply failure), with focused success/failure ack tests migrated into `search_runtime` to preserve coverage while reducing one-function search wrapper fragmentation.
- 2026-02-26 (Slice 41): Continued Phase 4 search wrapper consolidation by folding `realtime/search_enqueue.rs` into `realtime/search_runtime.rs` and removing the standalone module import/file from `realtime.rs`. Search enqueue + ack behavior remains unchanged (`wait_for_apply` still uses oneshot ack and fail-closed `AuthFailure::Internal` on send/ack failures), with focused enqueue unit coverage migrated into `search_runtime` tests.
- 2026-02-26 (Slice 42): Continued Phase 4 search wrapper consolidation by folding `realtime/search_collect_db.rs` into `realtime/search_runtime.rs` and removing the standalone module import/file from `realtime.rs`. DB row-to-indexed-message mapping and guild fetch-limit/doc-cap guard behavior remain unchanged (`max_docs + 1` fail-closed cap check and `AuthFailure::InvalidRequest` over-cap rejection), with focused row-mapping/cap tests migrated into `search_runtime` tests.
- 2026-02-26 (Slice 43): Continued Phase 4 search wrapper consolidation by folding `realtime/search_collect_index_ids.rs` into `realtime/search_reconciliation_plan.rs` and removing the standalone module import/file from `realtime.rs`. Tantivy guild index-id lookup remains fail-closed (internal search errors map to `AuthFailure::Internal`, over-cap counts map to `AuthFailure::InvalidRequest`) with unchanged reconciliation planning behavior, and focused guild index-id lookup tests migrated into `search_reconciliation_plan`.
- 2026-02-26 (Slice 44): Continued Phase 4 hydration wrapper consolidation by folding `realtime/hydration_db.rs` and `realtime/hydration_in_memory.rs` into `realtime/hydration_runtime.rs`, then removing both standalone module imports/files from `realtime.rs`. Hydration behavior is unchanged: DB/in-memory message collection, request-order assembly, attachment/reaction merge, and fail-closed channel lookup semantics all remain intact; migrated focused DB-row mapping and in-memory hydration tests into the consolidated runtime module.
- 2026-02-26 (Slice 45): Continued Phase 4 message wrapper consolidation by folding `realtime/message_attachment_bind.rs` and `realtime/message_store_in_memory.rs` into `realtime/message_record.rs`, then removing both standalone module imports/files from `realtime.rs`. Message attachment binding constraints and in-memory append fail-closed semantics remain unchanged (`InvalidRequest` for invalid bindings, `NotFound` for missing guild/channel), with focused binding and append tests migrated into the consolidated message module.
- 2026-02-26 (Slice 46): Continued Phase 4 message wrapper consolidation by folding `realtime/message_prepare.rs` into `realtime.rs` and removing the standalone module import/file. Message body preparation remains unchanged for both REST and ingress-prevalidated paths (same empty-content guard behavior, message-content validation, and markdown tokenization semantics); migrated focused message-prepare unit coverage into `realtime` tests.
- 2026-02-26 (Slice 47): Continued Phase 4 search wrapper consolidation by folding `realtime/search_apply.rs` into `realtime/search_runtime.rs` and removing the standalone module import/file from `realtime.rs`. Search operation application remains unchanged for upsert/delete/rebuild/reconcile flows (same delete-before-upsert and fail-closed writer behavior), with focused index mutation tests migrated into `search_runtime` tests.
- 2026-02-26 (Slice 48): Reduced hydration indirection by moving the search-facing hydration runtime entrypoint from `realtime/hydration_runtime.rs` into `realtime/search_runtime.rs`. `hydrate_messages_by_id` now orchestrates DB/in-memory hydration directly while reusing consolidated hydration helper functions (`collect_*`, merge/order helpers), preserving the same fail-closed behavior, attachment/reaction hydration semantics, and external function signature for search handlers.
- 2026-02-26 (Validation status): Ran `cargo test -p filament-server` after slices 46-48; suite fails at pre-existing docs-contract drift (`server::tests::tests::contract::api_docs_cover_router_manifest_routes` expects `POST /guilds/{guild_id}/roles/default` in `docs/API.md`). Ran `cargo clippy -p filament-server --tests --no-deps`; warnings remain in pre-existing non-Phase-4 areas (`gateway_events/workspace.rs`, `handlers/guilds.rs`, `types.rs`, unused `presence_sync` export).
- 2026-02-26 (Slice 49): Closed the outstanding Phase 4 validation tasks by restoring API docs route parity for `POST /guilds/{guild_id}/roles/default`, removing the stale `presence_sync` test-only export/wrapper left after module moves, and stabilizing `gateway_network_flow` voice cleanup assertions around the deterministic `voice/leave` cleanup path. Re-ran `cargo test -p filament-server` (full suite, including `gateway_network_flow`) and explicit `cargo test -p filament-server --test gateway_network_flow` with all tests passing; ran `cargo clippy -p filament-server --tests --no-deps` and confirmed remaining warnings are pre-existing outside the Phase 4 moved-module surfaces; ran `RUSTDOCFLAGS='-D warnings' cargo doc -p filament-server --no-deps` successfully.

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
