# PLAN_EVENT.md

## Objective
Build a complete, secure realtime event model so the app shell and RTC-adjacent UI can stay in sync without manual refreshes, and split client vs workspace settings UX into distinct panels.

## Requested Scope
- Audit all mutable surfaces that can change UI state.
- Define missing gateway update events and phased implementation plan.
- Include explicit phase to:
  - restore client settings entry (gear at bottom of channel rail)
  - move workspace/server settings into its own panel opened from workspace dropdown

## Code-State Audit (2026-02-12)

### Current gateway events actually emitted (server)
- `ready`
- `subscribed`
- `message_create`
- `message_reaction`
- `channel_create`
- `presence_sync`
- `presence_update`

### Current gateway events actually handled (web)
- `message_create`
- `message_reaction`
- `channel_create`
- `presence_sync`
- `presence_update`

### Current mutation coverage gap
Only `create_message`, reaction add/remove, and `create_channel` broadcast updates. Most REST mutations do not emit gateway events.

### High-impact mutable surfaces with no broadcast update
- Message lifecycle:
  - `PATCH /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}` (edit)
  - `DELETE /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}` (delete)
- Workspace structure/settings:
  - no endpoint currently exists for workspace rename/visibility update
  - no workspace update event currently exists
- Channel lifecycle/settings:
  - channel rename/delete/reorder endpoints do not exist yet (future API surface) and therefore no events
- Workspace membership/moderation:
  - `POST /guilds/{guild_id}/members/{user_id}`
  - `PATCH /guilds/{guild_id}/members/{user_id}`
  - `POST /guilds/{guild_id}/members/{user_id}/kick`
  - `POST /guilds/{guild_id}/members/{user_id}/ban`
  - `POST /guilds/{guild_id}/join`
- Roles/permissions:
  - role create/update/delete/reorder
  - role assignment/unassignment
  - channel override updates
- Profile and social state:
  - `PATCH /users/me/profile`
  - `POST /users/me/profile/avatar`
  - friend request create/accept/delete and friend remove
- Attachments:
  - upload/delete attachment currently rely on local updates/fetch, no broadcast event contract

### Settings UX state
- Workspace dropdown “Server Settings” currently opens the same generic `settings` panel used for client settings.
- Channel rail account footer currently has no gear action (regression from prior commit).
- Existing settings panel is client-centric (`Voice`, `Profile`) and should remain client settings.

## Locked Design Rules For This Plan
- Keep protocol envelope `{ v, t, d }` and backward-compatible parsing.
- Keep strict payload validation on web and server boundaries.
- No event may exceed configured gateway limits (`max_gateway_event_bytes`).
- Reuse bounded queues / slow-consumer protections; do not relax limits.
- Event names remain bounded lowercase snake-case (`[a-z0-9_.]{1,64}` already enforced on web).
- Prefer additive events over overloaded payload shapes.

## Event Taxonomy (target)
- Channel-scoped: updates relevant to one channel stream.
- Guild-scoped: updates relevant across channels in a workspace.
- User-scoped: updates relevant to one authenticated user (profile/friendship).

## Mutation-to-Event Backlog (source of truth)
- Message edit -> `message_update`
- Message delete -> `message_delete`
- Channel create (already exists) -> `channel_create`
- Channel rename -> `channel_update` (future endpoint)
- Channel delete -> `channel_delete` (future endpoint)
- Workspace rename/visibility update -> `workspace_update`
- Member joined workspace -> `workspace_member_add`
- Member role changed -> `workspace_member_update`
- Member kicked -> `workspace_member_remove`
- Member banned -> `workspace_member_ban`
- Role created -> `workspace_role_create`
- Role updated -> `workspace_role_update`
- Role deleted -> `workspace_role_delete`
- Role reorder -> `workspace_role_reorder`
- Role assigned to member -> `workspace_role_assignment_add`
- Role unassigned from member -> `workspace_role_assignment_remove`
- Channel role override changed -> `workspace_channel_override_update`
- Guild IP bans changed -> `workspace_ip_ban_sync` (redacted, no raw IP exposure)
- Profile updated -> `profile_update`
- Profile avatar updated -> `profile_avatar_update`
- Friend request created/accepted/deleted -> `friend_request_create|friend_request_update|friend_request_delete`
- Friend removed -> `friend_remove`

