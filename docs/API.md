# Filament Server API

This document describes the currently implemented API surface in `apps/filament-server`.

## Scope and Version
- Transport:
  - REST over HTTP
  - Gateway over WebSocket (`/gateway/ws`)
- Gateway protocol version: `1`
- This reflects the implementation in `apps/filament-server/src/lib.rs` and related tests.

## Base Conventions
- IDs are ULID strings.
- JSON request bodies for most endpoints use strict decoding (`deny_unknown_fields`), so unknown fields are rejected.
- Authenticated routes require `Authorization: Bearer <access_token>` unless stated otherwise.
- Timestamps are Unix seconds (`*_unix`).

## Authentication Model
- Access token:
  - PASETO local token
  - TTL: `900` seconds (15 minutes)
- Refresh token:
  - Opaque format: `<session_id>.<secret>`
  - Rotation on every refresh
  - Replay detection revokes the session
- Password policy:
  - Length `12..=128`
- Username policy:
  - Length `3..=32`
  - Allowed chars: ASCII alphanumeric, `_`, `.`

## Error Model
Application errors return JSON:

```json
{ "error": "<code>" }
```

Common codes:
- `invalid_request` -> `400`
- `invalid_credentials` -> `401`
- `forbidden` -> `403`
- `not_found` -> `404`
- `rate_limited` -> `429`
- `payload_too_large` -> `413`
- `quota_exceeded` -> `409`
- `e2ee_capability_required` -> `409`
- `e2ee_conversation_conflict` -> `409`
- `epoch_conflict` -> `409`
- `internal_error` -> `500`

Global middleware can also return non-handler errors such as `408 Request Timeout` and baseline `429` rate limit responses.

## Security and Limits (defaults)
- Global JSON body limit: `1 MiB`
- Request timeout: `10s`
- Baseline IP rate limit: `600 req/min`
- Auth route rate limit (`register/login/refresh`): `60 req/min` per route+IP
- Gateway max event size: `64 KiB`
- Gateway ingress limit: `60 events / 10s / connection`
- Gateway outbound queue: `256` events/connection
- Message content length: `1..=2000`
- History pagination max `limit`: `100`
- Search defaults:
  - query max chars: `256`
  - default limit: `20`
  - max limit: `50`
  - max terms: `20`
  - max wildcards (`*` + `?`): `4`
  - max fuzzy marker (`~`): `2`
  - `:` disallowed in query
- Attachment upload max: `25 MiB`
- Per-user attachment quota: `250 MiB`
- Attachment filename: non-empty, max `128`, no `/`, `\\`, or `NUL`
- Reaction emoji path segment: non-empty, max `32` chars, no whitespace
- LiveKit token TTL: max/default `300s`

## Directory Moderation Contract (Phase 0 design lock)
This section locks response semantics and limits for upcoming directory-join/audit/IP-ban endpoints.

### Locked policy semantics
- `POST /guilds/{guild_id}/join`:
  - Public + eligible: `200` with typed join outcome.
  - Private or nonexistent guild ID: `404 {"error":"not_found"}` (no visibility oracle).
  - User-level guild ban: `403 {"error":"directory_join_user_banned"}`.
  - Guild IP-ban hit: `403 {"error":"directory_join_ip_banned"}`.
  - Join not permitted by visibility/policy: `403 {"error":"directory_join_not_allowed"}`.
  - Rate-limited: `429 {"error":"rate_limited"}`.
- `GET /guilds/{guild_id}/audit`:
  - Authorized owner/moderator: `200` typed redacted page payload.
  - Non-member or unauthorized member: `403 {"error":"audit_access_denied"}`.
  - Unknown guild: `404 {"error":"not_found"}`.
- `GET /guilds/{guild_id}/ip-bans`, `POST /guilds/{guild_id}/ip-bans/by-user`,
  `DELETE /guilds/{guild_id}/ip-bans/{ban_id}`:
  - owner/moderator only; unauthorized callers receive `403 {"error":"forbidden"}`.
  - list/create/delete payloads never include raw `ip`/`cidr` fields.

### Locked per-route limits (default contracts)
- `POST /guilds/{guild_id}/join`:
  - `60 req/min` per client IP
  - `30 req/min` per authenticated user
- `GET /guilds/{guild_id}/audit`:
  - `limit` default `20`, max `100`
  - `action_prefix` max `64` chars, charset `[a-z0-9._]`
  - `cursor` max `128` chars, charset `[A-Za-z0-9_-]`
- `GET /guilds/{guild_id}/ip-bans`:
  - `limit` default `20`, max `100`
  - `cursor` max `128` chars, charset `[A-Za-z0-9_-]`
- `POST /guilds/{guild_id}/ip-bans/by-user`:
  - `reason` max `240` chars
  - `expires_in_secs` max `15_552_000` (180 days)
  - guild IP-ban total entries cap default `4_096`

## REST API

### Public Utility
- `GET /health`
  - Response `200`: `{ "status": "ok" }`
- `GET /metrics`
  - Response `200`: Prometheus text format
