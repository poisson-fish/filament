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
  - Response `200`: `{ "user_id": "...", "username": "..." }`
- `POST /users/lookup`
  - Auth required
  - Request: `{ "user_ids": ["..."] }`
  - `user_ids`: deduplicated server-side, `1..=64` ULID values
  - Response `200`:
    - `{ "users": [{ "user_id": "...", "username": "..." }] }`
  - Missing users are omitted from `users`

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
- `text { text }`
- `code { code }`
- `soft_break`, `hard_break`

`attachments` contains zero or more attachment records linked to this message.

### Reactions
- `POST /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}/reactions/{emoji}`
- `DELETE /guilds/{guild_id}/channels/{channel_id}/messages/{message_id}/reactions/{emoji}`
  - Auth required, channel write permission
  - Response `200`: `{ "emoji": "...", "count": <number> }`

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

## Notes
- Search index is derived/cache; source of truth is persisted message storage.
- Voice token route name remains `/voice/token` but supports scoped publish/subscribe grants for voice/video/screen share.