## Status Legend
- `NOT STARTED`
- `IN PROGRESS`
- `DONE`
- `BLOCKED`

---

## Phase 0 - Contract + Inventory Lock
### Goal
Freeze event contracts and explicit endpoint-to-event mapping before implementation.

### Completion Status
`DONE`

### Tasks
- [x] Add `docs/GATEWAY_EVENTS.md` with canonical event list, payload schemas, scope, and auth visibility rules.
- [x] Define minimum payload fields per event (`guild_id`/`channel_id`/entity IDs + changed fields).
- [x] Decide and document actor metadata policy (`actor_user_id` included only where safe/useful).
- [x] Add compatibility notes: unknown events ignored by old clients.
- [x] Record final endpoint-to-event mapping table in this plan after signoff.

### Locked Endpoint-to-Event Mapping (Phase 0 signoff)
| Mutation endpoint | Event(s) | Scope | Status |
| --- | --- | --- | --- |
| `POST /guilds/{guild_id}/channels/{channel_id}/messages` | `message_create` | channel | Implemented |
| `PATCH /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}` | `message_update` | channel | Implemented |
| `DELETE /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}` | `message_delete` | channel | Implemented |
| `POST/DELETE /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}/reactions/{emoji}` | `message_reaction` | channel | Implemented |
| `POST /guilds/{guild_id}/channels` | `channel_create` | guild | Implemented |
| `PATCH /guilds/{guild_id}/channels/{channel_id}` | `channel_update` | guild | Planned (future endpoint) |
| `DELETE /guilds/{guild_id}/channels/{channel_id}` | `channel_delete` | guild | Planned (future endpoint) |
| `PATCH /guilds/{guild_id}` | `workspace_update` | guild | Implemented |
| `POST /guilds/{guild_id}/join` | `workspace_member_add` | guild | Implemented |
| `POST /guilds/{guild_id}/members/{user_id}` | `workspace_member_add` | guild | Implemented |
| `PATCH /guilds/{guild_id}/members/{user_id}` | `workspace_member_update` | guild | Implemented |
| `POST /guilds/{guild_id}/members/{user_id}/kick` | `workspace_member_remove` | guild | Implemented |
| `POST /guilds/{guild_id}/members/{user_id}/ban` | `workspace_member_ban`, `workspace_member_remove` | guild | Implemented |
| `POST /guilds/{guild_id}/roles` | `workspace_role_create` | guild | Planned (Phase 4) |
| `PATCH /guilds/{guild_id}/roles/{role_id}` | `workspace_role_update` | guild | Planned (Phase 4) |
| `DELETE /guilds/{guild_id}/roles/{role_id}` | `workspace_role_delete` | guild | Planned (Phase 4) |
| `POST /guilds/{guild_id}/roles/reorder` | `workspace_role_reorder` | guild | Planned (Phase 4) |
| `POST /guilds/{guild_id}/roles/{role_id}/members/{user_id}` | `workspace_role_assignment_add` | guild | Planned (Phase 4) |
| `DELETE /guilds/{guild_id}/roles/{role_id}/members/{user_id}` | `workspace_role_assignment_remove` | guild | Planned (Phase 4) |
| `POST /guilds/{guild_id}/channels/{channel_id}/overrides/{role}` | `workspace_channel_override_update` | guild | Planned (Phase 4) |
| `GET/POST/DELETE /guilds/{guild_id}/ip-bans...` | `workspace_ip_ban_sync` | guild | Planned (Phase 4, redacted payload only) |
| `PATCH /users/me/profile` | `profile_update` | user | Implemented |
| `POST /users/me/profile/avatar` | `profile_avatar_update` | user | Implemented |
| `POST /friends/requests` | `friend_request_create` | user | Implemented |
| `POST /friends/requests/{request_id}/accept` | `friend_request_update` | user | Implemented |
| `DELETE /friends/requests/{request_id}` | `friend_request_delete` | user | Implemented |
| `DELETE /friends/{friend_user_id}` | `friend_remove` | user | Implemented |