- `POST /echo`
  - Request: `{ "message": "..." }`
  - Empty message -> `400`
  - Response `200`: `{ "message": "..." }`
- `GET /slow`
  - Test route for timeout behavior

### Auth
- `POST /auth/register`
  - Request: `{ "username": "...", "password": "...", "captcha_token"?: "..." }`
  - If hCaptcha is enabled on the server (`FILAMENT_HCAPTCHA_SITE_KEY` + `FILAMENT_HCAPTCHA_SECRET`):
    - `captcha_token` is required
    - token must be visible ASCII and `20..=4096` chars
    - verification uses hCaptcha `siteverify` and fails closed on verification/network errors
    - invalid/failed verification returns `403 {"error":"captcha_failed"}`
  - Always returns accepted shape for valid input (existing/new user not disclosed)
  - Response `200`: `{ "accepted": true }`
- `POST /auth/login`
  - Request: `{ "username": "...", "password": "..." }`
  - On success `200`:
    - `{ "access_token": "...", "refresh_token": "...", "expires_in_secs": 900 }`
  - Invalid credentials/locked account -> `401 {"error":"invalid_credentials"}`
- `POST /auth/refresh`
  - Request: `{ "refresh_token": "..." }`
  - Success `200`: same shape as login
  - Replay/invalid/revoked/expired -> `401`
- `POST /auth/logout`
  - Request: `{ "refresh_token": "..." }`
  - Success `204 No Content`
- `GET /auth/me`
  - Auth required
  - Response `200`:
    - `{ "user_id": "...", "username": "...", "about_markdown": "...", "about_markdown_tokens": [...], "avatar_version": <number>, "banner_version": <number> }`
- `POST /users/lookup`
  - Auth required
  - Request: `{ "user_ids": ["..."] }`
  - `user_ids`: deduplicated server-side, `1..=64` ULID values
  - Response `200`:
    - `{ "users": [{ "user_id": "...", "username": "..." }] }`
  - Missing users are omitted from `users`

### Profile
- `PATCH /users/me/profile`
  - Auth required
  - Request: `{ "username"?: "...", "about_markdown"?: "..." }`
  - `about_markdown` max length `2048` chars
  - Response `200`: `{ "user_id": "...", "username": "...", "about_markdown": "...", "about_markdown_tokens": [...], "avatar_version": <number>, "banner_version": <number> }`
- `GET /users/{user_id}/profile`
  - Auth required
  - Response `200`: same shape as profile update/read model
- `GET /users/{user_id}/avatar`
  - Auth required
  - Response `200`: raw avatar bytes with image content type
- `POST /users/me/profile/avatar`
  - Auth required
  - Raw binary body upload (not multipart)
  - MIME is sniffed from bytes; unsupported or mismatched image type is rejected
  - Response `200`: profile shape including `avatar_version` and `banner_version`

#### Profile Banner
- `banner_version` is present in profile responses (`/auth/me`, `/users/me/profile`, `/users/{user_id}/profile`) and increments on successful banner upload.
- `POST /users/me/profile/banner`
  - Auth required
  - Raw binary body upload (not multipart)
  - Size cap: `6 MiB` hard limit
  - MIME allowlist (sniffed via `infer`; declared type must match when present):
    - `image/jpeg`
    - `image/png`
    - `image/webp`
    - `image/avif`
    - `image/gif`
  - Response `200`: profile shape including `banner_version`
- `GET /users/{user_id}/banner`
  - Auth required
  - Response `200`: raw banner bytes with image content type, `nosniff`, and cache headers

### Friendships
- `GET /friends`
  - Auth required
  - Response `200`:
    - `{ "friends": [{ "user_id": "...", "username": "...", "created_at_unix": 123 }] }`
- `POST /friends/requests`
  - Auth required
  - Request: `{ "recipient_user_id": "..." }`
  - Rejects self-targeting, duplicates, existing friendships, and unknown users
  - Response `200`:
    - `{ "request_id": "...", "sender_user_id": "...", "recipient_user_id": "...", "created_at_unix": 123 }`
- `GET /friends/requests`
  - Auth required
  - Permission-safe exposure: only caller-visible incoming/outgoing requests are returned
  - Response `200`:
    - `{ "incoming": [FriendRequest], "outgoing": [FriendRequest] }`
  - `FriendRequest`:
    - `{ "request_id": "...", "sender_user_id": "...", "sender_username": "...", "recipient_user_id": "...", "recipient_username": "...", "created_at_unix": 123 }`
- `POST /friends/requests/{request_id}/accept`
  - Auth required
  - Only the request recipient may accept
  - Response `200`: `{ "accepted": true }`
- `DELETE /friends/requests/{request_id}`
  - Auth required
  - Sender or recipient may delete/cancel
  - Response `204 No Content`
- `DELETE /friends/{friend_user_id}`
  - Auth required
  - Removes an existing friendship pair (idempotent)
  - Response `204 No Content`

