# PLAN_UX.md

## Objective
Implement as much of `docs/API.md` as possible in the web client while preserving security constraints and strict client-side validation.

## Progress Log
- 2026-02-10: Added typed chat domain models and invariant constructors in `apps/filament-client-web/src/domain/chat.ts`.
- 2026-02-10: Expanded API client coverage for guild/channel/message/search endpoints in `apps/filament-client-web/src/lib/api.ts`.
- 2026-02-10: Replaced demo-only shell interactions with API-backed flows in `apps/filament-client-web/src/pages/AppShellPage.tsx`.
- 2026-02-10: Added workspace cache persistence for known guild/channel IDs in `apps/filament-client-web/src/lib/workspace-cache.ts`.
- 2026-02-10: Updated and expanded tests for routing and chat-domain invariants.
- 2026-02-10: Added gateway websocket client wiring (`/gateway/ws`) with safe envelope parsing and live presence/message updates in the web shell.
- 2026-02-10: Added message reactions (`POST/DELETE .../reactions/{emoji}`) to API client and UI.
- 2026-02-10: Expanded client domain/API coverage for attachments, moderation, channel overrides, voice token issuance, search maintenance, and auth refresh/logout endpoints.
- 2026-02-10: Upgraded app shell UX with message edit/delete, history pagination, safe markdown token rendering, attachment lifecycle actions, moderation controls, and utility diagnostics.

## Completed
- [x] Login flow reliably navigates to app shell.
- [x] App shell supports first workspace creation via `POST /guilds` + `POST /guilds/{guild_id}/channels`.
- [x] App shell loads message history via `GET /guilds/{guild_id}/channels/{channel_id}/messages`.
- [x] Composer sends messages via `POST /guilds/{guild_id}/channels/{channel_id}/messages`.
- [x] Search panel queries `GET /guilds/{guild_id}/search`.
- [x] Strict response parsing and invariant checks on client DTO conversion.
- [x] Gateway websocket integration for `ready`, `message_create`, `presence_sync`, `presence_update`.
- [x] Reactions UI + API path for add/remove reaction flows.
- [x] Message edit/delete UI wired to `PATCH/DELETE /messages/{message_id}`.
- [x] Message history pagination control using `before` cursor + `next_before`.
- [x] Attachment upload/download/delete UI wired to attachment endpoints.
- [x] Voice token request UI wired to `/voice/token`.
- [x] Search maintenance actions wired to `/search/rebuild` + `/search/reconcile`.
- [x] Moderation/member management actions wired to add/role/kick/ban endpoints.
- [x] Channel role override UI wired to `/overrides/{role}`.
- [x] Session refresh/logout actions wired to `/auth/refresh` + `/auth/logout`.
- [x] Chat message rendering uses server `markdown_tokens` safe token stream (no HTML path).

## In Progress
- [ ] Validate expanded operator UI against backend role/permission fixtures with dedicated integration-like frontend tests.

## Next
- [ ] Fix guild visibility bug: only show guilds the authenticated user is a member of; private guilds must require invite/membership before appearing.
- [ ] Add public guild discovery model: support guild visibility state (private/public) and a server-level searchable public guild list UI.
- [ ] Add configurable per-user guild creation limits: allow self-serve guild creation for all users, constrained by a server-configured max created guild count per user.
- [ ] Enforce least-visibility defaults across API + client: if a user lacks default permission for a resource, the resource must not be discoverable or rendered in UI (including preventing ops console exposure for non-members).
- [ ] Add UX flow for any authenticated user to create their own guild (subject to server-configured limits).
- [ ] Add friendship system UX + backend support (requests, acceptance, list management, and permission-safe exposure).
- [ ] Add robust username query/lookup system with client-side caching and smart invalidation so name resolution is dynamic without naive repeated re-fetching.
- [ ] Improve ops console UX by moving guild/admin settings into layered overlay panels (open/close) instead of a single dense rail.
- [ ] If backend adds list endpoints for guilds/channels, replace local workspace cache bootstrap with server-driven discovery.