### Refactor Notes
- Added `docs/GATEWAY_EVENTS.md` as the contract source for:
  - canonical event list and minimum payload schemas
  - scope and auth visibility rules
  - actor metadata policy for optional `actor_user_id`
  - compatibility guidance for unknown events in mixed-version clients

### Exit Criteria
- Team-aligned contract exists before coding event fanout.

---

## Phase 1 - Gateway Event Infrastructure
### Goal
Make event emission consistent and support channel/guild/user fanout targets.

### Completion Status
`DONE`

### Tasks
- [x] Add typed helper constructors for common event payloads (avoid ad-hoc JSON blobs).
- [x] Add `broadcast_user_event` path keyed by authenticated user connections.
- [x] Keep strict event-size checks and queue bounds unchanged.
- [x] Add tracing/metrics for emitted event type and dropped events (slow consumer/full queue).
- [x] Add server tests for channel/guild/user fanout correctness and unauthorized non-delivery.

### Refactor Notes
- Added `apps/filament-server/src/server/gateway_events.rs` with typed gateway payload constructors and event-name constants for:
  - `ready`, `subscribed`
  - `message_create`, `message_reaction`
  - `channel_create`
  - `presence_sync`, `presence_update`
- Replaced ad-hoc JSON event construction in:
  - `apps/filament-server/src/server/realtime.rs`
  - `apps/filament-server/src/server/handlers/messages.rs`
  - `apps/filament-server/src/server/handlers/guilds.rs`
- Added user-targeted fanout plumbing in realtime state:
  - new `connection_senders` map in `AppState`
  - new `broadcast_user_event` function keyed by authenticated `user_id`
  - `handle_gateway_connection` now registers/removes per-connection outbound senders
- Added gateway emission/drop observability:
  - new metrics counters in `MetricsState` and `metrics.rs`:
    - `filament_gateway_events_emitted_total{scope,event_type}`
    - `filament_gateway_events_dropped_total{scope,event_type,reason}`
  - tracing events on successful fanout with scope/type/delivered count
- Added server fanout correctness tests in `apps/filament-server/src/server/tests.rs`:
  - channel fanout only reaches the targeted channel key
  - guild fanout deduplicates per connection and does not cross guild boundaries
  - user fanout reaches all target user sessions and does not leak to other users

### Exit Criteria
- Server can publish safely to all required scopes with tests.

---

## Phase 2 - Message Lifecycle Events
### Goal
Cover full message CRUD-driven UI updates.

### Completion Status
`DONE`

### Tasks
- [x] Emit `message_update` from message edit endpoint.
- [x] Emit `message_delete` from message delete endpoint.
- [x] Keep `message_reaction` event behavior, add tests for zero-count delete semantics.
- [x] Update web gateway parser/controller/state reducers for new message events.
- [x] Add server + web tests for edit/delete realtime sync across multiple clients.

### Refactor Notes
- Added typed gateway event constructors in `apps/filament-server/src/server/gateway_events.rs`:
  - `message_update` with bounded `updated_fields` (`content`, `markdown_tokens`) + `updated_at_unix`
  - `message_delete` with `deleted_at_unix`
- Wired message lifecycle broadcasts in `apps/filament-server/src/server/handlers/messages.rs` for both DB and in-memory execution paths, preserving existing fanout scope/size checks.
- Expanded web gateway boundary parsing in `apps/filament-client-web/src/lib/gateway.ts` for `message_update`/`message_delete` with strict fail-closed payload validation.
- Added deterministic reducer helpers in `apps/filament-client-web/src/features/app-shell/controllers/gateway-controller.ts` for patching and removing messages plus reaction state cleanup on delete.
- Added realtime multi-client coverage:
  - server integration test in `apps/filament-server/tests/gateway_network_flow.rs`
  - web gateway/controller tests in:
    - `apps/filament-client-web/tests/gateway.test.ts`
    - `apps/filament-client-web/tests/app-shell-gateway-controller.test.ts`