### Guilds and Channels
- `POST /guilds`
  - Auth required
  - Request: `{ "name": "...", "visibility"?: "private"|"public" }` (`visibility` defaults to `private`)
  - `name`: 1..64 visible chars/spaces
  - Enforces per-user creator cap configured by server (`FILAMENT_MAX_CREATED_GUILDS_PER_USER`)
  - Response `200`: `{ "guild_id": "...", "name": "...", "visibility": "private"|"public" }`
  - When limit is reached: `403 {"error":"guild_creation_limit_reached"}`
- `GET /guilds`
  - Auth required
  - Returns only guilds where requester is an active member (banned guilds are excluded)
  - Response `200`:
    - `{ "guilds": [{ "guild_id": "...", "name": "...", "visibility": "private"|"public" }] }`
- `PATCH /guilds/{guild_id}`
  - Auth required
  - Requires effective `manage_roles` permission in the workspace
  - Request: `{ "name"?: "...", "visibility"?: "private"|"public" }`
  - At least one field is required
  - Response `200`: `{ "guild_id": "...", "name": "...", "visibility": "private"|"public" }`
- `GET /guilds/public?q=<query>&limit=<n>`
  - Auth required
  - Returns only guilds marked `public`
  - `q` optional, case-insensitive substring on guild name, max `64` chars
  - `limit` default `20`, max `50`
  - Response `200`:
    - `{ "guilds": [{ "guild_id": "...", "name": "...", "visibility": "public" }] }`
- `POST /guilds/{guild_id}/channels`
  - Auth required; role must be `owner` or `moderator`
  - Request: `{ "name": "...", "kind"?: "text"|"voice" }` (`kind` defaults to `text`)
  - `name`: 1..64 visible chars/spaces
  - Response `200`: `{ "channel_id": "...", "name": "...", "kind": "text"|"voice" }`
- `GET /guilds/{guild_id}/channels`
  - Auth required; requester must be a guild member
  - Returns channels in that guild where requester has effective `create_message` permission
  - Response `200`:
    - `{ "channels": [{ "channel_id": "...", "name": "...", "kind": "text"|"voice" }] }`
- `GET /guilds/{guild_id}/channels/{channel_id}/permissions/self`
  - Auth required
  - Least-visibility gate: requires effective `create_message` permission in the channel
  - Response `200`:
    - `{ "role": "owner|moderator|member", "permissions": [Permission...] }`
- `GET /guilds/{guild_id}/roles`
  - Auth required; requester must be a guild member
  - Response `200`:
    - `{ "roles": [{ "role_id": "...", "name": "...", "permissions": [Permission...], "priority": <number>, "is_system": <bool> }] }`
- `POST /guilds/{guild_id}/roles`
  - Auth required; requires `manage_roles`
  - Request: `{ "name": "...", "permissions": [Permission...] }`
  - Response `200`: `{ "role_id": "...", "name": "...", "permissions": [Permission...], "priority": <number>, "is_system": false }`
- `POST /guilds/{guild_id}/roles/reorder`
  - Auth required; requires `manage_roles`
  - Request: `{ "role_ids": ["<role_id>", ...] }`
  - Response `200`: `{ "accepted": true }`
- `POST /guilds/{guild_id}/roles/default`
  - Auth required; requires `manage_roles`
  - Request: `{ "role_id": "<role_id>" | null }`
  - `role_id: null` clears the default join role assignment
  - Response `200`: `{ "accepted": true }`
- `PATCH /guilds/{guild_id}/roles/{role_id}`
  - Auth required; requires `manage_roles`
  - Request: `{ "name"?: "...", "permissions"?: [Permission...] }`
  - Response `200`: updated role payload
- `DELETE /guilds/{guild_id}/roles/{role_id}`
  - Auth required; requires `manage_roles`
  - Response `204 No Content`
- `POST /guilds/{guild_id}/roles/{role_id}/members/{user_id}`
  - Auth required; requires `manage_roles`
  - Response `200`: `{ "accepted": true }`
- `DELETE /guilds/{guild_id}/roles/{role_id}/members/{user_id}`
  - Auth required; requires `manage_roles`
  - Response `200`: `{ "accepted": true }`

### Messages
- `POST /guilds/{guild_id}/channels/{channel_id}/messages`
  - Auth required, `create_message` permission
  - Request: `{ "content": "...", "attachment_ids": ["<attachment_id>", ...] }`
  - `content` may be empty only when `attachment_ids` is non-empty
  - `attachment_ids` optional, max `5`, deduped server-side
  - each attachment must belong to requester, match guild/channel, and be unclaimed
  - Response `200`:
    - `{ "message_id", "guild_id", "channel_id", "author_id", "content", "markdown_tokens", "attachments", "created_at_unix" }`
- `GET /guilds/{guild_id}/channels/{channel_id}/messages?limit=<n>&before=<message_id>`
  - Auth required, `create_message` permission
  - `limit` default `20`, max `100`
  - Response `200`:
    - `{ "messages": [MessageResponse], "next_before": "..." | null }`
- `PATCH /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}`
  - Auth required
  - Author may edit own message; moderators/owners can edit via `delete_message` permission
  - Request: `{ "content": "..." }`
  - Response `200`: `MessageResponse`
