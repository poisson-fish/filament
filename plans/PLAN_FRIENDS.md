# PLAN_FRIENDS.md

## Objective
Build a Discord-style friends surface opened by the `F` button in the server rail:
- Left friends rail: view options + recent direct conversations (most recent first).
- Main message rail: either friend list mode or selected direct conversation.
- Friend list mode: searchable, alphabetical list with friend actions.
- Directional target: group DMs plus voice/video calls from the friends surface.

This plan assumes current Filament constraints:
- Security-first, fail-closed parsing.
- No federation.
- Default plaintext behavior; optional E2EE support is phased and explicit.
- No HTML rendering in chat.

## Current State (2026-02-27)
- `F` opens an overlay `FriendshipsPanel` modal.
- Friendships API + gateway events exist (`/friends`, `/friends/requests`, `friend_*` events).
- No DM conversation/message backend model exists yet.
- Gateway ingress only supports guild/channel subscribe + message create.

## Product Scope
- Replace friendships modal workflow with a first-class app surface.
- Keep guild chat unchanged when not in friends mode.
- Add direct messaging between friends.
- Ship 1:1 DM first, then group DM, then calls.
- Keep current friendship request/accept/remove flows, moved into friends views.

## Non-Goals (for this plan)
- Federation.
- Mandatory E2EE by default.
- Remote/embedded HTML content rendering.
- Broad redesign of workspace/server chat flows.

## UX Behavior Contract
1. `F` in `ServerRail` switches the shell into `friends` surface mode (not modal).
2. Friends rail (left middle rail) contains:
- `Find or start a conversation` search.
- View options (minimum): `Friends`, `Pending`, `Add Friend`.
- Recent DMs sorted by `last_message_at_unix DESC`.
3. Main rail behavior:
- `Friends` view: searchable alphabetical friend list.
- `Pending` view: incoming/outgoing requests.
- `Add Friend` view: add-by-user-id form.
- DM selected: full message list + composer (same safety/rendering rules as guild chat).
4. Existing guild/channel rails and member rail remain unchanged outside friends mode.

## Security and Trust Model Notes (Centralized Server)
### Reality to document clearly
- In this architecture, server operators can observe DM and guild message contents and metadata.
- Default mode is plaintext, so operator-readable content is expected unless encryption is explicitly enabled.

### What we should do in this phase
- Add explicit UI/docs disclosure: DMs are private from other users, not from the hosting server operator.
- Keep strict hostile-server client posture:
  - fail-closed DTO parsing for all DM/friends payloads
  - bound list sizes, string lengths, and payload sizes
  - ignore/reject malformed/unknown DM gateway payloads
- Add observability for DM event rejects/drops and rate-limit triggers.
- Prepare mixed plaintext/E2EE capability fields so encrypted state is explicit and never inferred.

### Centralized-command ideas worth discussing (future)
- Server policy manifest endpoint (`/server/policy`) shown in client settings.
- Optional DM access transparency logs (admin access audit events) surfaced to users.
- Per-server trust badges/warnings in UI based on configured policy disclosures.

## Data and Domain Model (Proposed)
Add dedicated DM domain types (no stringly typed IDs):
- `DmConversationId`
- `DmMessageId`
- `DmParticipantRole`
- `DmConversationKind = "direct" | "group"`
- `DmConversationCryptoMode = "plaintext" | "e2ee_v1"`

Prefer separate DM persistence (clear invariants) over overloading guild channel tables:
- `dm_conversations (conversation_id, created_at_unix, updated_at_unix, last_message_at_unix)`
- `dm_participants (conversation_id, user_id, joined_at_unix, last_read_at_unix, role nullable)`
- `dm_messages (message_id, conversation_id, author_user_id, crypto_mode, content_or_ciphertext, markdown_tokens nullable, created_at_unix, updated_at_unix, deleted_at_unix nullable, key_epoch nullable, crypto_suite nullable)`

Indexes/caps:
- participant lookup by user (`(user_id, last_message_at_unix DESC)` via join path)
- message pagination (`(conversation_id, created_at_unix DESC, message_id DESC)`)
- hard cap DM participants per conversation.

Decision:
- Use dedicated DM tables (not nullable guild/message overloading) for auth safety and invariant clarity.

## API Contract Additions (Proposed)
REST:
- `GET /dm/conversations?cursor=<id>&limit=<n>`
- `POST /dm/conversations` (1:1 and group create with bounded participant sets; idempotent for 1:1)
- `GET /dm/conversations/{conversation_id}/messages?before=<id>&limit=<n>`
- `POST /dm/conversations/{conversation_id}/messages`
- `GET /dm/conversations/{conversation_id}/permissions/self` (optional, for parity/guards)
- `POST /dm/conversations/{conversation_id}/calls` (voice/video session start request; policy-gated)

Gateway:
- Ingress:
  - `dm_subscribe` `{ conversation_id }`
  - `dm_message_create` `{ conversation_id, content, attachment_ids? }`
- Egress:
  - `dm_subscribed`
  - `dm_message_create`
  - `dm_message_update`
  - `dm_message_delete`
  - `dm_conversation_upsert` (recents ordering/unread metadata)
  - `dm_conversation_remove`
  - `dm_call_state` (call metadata and participant state; no sensitive media secrets)

Message/file encryption markers (for mixed plaintext/E2EE rollout):
- `crypto_mode`
- `crypto_suite` (when encrypted)
- `key_epoch` (when encrypted)