### Exit Criteria
- Editing/deleting a message on one client updates other subscribed clients without refresh.

---

## Phase 3 - Workspace + Membership Events
### Goal
Sync workspace-level structural and membership state.

### Completion Status
`DONE`

### Tasks
- [x] Add workspace update API endpoint(s) for rename/visibility (if missing) with strict validation.
- [x] Emit `workspace_update` for server name/settings updates.
- [x] Emit membership events for join/add/role/kick/ban flows.
- [x] Emit `workspace_member_remove` on kick and ban; ensure active-channel safety behavior on web.
- [x] Web reducers update workspace list/member rails/permission-sensitive views in-place.
- [x] Add integration tests for multi-client workspace rename and membership transitions.

### Refactor Notes
- Added `PATCH /guilds/{guild_id}` in `apps/filament-server/src/server/router.rs` and implemented `update_guild` in `apps/filament-server/src/server/handlers/guilds.rs` with:
  - strict `GuildName`/`GuildVisibility` validation
  - permission enforcement via `Permission::ManageRoles`
  - fail-closed empty update rejection
  - additive `workspace_update` emission only when fields actually changed
- Expanded typed gateway contracts in `apps/filament-server/src/server/gateway_events.rs` for:
  - `workspace_update`
  - `workspace_member_add`
  - `workspace_member_update`
  - `workspace_member_remove`
  - `workspace_member_ban`
- Wired membership fanout in `apps/filament-server/src/server/handlers/guilds.rs`:
  - `join_public_guild` emits `workspace_member_add` on accepted join
  - `add_member` emits `workspace_member_add` on new insertion
  - `update_member_role` emits `workspace_member_update`
  - `kick_member` emits `workspace_member_remove` (`reason = kick`)
  - `ban_member` emits both `workspace_member_ban` and `workspace_member_remove` (`reason = ban`)
- Extended web gateway boundary validation in `apps/filament-client-web/src/lib/gateway.ts` for all Phase 3 workspace/member events and strict ready payload parsing (`ready.user_id`).
- Updated web reducers in `apps/filament-client-web/src/features/app-shell/controllers/gateway-controller.ts` to:
  - apply in-place workspace name/visibility updates
  - drop the workspace from local state when the authenticated user receives a self-targeted remove/ban event (active-channel safety path via existing selection controller)
- Added/updated tests:
  - server integration coverage in `apps/filament-server/tests/gateway_network_flow.rs` for multi-client workspace rename + membership transition fanout
  - web parser/controller coverage in:
    - `apps/filament-client-web/tests/gateway.test.ts`
    - `apps/filament-client-web/tests/app-shell-gateway-controller.test.ts`

### Exit Criteria
- Workspace name/settings and membership changes propagate in realtime.

---

## Phase 4 - Roles, Overrides, Moderation Events
### Goal
Realtime sync for permission graph changes that alter visible capabilities.

### Completion Status
`DONE`

### Tasks
- [x] Emit role lifecycle events (`workspace_role_create|update|delete|reorder`).
- [x] Emit role assignment events per member.
- [x] Emit channel override update events.
- [x] Emit redacted IP-ban sync event for moderation views (no raw IP fields).
- [x] Web applies permission-impacting events by refreshing snapshots where needed and pruning inaccessible channels.
- [x] Add tests proving no privileged data leakage in event payloads.