- `DELETE /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}`
  - Auth required
  - Author may delete own message; moderators/owners can delete via `delete_message` permission
  - Response `204`

#### `MessageResponse` and markdown tokens
`markdown_tokens` is a safe token stream (no raw HTML rendering path). Token variants include:
- `paragraph_start`, `paragraph_end`
- `emphasis_start`, `emphasis_end`
- `strong_start`, `strong_end`
- `list_start { ordered }`, `list_end`
- `list_item_start`, `list_item_end`
- `link_start { href }`, `link_end` (only `http`, `https`, `mailto` links survive sanitization)
  - scheme checks are case-insensitive and trim surrounding whitespace before validation
  - disallowed/obfuscated schemes (for example `javascript:` or `data:` with mixed casing) are dropped
- `text { text }`
- `code { code }`
- `fenced_code { language, code }`
  - `language`: optional; when present, lowercased and restricted to `[A-Za-z0-9_.+-]{1,32}`
  - max `64` fenced code tokens per markdown payload
  - max `16384` chars per fenced code `code` field
- `soft_break`, `hard_break`

#### Fenced Code Highlighting Contract (Locked Pre-Deploy)
- Rendering uses an AST/token highlighter pipeline only (`lowlight` + explicitly registered `highlight.js` grammars).
- No highlighter HTML string output may be injected into the DOM.
- Language labels are allowlisted and bounded; unknown/invalid labels degrade to plain-text fenced code rendering.

`attachments` contains zero or more attachment records linked to this message.
`reactions` contains bounded reaction snapshots:
- `emoji`: reaction identifier
- `count`: non-negative aggregate count
- `reacted_by_me`: whether the authenticated caller has reacted with this emoji
- `reactor_user_ids`: bounded user-id sample for future reaction-member UI (max `32` ids per emoji)
- per-message reaction snapshot entries are capped at `64`

### Reactions
- `POST /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}/reactions/{emoji}`
- `DELETE /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}/reactions/{emoji}`
  - Auth required, channel write permission
  - Response `200`: `{ "emoji": "...", "count": <number>, "reacted_by_me": <boolean>, "reactor_user_ids": [<user_id>...] }`

### Attachments
- `POST /guilds/{guild_id}/channels/{channel_id}/attachments?filename=<name>`
  - Auth required, channel write permission
  - Raw binary body upload (not multipart)
  - MIME is sniffed from bytes (`infer`); if `Content-Type` is provided it must match sniffed type
  - Response `200`:
    - `{ "attachment_id", "guild_id", "channel_id", "owner_id", "filename", "mime_type", "size_bytes", "sha256_hex" }`
- `GET /guilds/{guild_id}/channels/{channel_id}/attachments/{attachment_id}`
  - Auth required, channel write permission
  - Response `200`: raw bytes with `Content-Type: <mime_type>`
- `DELETE /guilds/{guild_id}/channels/{channel_id}/attachments/{attachment_id}`
  - Auth required
  - Allowed for owner or users with `delete_message` permission
  - Response `204`

### Search
- `GET /guilds/{guild_id}/search?q=<query>&limit=<n>&channel_id=<channel_id>`
  - Auth required, member with `create_message` permission
  - Response `200`:
    - `{ "message_ids": ["..."], "messages": [MessageResponse] }`
- `POST /guilds/{guild_id}/search/rebuild`
  - Auth required; `owner`/`moderator`
  - Rebuilds Tantivy index from source-of-truth messages
  - Response `204`
- `POST /guilds/{guild_id}/search/reconcile`
  - Auth required; `owner`/`moderator`
  - Reconciles missing/orphaned docs (bounded)
  - Response `200`: `{ "upserted": <number>, "deleted": <number> }`

### Membership and Moderation
- `GET /guilds/{guild_id}/members?cursor=<user_id>&limit=<n>`
  - Auth required
  - Requester must be a guild member
  - Response `200`:
    - `{ "members": [{ "user_id", "role_ids" }], "next_cursor": "..." | null }`
- `POST /guilds/{guild_id}/members/{user_id}`
  - Add member as `member`
  - Requires `manage_roles`
  - Response `200`: `{ "accepted": true }`
- `PATCH /guilds/{guild_id}/members/{user_id}`
  - Request: `{ "role": "owner|moderator|member" }`
  - Role transition rules are enforced (`can_assign_role`)
  - Response `200`: `{ "accepted": true }`
- `POST /guilds/{guild_id}/members/{user_id}/kick`
  - Requires moderation privileges (`ban_member` + hierarchy)
  - Response `200`: `{ "accepted": true }`
- `POST /guilds/{guild_id}/members/{user_id}/ban`
  - Requires moderation privileges (`ban_member` + hierarchy)
  - Response `200`: `{ "accepted": true }`

### Channel Role Overrides
- `POST /guilds/{guild_id}/channels/{channel_id}/overrides/{role}`
  - `role` path: `owner|moderator|member`
  - Request:
    - `{ "allow": [Permission...], "deny": [Permission...] }`
  - `allow` and `deny` cannot overlap
  - Requires `manage_channel_overrides`
  - Response `200`: `{ "accepted": true }`