All new events must be added to:
- `docs/GATEWAY_EVENTS.md`
- `crates/filament-protocol/.../gateway_events_manifest.json`
- server/web contract tests for manifest parity.

## Abuse/DoS Controls (Must-Have)
- DM route-specific rate limits (per-IP and per-user).
- Reuse global body/message size limits (`<= 64 KiB` gateway event payloads).
- Bounded DM fanout queue behavior consistent with existing gateway rules.
- DM message content length parity with guild message rules.
- Pagination limits and search-input limits for friends list search and DM lookup.
- Group DM participant caps and per-conversation creation throttles.
- Call setup/join rate limits per user/conversation/IP.

## Web Client Architecture Plan
### State
- Add top-level surface state: `activeSurface = "guild" | "friends"`.
- Add friends-surface state:
  - `activeFriendsView = "friends" | "pending" | "add-friend" | "dm"`
  - `activeConversationId`
  - `friendSearchQuery`
  - `dmRecents`
  - `dmCallState`

### Components
- Keep `ServerRail` `F` button, but route to surface state switch.
- Add `FriendsRail` component (reuses shell rail style tokens).
- Add `FriendsMainView` container:
  - `FriendsListView` (alphabetical + searchable)
  - `PendingRequestsView`
  - `AddFriendView`
  - `DirectMessageView` (message list/composer)
  - `DmCallControls` (voice/video controls in DM context)

### Reuse
- Reuse existing message rendering/composer primitives where possible.
- Keep safe markdown token rendering path unchanged.

## Rollout Phases
### Phase 1: Friends Surface Refactor (No DM backend yet)
- Convert modal friendships panel into first-class friends surface.
- Implement view options + searchable alphabetical friend list.
- Move existing pending/add/request actions into new views.
- Keep DM recents rail placeholders (no conversation open yet).

Exit criteria:
- `F` opens friends surface in-place.
- Friend list search + alphabetical sort deterministic and tested.
- Existing friendship flows remain functional.

### Phase 2: DM Backend + REST
- Add DM schema, domain types, and invariant constructors (1:1 + group-ready).
- Implement DM conversation list/create/message list/send endpoints (1:1 launch path).
- Add per-route rate limits and payload bounds.
- Add integration tests for authorization/visibility and pagination.

Exit criteria:
- Users can create/open 1:1 DM and send/list messages.
- Non-participants cannot read/write/subscribe.
- API docs updated.

### Phase 3: Group DMs + DM Gateway Realtime
- Enable bounded group DM creation and participant membership operations.
- Add DM ingress/egress events and strict decoder/dispatch in web client.
- Wire recents ordering updates on new messages.
- Add metrics counters and drop/parse-reject observability for DM scopes.

Exit criteria:
- Realtime DM delivery works across multi-session tests.
- Invalid DM envelopes fail closed with no local mutation.
- Group DM membership authorization and fanout behavior are covered by tests.

### Phase 4: DM Calls (Voice/Video)
- Add DM call session wiring through existing media token/LiveKit policy path.
- Add call state events and bounded participant state synchronization.
- Keep media permission scope least-privilege; no remote script/navigation paths.

Exit criteria:
- Friends/group DM calls connect reliably with policy-checked token issuance.
- Call-state realtime updates are stable and bounded.

### Phase 5: Optional E2EE Markers in Friends Surface
- Add explicit encryption badges/fields for mixed plaintext vs encrypted messages/files.
- Block ambiguous encryption rendering (fail closed when crypto fields are malformed).

Exit criteria:
- Users can clearly see plaintext vs encrypted state in DM/group DM contexts.
- Unsupported/invalid crypto payloads do not mutate local state.

### Phase 6: Hardening + UX Polish
- Add unread indicators, empty states, and keyboard navigation.
- Add trust disclosure copy for centralized server observability.
- Validate mobile/desktop responsive behavior.

Exit criteria:
- UX matches agreed baseline and accessibility checks pass.
- Threat model/docs include DM observability language.

## Test Plan
Server:
- Unit tests for DM newtype invariants and DTO conversions.
- Integration tests:
  - create/list/send DM happy path
  - non-participant access denied
  - pagination bounds
  - gateway dm_subscribe + dm_message_create flow
  - oversized/invalid DM gateway payload rejection

Web:
- Component tests for `FriendsRail` and friends view switching.
- Controller tests for friends-surface state transitions.
- API client tests for DM DTO fail-closed behavior.
- Gateway parser/dispatch tests for new DM event contracts.
- App-shell integration tests for full `F -> friends -> DM` flow.

## Open Decisions (Need Maintainer Direction)
1. Group DM participant cap:
- Option A: `10` max (recommended initial safety cap).
- Option B: `25` max (higher fanout/abuse surface).
2. Trust disclosure UX:
- Option A: always-visible note in friends/DM surface.
- Option B: settings-only disclosure and onboarding copy.
3. Friends-surface call launch:
- Option A: 1:1 voice/video first, group call shortly after.
- Option B: launch both 1:1 and group call together.

Locked decision:
- Friends-only DM for v1 (message requests for non-friends are deferred).

## Immediate Next Slice (Recommended)
- Ship Phase 1 first (friends surface + searchable alphabetical list + pending/add views) without backend DM changes.
- In parallel, draft DM/group/call API and gateway contracts before implementing Phase 2+.