### Refactor Notes
- Expanded typed gateway contracts in `apps/filament-server/src/server/gateway_events.rs` for:
  - `workspace_role_create`
  - `workspace_role_update`
  - `workspace_role_delete`
  - `workspace_role_reorder`
  - `workspace_role_assignment_add`
  - `workspace_role_assignment_remove`
  - `workspace_channel_override_update`
  - `workspace_ip_ban_sync` (redacted summary only)
- Wired role/override/moderation event emission in `apps/filament-server/src/server/handlers/guilds.rs` across:
  - role create/update/delete/reorder endpoints
  - role assign/unassign endpoints
  - channel override update endpoint
  - guild IP-ban add/remove endpoints
- Extended web gateway validation and dispatch in `apps/filament-client-web/src/lib/gateway.ts` for all Phase 4 event payloads with strict fail-closed parsing.
- Updated app-shell realtime behavior in:
  - `apps/filament-client-web/src/features/app-shell/controllers/gateway-controller.ts`
  - `apps/filament-client-web/src/features/app-shell/runtime/create-app-shell-runtime.ts`
  so permission-impacting events trigger role/permission refresh and channel pruning on access loss.
- Added/updated tests:
  - server integration coverage in `apps/filament-server/tests/gateway_network_flow.rs` for role lifecycle/assignment, channel override, and redacted IP-ban sync fanout
  - web parser coverage in `apps/filament-client-web/tests/gateway.test.ts`
  - web controller coverage in `apps/filament-client-web/tests/app-shell-gateway-controller.test.ts`

### Exit Criteria
- Permission and moderation UI reflects role/override/ban changes without manual reload.

---

## Phase 5 - Profile + Friendship User-Scoped Events
### Goal
Keep client profile/social panels synchronized across sessions/devices.

### Completion Status
`DONE`

### Tasks
- [x] Emit `profile_update` and `profile_avatar_update` to the acting user and relevant observers where permitted.
- [x] Emit friendship request/friendship state events as user-scoped updates.
- [x] Add web handlers to update profile cache, avatar versions, and friendship panel state incrementally.
- [x] Add tests for multi-session profile update propagation and friend request lifecycle sync.

### Refactor Notes
- Expanded typed gateway contracts in `apps/filament-server/src/server/gateway_events.rs` for:
  - `profile_update`
  - `profile_avatar_update`
  - `friend_request_create`
  - `friend_request_update`
  - `friend_request_delete`
  - `friend_remove`
- Wired user-scoped event fanout in:
  - `apps/filament-server/src/server/handlers/profile.rs` to emit profile/avatar updates to the acting user and confirmed friendship observers
  - `apps/filament-server/src/server/handlers/friends.rs` to emit request lifecycle and friendship removal events to affected participants
- Extended web gateway boundary parsing and dispatch in `apps/filament-client-web/src/lib/gateway.ts` for all Phase 5 payloads with strict fail-closed validation.
- Updated app-shell realtime reducers in:
  - `apps/filament-client-web/src/features/app-shell/controllers/gateway-controller.ts`
  - `apps/filament-client-web/src/features/app-shell/runtime/create-app-shell-runtime.ts`
  so profile drafts, avatar version cache, friendship requests, and friend list state update incrementally from gateway events.
- Added/updated tests:
  - server integration coverage in `apps/filament-server/tests/gateway_network_flow.rs` for multi-session profile/friendship event fanout and observer scoping
  - web parser coverage in `apps/filament-client-web/tests/gateway.test.ts`
  - web controller coverage in `apps/filament-client-web/tests/app-shell-gateway-controller.test.ts`

### Exit Criteria
- Profile and friendship state stays fresh across concurrent sessions.

---

## Phase 6 - Settings UX Split (Client vs Workspace)
### Goal
Fix settings information architecture and restore expected entry points.

### Completion Status
`DONE`

### Tasks
- [x] Add separate overlay panels/types:
  - `client-settings` (existing Voice/Profile content)
  - `workspace-settings` (workspace/server config)