- `POST /guilds/{guild_id}/channels/{channel_id}/permission-overrides/{target_kind}/{target_id}`
  - `target_kind` path: `0` (role), `1` (member)
  - Request:
    - `{ "allow": [Permission...], "deny": [Permission...] }`
  - `allow` and `deny` cannot overlap
  - Requires `manage_channel_overrides`
  - Response `200`: `{ "accepted": true }`

Permission enum values:
- `manage_roles`
- `manage_channel_overrides`
- `delete_message`
- `ban_member`
- `create_message`
- `publish_video`
- `publish_screen_share`
- `subscribe_streams`

### LiveKit Voice/Video Token
- `POST /guilds/{guild_id}/channels/{channel_id}/voice/token`
  - Auth required
  - Request:
    - `{ "can_publish"?: bool, "can_subscribe"?: bool, "publish_sources"?: ["microphone"|"camera"|"screen_share"] }`
  - Effective grants are clamped by channel permissions and abuse controls:
    - token request rate limit
    - publish rate limit (camera/screen share)
    - subscribe active-token cap per user/channel
  - Response `200`:
    - `{ "token", "livekit_url", "room", "identity", "can_publish", "can_subscribe", "publish_sources", "expires_in_secs" }`

### Voice Leave
- `POST /guilds/{guild_id}/channels/{channel_id}/voice/leave`
  - Auth required
  - Request: none
  - Response `200`: `{ "accepted": true }`

### Voice Participant State
- `POST /guilds/{guild_id}/channels/{channel_id}/voice/state`
  - Auth required
  - Request:
    - `{ "is_muted"?: bool, "is_deafened"?: bool }`
  - At least one field is required.
  - Updates the caller's voice participant mute/deafen state and broadcasts `voice_participant_update` for changed fields.
  - Response `204 No Content`

### RTC Client UX Behavior (Web)
- Voice controls are only shown for channels with `kind: "voice"` and effective `create_message` access.
- Web client call states are surfaced as `connecting`, `connected`, `reconnecting`, and `error`.
- Troubleshooting states are explicit:
  - token/session expiry (`invalid_credentials`) prompts refresh/login before rejoin
  - permission rejection (`forbidden`) reports channel permission/override denial
  - signaling/connect failures prompt verification of `livekit_url` reachability from the browser
- Camera/screen controls remain capability-based on top of voice (`publish_video`, `publish_screen_share`); no separate video-channel mode exists.

## Gateway WebSocket API

### Connect
- Endpoint: `GET /gateway/ws`
- Auth methods:
  - Query param: `?access_token=<token>`
  - Or bearer header
- On successful upgrade, server sends:
  - `{"v":1,"t":"ready","d":{"user_id":"..."}}`

### Envelope
All client and server events use:

```json
{ "v": 1, "t": "event_type", "d": { ... } }
```

Rules:
- `v` must be `1`
- `t` charset: `a-z`, `0-9`, `_`, `.`; max len `64`
- max event payload size `64 KiB`

### Client -> Server events
- `subscribe`
  - `d`: `{ "guild_id": "...", "channel_id": "..." }`
  - Subscribes connection to channel broadcast + presence scope
- `message_create`
  - `d`: `{ "guild_id": "...", "channel_id": "...", "content": "..." }`
  - Creates and broadcasts message (same validation as REST)

Unknown event types or invalid envelopes close the connection.

### Server -> Client events
- `ready`
  - `d`: `{ "user_id": "..." }`
- `subscribed`
  - `d`: `{ "guild_id": "...", "channel_id": "..." }`
- `message_create`
  - `d`: message payload (same fields as `MessageResponse`)
- `presence_sync`
  - `d`: `{ "guild_id": "...", "user_ids": ["..."] }`
- `presence_update`
  - `d`: `{ "guild_id": "...", "user_id": "...", "status": "online|offline" }`

### Gateway disconnect reasons (observed in implementation)
The server tracks disconnect categories including:
- `slow_consumer`
- `event_too_large`
- `ingress_rate_limited`
- `invalid_envelope`
- `unknown_event`
- `forbidden_channel`
- `message_rejected`
- `socket_error`
- `client_close`
- `connection_closed`

## E2EE Endpoints

These endpoints provide the Delivery Service for MLS-based end-to-end encryption.
The server stores and relays opaque blobs — it never parses MLS interiors or
holds client or content-decryption keys. Its distinct external-sender signing
key can authorize only MLS Remove proposals. See
`docs/adr/0001-e2ee-mls-openmls.md` for the protocol decision.

### `GET /e2ee/delivery-service/identity`
Returns the authenticated public configuration clients pin before creating or
joining an MLS group with server-initiated removal support.
- Response: `{ "protocol_version": 1, "external_sender_index": 0, "signature_key": [32 bytes] }`
- Authentication is required. If the operator has not configured the stable
  signing key, the endpoint fails closed with
  `409 { "error": "e2ee_capability_required" }`.
