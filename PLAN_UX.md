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

## Completed
- [x] Login flow reliably navigates to app shell.
- [x] App shell supports first workspace creation via `POST /guilds` + `POST /guilds/{guild_id}/channels`.
- [x] App shell loads message history via `GET /guilds/{guild_id}/channels/{channel_id}/messages`.
- [x] Composer sends messages via `POST /guilds/{guild_id}/channels/{channel_id}/messages`.
- [x] Search panel queries `GET /guilds/{guild_id}/search`.
- [x] Strict response parsing and invariant checks on client DTO conversion.
- [x] Gateway websocket integration for `ready`, `message_create`, `presence_sync`, `presence_update`.
- [x] Reactions UI + API path for add/remove reaction flows.

## In Progress
- [ ] Attachment upload UI wired to attachments endpoints (with explicit size + MIME UX checks).

## Next
- [ ] Voice token UI path using `/voice/token`.
- [ ] Pagination controls (`before` cursor) for older message history.
- [ ] If backend adds list endpoints for guilds/channels, replace local workspace cache bootstrap with server-driven discovery.