- [x] Restore gear icon button in channel rail account footer to open `client-settings`.
- [x] Keep workspace dropdown “Server Settings” but wire it to `workspace-settings`.
- [x] Move workspace-specific controls out of client settings panel.
- [x] Add/update tests:
  - gear opens client settings
  - workspace menu opens workspace settings
  - panel titles/authorization behavior remain correct

### Refactor Notes
- Split overlay panel types and routing to:
  - `client-settings` for Voice/Profile settings
  - `workspace-settings` for workspace rename/visibility controls
- Added `WorkspaceSettingsPanel` in `apps/filament-client-web/src/features/app-shell/components/panels/WorkspaceSettingsPanel.tsx` with:
  - bounded name/visibility form inputs
  - permission-gated save action (`manage_roles`/role-management access)
  - explicit status/error feedback
- Restored channel-rail account footer settings gear in `apps/filament-client-web/src/features/app-shell/components/ChannelRail.tsx` and wired:
  - footer gear -> `client-settings`
  - workspace dropdown Server Settings -> `workspace-settings`
- Added `updateGuild` API client method in `apps/filament-client-web/src/lib/api.ts` and runtime save flow in `apps/filament-client-web/src/features/app-shell/runtime/create-app-shell-runtime.ts`.
- Extended panel host prop adapters and panel host wiring to include workspace settings panel props and lazy loading.
- Updated web tests for:
  - split entry points and panel titles
  - workspace settings save flow
  - workspace settings authorization-disabled behavior

### Exit Criteria
- Client and workspace settings are clearly separated and reachable from correct entry points.

---

## Phase 7 - Hardening, Rollout, and Regression Net
### Goal
Finalize stability and prevent event regressions.

### Completion Status
`DONE`

### Tasks
- [x] Add contract tests for all gateway event payload validators (server + web).
- [x] Add end-to-end multi-client sync tests for key flows (message, workspace rename, role changes, profile).
- [x] Add event observability counters (`emitted`, `dropped`, `unknown_received`, `parse_rejected`).
- [x] Add rollout checklist and fallback behavior notes for mixed-version clients.
- [x] Update this file with implementation notes after each completed phase.

### Refactor Notes
- Added missing gateway ingress observability counters in server metrics:
  - `filament_gateway_events_unknown_received_total{scope,event_type}`
  - `filament_gateway_events_parse_rejected_total{scope,reason}`
  and wired them in `apps/filament-server/src/server/realtime.rs` for malformed envelope/payload and unknown ingress event paths.
- Expanded metrics output and baseline coverage:
  - updated `apps/filament-server/src/server/metrics.rs`
  - updated `apps/filament-server/src/server/tests.rs` metrics endpoint assertions
  - added network-level metric regression in `apps/filament-server/tests/gateway_network_flow.rs` to verify unknown + parse-rejected ingress counters increment through real websocket traffic.
- Added server-side gateway event contract tests in `apps/filament-server/src/server/gateway_events.rs`:
  - validates envelope + minimum payload fields for all emitted event constructors
  - asserts redaction invariants for `workspace_ip_ban_sync` payloads (no raw IP fields).
- Added web gateway validator contract coverage in `apps/filament-client-web/tests/gateway.test.ts`:
  - malformed payload matrix across all parsed event types to ensure fail-closed rejection with zero handler dispatch.
- Added rollout and mixed-version fallback guidance in `docs/GATEWAY_EVENTS.md`:
  - deploy/verification checklist
  - fallback behavior requirements for unknown or malformed events in mixed-client fleets.

### Exit Criteria
- Event system has coverage for main mutable state and regression alarms for future changes.

---

## Phase 8 - Voice/Streaming Presence Realtime Sync
### Goal
Keep call participant and stream-state UI consistent across all connected clients without manual refresh.

### Completion Status
`DONE`

### Tasks
- [x] Define/lock voice gateway event contracts in `docs/GATEWAY_EVENTS.md` with bounded payloads:
  - `voice_participant_sync` (channel snapshot)
  - `voice_participant_join`
  - `voice_participant_leave`
  - `voice_participant_update` (mute/deafen/speaking/video/screen-share flags)
  - `voice_stream_publish`
  - `voice_stream_unpublish`
