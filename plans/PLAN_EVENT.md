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
| `PATCH /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}` | `message_update` | channel | Planned (Phase 2) |
| `DELETE /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}` | `message_delete` | channel | Planned (Phase 2) |
| `POST/DELETE /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}/reactions/{emoji}` | `message_reaction` | channel | Implemented |
| `POST /guilds/{guild_id}/channels` | `channel_create` | guild | Implemented |
| `PATCH /guilds/{guild_id}/channels/{channel_id}` | `channel_update` | guild | Planned (future endpoint) |
| `DELETE /guilds/{guild_id}/channels/{channel_id}` | `channel_delete` | guild | Planned (future endpoint) |
| `PATCH /guilds/{guild_id}` | `workspace_update` | guild | Planned (Phase 3; endpoint not implemented yet) |
| `POST /guilds/{guild_id}/join` | `workspace_member_add` | guild | Planned (Phase 3) |
| `POST /guilds/{guild_id}/members/{user_id}` | `workspace_member_add` | guild | Planned (Phase 3) |
| `PATCH /guilds/{guild_id}/members/{user_id}` | `workspace_member_update` | guild | Planned (Phase 3) |
| `POST /guilds/{guild_id}/members/{user_id}/kick` | `workspace_member_remove` | guild | Planned (Phase 3) |
| `POST /guilds/{guild_id}/members/{user_id}/ban` | `workspace_member_ban`, `workspace_member_remove` | guild | Planned (Phase 3) |
| `POST /guilds/{guild_id}/roles` | `workspace_role_create` | guild | Planned (Phase 4) |
| `PATCH /guilds/{guild_id}/roles/{role_id}` | `workspace_role_update` | guild | Planned (Phase 4) |
| `DELETE /guilds/{guild_id}/roles/{role_id}` | `workspace_role_delete` | guild | Planned (Phase 4) |
| `POST /guilds/{guild_id}/roles/reorder` | `workspace_role_reorder` | guild | Planned (Phase 4) |
| `POST /guilds/{guild_id}/roles/{role_id}/members/{user_id}` | `workspace_role_assignment_add` | guild | Planned (Phase 4) |
| `DELETE /guilds/{guild_id}/roles/{role_id}/members/{user_id}` | `workspace_role_assignment_remove` | guild | Planned (Phase 4) |
| `POST /guilds/{guild_id}/channels/{channel_id}/overrides/{role}` | `workspace_channel_override_update` | guild | Planned (Phase 4) |
| `GET/POST/DELETE /guilds/{guild_id}/ip-bans...` | `workspace_ip_ban_sync` | guild | Planned (Phase 4, redacted payload only) |
| `PATCH /users/me/profile` | `profile_update` | user | Planned (Phase 5) |
| `POST /users/me/profile/avatar` | `profile_avatar_update` | user | Planned (Phase 5) |
| `POST /friends/requests` | `friend_request_create` | user | Planned (Phase 5) |
| `POST /friends/requests/{request_id}/accept` | `friend_request_update` | user | Planned (Phase 5) |
| `DELETE /friends/requests/{request_id}` | `friend_request_delete` | user | Planned (Phase 5) |
| `DELETE /friends/{friend_user_id}` | `friend_remove` | user | Planned (Phase 5) |

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
`NOT STARTED`

### Tasks
- [ ] Emit `message_update` from message edit endpoint.
- [ ] Emit `message_delete` from message delete endpoint.
- [ ] Keep `message_reaction` event behavior, add tests for zero-count delete semantics.
- [ ] Update web gateway parser/controller/state reducers for new message events.
- [ ] Add server + web tests for edit/delete realtime sync across multiple clients.

### Exit Criteria
- Editing/deleting a message on one client updates other subscribed clients without refresh.

---

## Phase 3 - Workspace + Membership Events
### Goal
Sync workspace-level structural and membership state.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add workspace update API endpoint(s) for rename/visibility (if missing) with strict validation.
- [ ] Emit `workspace_update` for server name/settings updates.
- [ ] Emit membership events for join/add/role/kick/ban flows.
- [ ] Emit `workspace_member_remove` on kick and ban; ensure active-channel safety behavior on web.
- [ ] Web reducers update workspace list/member rails/permission-sensitive views in-place.
- [ ] Add integration tests for multi-client workspace rename and membership transitions.

### Exit Criteria
- Workspace name/settings and membership changes propagate in realtime.

---

## Phase 4 - Roles, Overrides, Moderation Events
### Goal
Realtime sync for permission graph changes that alter visible capabilities.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Emit role lifecycle events (`workspace_role_create|update|delete|reorder`).
- [ ] Emit role assignment events per member.
- [ ] Emit channel override update events.
- [ ] Emit redacted IP-ban sync event for moderation views (no raw IP fields).
- [ ] Web applies permission-impacting events by refreshing snapshots where needed and pruning inaccessible channels.
- [ ] Add tests proving no privileged data leakage in event payloads.

### Exit Criteria
- Permission and moderation UI reflects role/override/ban changes without manual reload.

---

## Phase 5 - Profile + Friendship User-Scoped Events
### Goal
Keep client profile/social panels synchronized across sessions/devices.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Emit `profile_update` and `profile_avatar_update` to the acting user and relevant observers where permitted.
- [ ] Emit friendship request/friendship state events as user-scoped updates.
- [ ] Add web handlers to update profile cache, avatar versions, and friendship panel state incrementally.
- [ ] Add tests for multi-session profile update propagation and friend request lifecycle sync.

### Exit Criteria
- Profile and friendship state stays fresh across concurrent sessions.

---

## Phase 6 - Settings UX Split (Client vs Workspace)
### Goal
Fix settings information architecture and restore expected entry points.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add separate overlay panels/types:
  - `client-settings` (existing Voice/Profile content)
  - `workspace-settings` (workspace/server config)
- [ ] Restore gear icon button in channel rail account footer to open `client-settings`.
- [ ] Keep workspace dropdown “Server Settings” but wire it to `workspace-settings`.
- [ ] Move workspace-specific controls out of client settings panel.
- [ ] Add/update tests:
  - gear opens client settings
  - workspace menu opens workspace settings
  - panel titles/authorization behavior remain correct

### Exit Criteria
- Client and workspace settings are clearly separated and reachable from correct entry points.

---

## Phase 7 - Hardening, Rollout, and Regression Net
### Goal
Finalize stability and prevent event regressions.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add contract tests for all gateway event payload validators (server + web).
- [ ] Add end-to-end multi-client sync tests for key flows (message, workspace rename, role changes, profile).
- [ ] Add event observability counters (`emitted`, `dropped`, `unknown_received`, `parse_rejected`).
- [ ] Add rollout checklist and fallback behavior notes for mixed-version clients.
- [ ] Update this file with implementation notes after each completed phase.

### Exit Criteria
- Event system has coverage for main mutable state and regression alarms for future changes.

## Immediate Order Recommendation
1. Phase 0
2. Phase 1
3. Phase 2
4. Phase 6
5. Phase 3
6. Phase 4
7. Phase 5
8. Phase 7

Rationale: message and settings UX unblock the current user-facing pain fastest; membership/roles/profile follow once event plumbing is stable.