- Clients treat any change from their pinned key as a blocking identity change;
  they never silently replace the Group Context external sender.
- The response contains public material only. The server signing surface emits
  only bounded external Remove proposals; it exposes no arbitrary signing or
  external Add operation.

### `PUT /e2ee/devices/{device_id}`
Publishes a device certificate for the authenticated user.
- Request body: `{ "device_signature_pubkey": [32 bytes], "root_key_signature": [64 bytes], "root_key_pub": [32 bytes] }`
- Response: `{ "device_id": "...", "published": true }`
- The path device ID and authenticated user ID are covered by the root-key
  signature. The server verifies the signature and pins the first root public
  key published for an account. Clients must independently verify certificates
  against their pinned root key; the directory remains an untrusted hint.
- Rate limit: 10 publishes/minute by both authenticated user and client IP by
  default (configurable, fail-fast if zero).

### `GET /e2ee/users/{user_id}/devices`
Lists certified devices for a user.
- Response: `{ "user_id": "...", "devices": [{ "device_id": "...", "device_signature_pubkey": [32 bytes], "root_key_signature": [64 bytes], "root_key_pub": [32 bytes], "created_at_unix": 0 }] }`
- Only active devices are returned; results are capped at 100.

### `GET /e2ee/users/{user_id}/identity`
Returns the current public root identity and its append-only continuity chain.
- Response fields include `protocol_version`, `current_root_key_pub`,
  `rotation_sequence`, and up to 100 ordered dual-signed rotation entries.
- Clients verify every transition from their locally pinned root. Missing,
  duplicated, reordered, disconnected, or invalidly signed entries fail closed.

### `POST /e2ee/identity/rotate`
Destructively rotates the authenticated user's root identity using protocol v1.
- The transition is bound to the user and next sequence and must be signed by
  both the previously pinned root and the replacement root.
- The replacement root also certifies fresh signing material for one retained
  active device. Every other device is irreversibly tombstoned and all
  unclaimed KeyPackages for the user are destroyed in the same transaction.
- A stale sequence, replay, malformed proof, unowned device, or unsupported
  protocol version is rejected. The bounded public proof and mutation counts
  are audit logged; no private key material reaches the server.

### `DELETE /e2ee/devices/{device_id}`
Irreversibly removes a device owned by the authenticated user.
- Response: `{ "device_id": "...", "tombstoned_at_unix": 0, "deleted_keypackage_count": 0 }`
- The certificate tombstone, deletion of all unclaimed KeyPackages, and public
  audit record are committed in one transaction. A tombstoned device ID cannot
  be republished; a newly paired device must use a fresh ID.
- Claimed packages cannot be recalled. Conversation-level cryptographic
  eviction begins with MLS group support in Phase 2.

### `POST /e2ee/keypackages`
Uploads a batch of KeyPackages for a device.
- Request body: `{ "device_id": "...", "key_packages": [{ "key_package_blob": [bytes], "is_last_resort": false }] }`
- Response: `{ "stored_count": 10 }`
- Requests contain 1–100 packages; each opaque blob is 1–4096 bytes. The
  authenticated user must own the active device, the unclaimed pool is capped
  at 100 by default, duplicates are ignored, and at most one unclaimed fallback
  may exist.

### `POST /e2ee/keypackages/claim`
Claims a KeyPackage for a target user/device.
- Request body: `{ "target_user_id": "...", "target_device_id": "..." | null }`
- Response: `{ "device_id": "...", "key_package_blob": [bytes], "is_last_resort": false }`
- Claims are atomic (`FOR UPDATE SKIP LOCKED`), audit-logged, and limited by
  requester user, target device, and client IP (30/minute by default).
- Ordinary packages are preferred. Every package is single-use, including the
  ordered fallback. Reuse remains disabled until an MLS last-resort extension
  is implemented and separately reviewed.
- A `keypackage_low` user-scoped gateway event is emitted after a successful
  claim leaves the target device below the replenishment water mark.

### `POST /e2ee/conversations`
Atomically provisions a new two-user MLS v1 conversation and its initial Add
commit.
- Request: `{ "conversation_id", "peer_user_id", "group_id", "suite_id", "committer_device_id", "welcome_device_id", "commit_blob", "welcome_blob", "group_info_blob" }`
- Response: `{ "conversation_id", "group_id", "crypto": "mls_v1", "epoch": 1, "suite_id", "provisioned_at_unix" }`
- Conversation, group, epoch-1 commit, Welcome, and GroupInfo rows commit in one
  transaction. MLS interiors remain opaque and use the existing `64 KiB` caps.
- Both distinct users must have at least one active certified MLS device, and
  the committer device must be active and owned by the caller. The Welcome
  device must be an active device owned by the peer. Capability gaps
  return `409 { "error": "e2ee_capability_required" }`; no plaintext fallback
  is created.
- Canonical user-pair uniqueness prevents duplicate encrypted DMs. An exact
  retry is idempotent; conflicting identifiers or bootstrap material return
  `409 { "error": "e2ee_conversation_conflict" }`.