- [x] Ensure event payloads are additive + redacted (no privileged token/session secrets, no raw network metadata).
- [x] Emit voice/stream events from all relevant mutation points:
  - LiveKit token issuance/join path
  - explicit leave/disconnect path
  - server-observed publish/unpublish/state updates (where available)
- [x] Add channel-scoped/guild-scoped fanout wiring so only authorized viewers receive events.
- [x] Add reconnect/resubscribe snapshot behavior:
  - always send `voice_participant_sync` on subscribe/join to repair drift.
- [x] Web gateway/parser updates:
  - strict fail-closed validation for all new voice event payloads
  - reducer/controller updates to reconcile participant list and stream badges incrementally
  - idempotent handling for duplicate/out-of-order join/leave updates
- [x] Add stale participant cleanup policy:
  - bounded timeout/heartbeat-based pruning for orphaned sessions
  - explicit tests for disconnect cleanup and fast rejoin.
- [x] Add observability for voice sync health:
  - emitted/dropped counts for new voice event types
  - drift-repair counter for snapshot resync events.
- [x] Add tests:
  - server integration: multi-client join/leave/publish/unpublish consistency in same channel
  - server negative tests: unauthorized clients do not receive voice participant/stream events
  - web tests: reducer correctness for join/leave/update ordering and duplicate suppression
  - end-to-end network flow test for participant list convergence after reconnect.

### Refactor Notes
- Added typed voice gateway event contracts in `apps/filament-server/src/server/gateway_events.rs` and `docs/GATEWAY_EVENTS.md` for:
  - `voice_participant_sync`
  - `voice_participant_join`
  - `voice_participant_leave`
  - `voice_participant_update`
  - `voice_stream_publish`
  - `voice_stream_unpublish`
- Added bounded server-side voice participant session tracking in `apps/filament-server/src/server/core.rs` and `apps/filament-server/src/server/realtime.rs` with:
  - per-channel and global caps for tracked voice presence state
  - TTL-based stale participant pruning
  - disconnect cleanup that emits leave/unpublish events when a user’s final gateway connection closes
  - subscribe-time `voice_participant_sync` drift repair snapshots
- Wired LiveKit token issuance to voice presence fanout in `apps/filament-server/src/server/handlers/media.rs` so join/update/publish transitions emit realtime events without exposing secrets.
- Added voice sync observability in server metrics:
  - `filament_voice_sync_repairs_total{reason}`
- Extended web gateway boundary parsing and runtime reconciliation:
  - strict fail-closed voice payload parsing in `apps/filament-client-web/src/lib/gateway.ts`
  - incremental/idempotent voice participant reducers in `apps/filament-client-web/src/features/app-shell/controllers/gateway-controller.ts`
  - voice participant snapshot state in `apps/filament-client-web/src/features/app-shell/state/voice-state.ts`
  - selector integration for roster/stream badge rendering in `apps/filament-client-web/src/features/app-shell/selectors/create-app-shell-selectors.ts`
- Added/updated tests:
  - server end-to-end voice sync flow in `apps/filament-server/tests/gateway_network_flow.rs`
  - web gateway parser fail-closed coverage in `apps/filament-client-web/tests/gateway.test.ts`
  - web controller reducer coverage in `apps/filament-client-web/tests/app-shell-gateway-controller.test.ts`

### Exit Criteria
- Participant list + stream indicators converge across all subscribed clients within one event round-trip.
- Reconnects/self-healing snapshot path resolves drift without full page reload.
- Unauthorized clients cannot observe voice participant or stream state.

## Immediate Order Recommendation
1. Phase 0
2. Phase 1
3. Phase 2
4. Phase 6
5. Phase 3
6. Phase 4
7. Phase 5
8. Phase 7
9. Phase 8

Rationale: message and settings UX unblock the current user-facing pain fastest; membership/roles/profile follow once event plumbing is stable.