- Provisioning uses the commit transport rate limit independently by client IP,
  caller, committer device, and group.

### `POST /e2ee/conversations/{conversation_id}/upgrade`
Explicitly upgrades an existing two-user plaintext conversation to MLS v1.
- Request: `{ "group_id", "suite_id", "committer_device_id", "welcome_device_id", "commit_blob", "welcome_blob", "group_info_blob" }`
- Response matches `POST /e2ee/conversations`.
- Only an existing member may upgrade, the membership must contain exactly two
  capable users, and all bootstrap rows commit atomically.
- The transition is one-way. A database trigger rejects every later attempt to
  change `mls_v1` back to plaintext; a downgrade requires a separate plaintext
  conversation and never changes the encrypted conversation's pinned mode.
- Exact retries are idempotent. A different group or bootstrap payload fails
  closed with `e2ee_conversation_conflict`.

### `GET /e2ee/groups/{group_id}/info`
Returns the latest opaque `GroupInfo` for an authenticated member of an
`mls_v1` conversation.
- Response: `{ "group_id": "...", "epoch": 1, "suite_id": 3, "group_info_blob": [bytes] }`
- Missing GroupInfo, non-membership, plaintext conversation mode, and unknown
  groups return `404` without revealing which authorization check failed.
- The server treats GroupInfo as opaque and applies the `64 KiB` hard cap.
- Native recovery treats every response field as an untrusted routing hint.
  The signed GroupInfo must match the pinned group, epoch, baseline suite,
  ratchet tree, and two-user root identities before an external commit is
  created. Recovery is prepared against an isolated clone of the complete MLS
  checkpoint, so rejection cannot overwrite the live provider state.

### `POST /e2ee/groups/{group_id}/commits`
Atomically orders one opaque MLS commit for an authenticated conversation member.
- Request: `{ "epoch": 1, "prior_epoch": 0, "committer_device_id": "...", "commit_blob": [bytes], "welcome_blob"?: [bytes], "welcome_device_id"?: "...", "group_info_blob"?: [bytes] }`
- Response `200`: `{ "accepted": true, "epoch": 1 }`
- The committer device must be active and owned by the authenticated user.
- Commits must advance exactly one epoch. A row lock makes the first valid
  commit for an epoch the sole winner; competitors receive
  `409 { "error": "epoch_conflict" }` and must rebase client-side.
- Rebase fetches and authenticates the accepted commit before clearing the
  rejected local commit. The native core advances through the normal pinned
  membership checks and restages a still-safe self-update, one-device Add, or
  Remove at the next epoch. A winning commit that already satisfied or
  invalidated the intent produces no retry; a rebased Add emits a new Welcome
  and the rejected Welcome must never be delivered.
- Commit, Welcome, and GroupInfo blobs are never parsed and are each capped at
  `64 KiB`. The default per-IP/user/device/group rate is 30 commits/minute.
- `welcome_blob` and `welcome_device_id` must be supplied together. The target
  must be a distinct active device owned by a conversation member.
- A recovery external commit is accepted by peers only when OpenMLS
  authenticates a `NewMemberCommit` whose update-path credential chains to one
  of the two pinned roots and matches the routed committer device. Its proposal
  shape is exactly one `ExternalInit` plus only the automatic same-device
  replacement `Remove`, when required. The candidate replaces local state only
  after an exact accepted-epoch response and an atomic encrypted checkpoint.

### `GET /e2ee/groups/{group_id}/commits`
Returns opaque commits pending for one owned active device.
- Query: `?device_id=<device ULID>&after_epoch=<epoch>&limit=<1..50>`
- Response: `{ "commits": [{ "epoch", "prior_epoch", "committer_device_id", "commit_blob", "welcome_blob"?, "created_at_unix", "expires_at_unix" }], "next_after_epoch": 2 | null }`
- Active participant devices are snapshotted in the commit transaction; the
  committer is immediately marked delivered. A Welcome is returned only to
  its exact target device and is omitted from every other device's response.
- Pages are capped at 50 records and `256 KiB` aggregate commit/Welcome bytes.
  New devices do not gain access to earlier commits through this endpoint.
- Native clients preflight the whole page, join only through a device-bound
  Welcome, and otherwise authenticate and merge commits in strict epoch order.
  Processing stops at the first rejected epoch; no later epoch is acknowledged.

### `POST /e2ee/groups/{group_id}/commits/ack`
Acknowledges successfully processed commits for one owned active device.
- Request: `{ "device_id": "...", "epochs": [1, 2] }`
- Response: `{ "acknowledged_count": 2, "deleted_count": 2 }`
- Batches contain 1–100 unique positive epochs. Entries outside the device's
  snapshotted group mailbox are ignored without exposing other groups.
- Clients send this request only after the corresponding joined/advanced MLS
  state is durably persisted. Already-durable replay epochs may be
  acknowledged without consuming the commit again.
- Once every snapshotted device acknowledges, the commit, its optional Welcome,
  and all delivery rows are hard-deleted atomically. TTL GC remains an
  independent upper bound.

### `POST /e2ee/groups/{group_id}/proposals`
Stores one member-authored opaque MLS proposal at the group's current epoch.
- Request: `{ "epoch": 1, "proposer_device_id": "...", "proposal_blob": [bytes] }`
- Response: `{ "proposal_id": "...", "created_at_unix": 0 }`
- The proposer device must be active, owned by the authenticated conversation
  member, and rate-limited independently by IP, user, device, and group.
- The server validates only the current epoch and the `64 KiB` blob bound. It
  never parses or trusts the proposal kind. Packaged clients authenticate the
  MLS sender and enforce Add/Remove/Update policy before proposal storage.
- Active participant devices are snapshotted atomically; the proposing device
  is immediately marked delivered. A routing-only `mls_proposal` event prompts
  clients to read their mailbox.

### `GET /e2ee/groups/{group_id}/proposals`
Returns opaque proposals pending for one owned active device.
- Query: `?device_id=<device ULID>&after_proposal_id=<proposal ULID>&limit=<1..50>`
- Response: `{ "proposals": [{ "proposal_id", "epoch", "proposer_device_id", "proposal_blob", "created_at_unix", "expires_at_unix" }], "next_after_proposal_id": "..." | null }`
- Pages are capped at 50 records and `256 KiB` aggregate proposal bytes. New
  devices do not gain access to proposals created before their delivery
  snapshot. All routing fields remain untrusted until MLS authentication.

### `POST /e2ee/groups/{group_id}/proposals/ack`
Acknowledges authenticated, durably stored proposals for one owned device.
- Request: `{ "device_id": "...", "proposal_ids": ["..."] }`
- Response: `{ "acknowledged_count": 1, "deleted_count": 1 }`
- Batches contain 1–100 unique canonical proposal ULIDs. A proposal and all
  delivery rows are hard-deleted after every snapshotted device acknowledges,
  or independently when the configured mailbox TTL expires.

### `POST /e2ee/groups/{group_id}/messages`
Stores one opaque MLS `PrivateMessage` in the bounded delivery mailbox.
- Request: `{ "epoch": 1, "suite_id": 3, "sender_device_id": "...", "message_blob": [bytes] }`
- Response: `{ "message_id": "...", "created_at_unix": 0 }`
- The sender device must be active and owned by the authenticated conversation
  member. Epoch and suite routing hints must equal the provisioned group state.
- The authenticated application envelope is padded before MLS encryption, then
  the opaque serialized MLS frame is zero-filled to exactly `512 B`, `1 KiB`,
  `4 KiB`, or `16 KiB`; all other transport sizes fail closed. Clients reject
  nonzero transport fill and never release padding bytes as content.
- Rows are always tagged `mls_v1`, contain no plaintext/content-derived fields,
  and receive a configurable mailbox-expiry deadline (30 days by default,
  90-day hard maximum). Active participant devices are snapshotted in the same
  transaction; the sending device is immediately marked delivered. A missing
  active device for either participant fails closed with
  `409 { "error": "e2ee_capability_required" }`. The default
  per-IP/user/device/group rate is 120 messages/minute.

### `GET /e2ee/groups/{group_id}/mailbox`
Returns opaque messages pending for one owned active device.
- Query: `?device_id=<device ULID>&after_message_id=<message ULID>&limit=<1..50>`
- Response: `{ "messages": [{ "message_id", "crypto": "mls_v1", "epoch", "suite_id", "sender_device_id", "message_blob", "created_at_unix", "expires_at_unix" }], "next_after_message_id": "..." | null }`
- Only send-time delivery rows for the requested device and group are visible.
  New devices do not gain access to earlier ciphertext through this endpoint.
- Pages are capped at 50 records and `256 KiB` aggregate ciphertext bytes.
  Routing fields remain untrusted hints. Native clients validate the canonical
  cursor and IDs before touching MLS state, isolate malformed entries, and
  acknowledge only records that pass MLS authentication, decryption, and local
  metadata checks. Authenticated plaintext and updated MLS state must be
  durably persisted before the returned acknowledgment is sent. The native
  client commits history, MLS state, and the pending acknowledgment in one
  encrypted-store transaction; after a restart it resubmits that durable
  outbox before reading another page for the group.

### `POST /e2ee/groups/{group_id}/messages/ack`
Acknowledges successfully decrypted messages for one owned active device.
- Request: `{ "device_id": "...", "message_ids": ["..."] }`
- Response: `{ "acknowledged_count": 1, "deleted_count": 1 }`
- Batches contain 1–100 unique canonical message ULIDs. IDs outside the
  device's snapshotted group mailbox are ignored without exposing other groups.
- When every snapshotted device has acknowledged a message, its ciphertext and
  delivery rows are hard-deleted atomically. Independently, a bounded
  background worker hard-deletes messages, commits, and proposals at their TTL
  deadline.

## Notes
- Search index is derived/cache; source of truth is persisted message storage.
- Voice token route name remains `/voice/token` but supports scoped publish/subscribe grants for voice/video/screen share.
