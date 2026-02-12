# PLAN_UX.md

## Objective
Implement as much of `docs/API.md` as possible in the web client while preserving security constraints and strict client-side validation.

## Progress Log
- 2026-02-12: Completed `PLAN_WEB_REFACTOR` Phase 8 cleanup/documentation/enforcement pass: removed stale imports in app-shell/api/tests, added warning-only CI size guardrail for `AppShellPage.tsx` (`npm run check:app-shell-size`), and closed validation gates with `npm --prefix apps/filament-client-web test` (`40` files / `162` tests) plus `npm --prefix apps/filament-client-web run build`.
- 2026-02-11: Completed app-shell cleanup pass by moving additional workspace/overlay/message-media/voice lifecycle orchestration into typed controller modules, reducing `AppShellPage` orchestration weight; lazy-loaded heavy overlay panel groups (public/friend/settings + operator tools) and split them into dedicated production chunks; added focused controller regressions for workspace pruning/selection and overlay panel default-open behavior.
- 2026-02-11: Finalized channel-group header ergonomics: made Text/Voice section bars bleed full rail width and added explicit `+` create actions to both headers (with typed defaults for text vs voice channel creation).
- 2026-02-11: Refined channel rail details to better match Discord ergonomics: section headers now render as full-width bars, channel creation moved to a `+` action at the right edge of the Voice Channels header, and in-call roster presentation shifted from a boxed panel to an indented tree-style participant list.
- 2026-02-11: Refreshed web app-shell voice/text UX toward Discord-like density and hierarchy: redesigned channel rail groups, added persistent voice-connected dock controls with live call duration, upgraded message rows with avatar/meta layout, and removed forced voice disconnect on same-workspace channel switches; updated voice-controls regression coverage for persistent in-call behavior.
- 2026-02-11: Completed RTC hardening/release gate slice for web client: added explicit voice troubleshooting UX for token/session expiry, permission rejection, and signaling failure; added reconnect/disconnect and join-permission regression coverage; and updated LiveKit signaling env/docs guidance (`ws://localhost:7880` local compose default + deploy patterns).
- 2026-02-10: Reworked app shell information architecture toward a Discord-style layout: moved workspace creation to a `+` action in the left workspace rail, added collapsible channel/member rails, and converted workspace/channel/directory/friendship/operator tools into focus modal windows with explicit close actions; updated app-shell regression tests for modal entry points and close behavior.
- 2026-02-10: Added typed chat domain models and invariant constructors in `apps/filament-client-web/src/domain/chat.ts`.
- 2026-02-10: Expanded API client coverage for guild/channel/message/search endpoints in `apps/filament-client-web/src/lib/api.ts`.
- 2026-02-10: Replaced demo-only shell interactions with API-backed flows in `apps/filament-client-web/src/pages/AppShellPage.tsx`.
- 2026-02-10: Added workspace cache persistence for known guild/channel IDs in `apps/filament-client-web/src/lib/workspace-cache.ts`.
- 2026-02-10: Updated and expanded tests for routing and chat-domain invariants.
- 2026-02-10: Added gateway websocket client wiring (`/gateway/ws`) with safe envelope parsing and live presence/message updates in the web shell.
- 2026-02-10: Added message reactions (`POST/DELETE .../reactions/{emoji}`) to API client and UI.
- 2026-02-10: Expanded client domain/API coverage for attachments, moderation, channel overrides, voice token issuance, search maintenance, and auth refresh/logout endpoints.
- 2026-02-10: Upgraded app shell UX with message edit/delete, history pagination, safe markdown token rendering, attachment lifecycle actions, moderation controls, and utility diagnostics.
- 2026-02-10: Added integration-like operator console tests in `apps/filament-client-web/tests/app-shell-operator-permissions.test.tsx` to validate owner/member permission fixture behavior for moderation, channel overrides, search maintenance, and voice token actions.
- 2026-02-10: Fixed client guild visibility bootstrap by pruning cached workspaces/channels that fail authenticated access checks; added `apps/filament-client-web/tests/app-shell-workspace-visibility.test.tsx` coverage to prevent non-member/private guild leakage in UI/cache.
- 2026-02-10: Added guild visibility model (`private|public`) across server/client, `GET /guilds/public` authenticated discovery endpoint with bounded query/limit, and public workspace directory UX in the web shell with tests.
- 2026-02-10: Added server-configured per-user guild creation caps (`FILAMENT_MAX_CREATED_GUILDS_PER_USER`) with strict API enforcement, explicit creator tracking, and web-shell error handling/tests for limit exhaustion.
- 2026-02-10: Added channel self-permission snapshot endpoint (`GET /guilds/{guild_id}/channels/{channel_id}/permissions/self`) and applied least-visibility UI gating so privileged/operator controls are hidden unless explicitly allowed for the active channel role/permissions.
- 2026-02-10: Added authenticated "new workspace" UX flow in the web shell so users can create additional guilds/channels at any time (including mobile header access), with limit-error handling and regression coverage.
- 2026-02-10: Added hCaptcha-backed signup hardening across `POST /auth/register` (server-side verification, fail-closed behavior) and web registration UX (token capture + submission), with server/frontend regression tests and deploy/docs updates.
- 2026-02-10: Added friendship system support across server + web client (`/friends`, `/friends/requests`, accept/remove flows), including strict ID validation, permission-safe request visibility, and regression coverage.
- 2026-02-10: Refactored ops console into layered open/close overlay panels for search, attachments, voice, moderation, and utility actions to replace dense always-expanded admin rails; updated permission fixture tests for panel launch/back flows.
- 2026-02-10: Added authenticated workspace/channel list discovery endpoints (`GET /guilds`, `GET /guilds/{guild_id}/channels`) and replaced web-shell local cache bootstrap probing with server-driven discovery + persistence, with frontend and server regression coverage.

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
- [x] Validate expanded operator UI against backend role/permission fixtures with dedicated integration-like frontend tests.
- [x] Fix guild visibility bug: only show guilds the authenticated user is a member of; private guilds must require invite/membership before appearing.
- [x] Add public guild discovery model: support guild visibility state (private/public) and a server-level searchable public guild list UI.
- [x] Enforce least-visibility defaults across API + client: if a user lacks default permission for a resource, the resource must not be discoverable or rendered in UI (including preventing ops console exposure for non-members).
- [x] Add configurable per-user guild creation limits: allow self-serve guild creation for all users, constrained by a server-configured max created guild count per user.
- [x] Add UX flow for any authenticated user to create their own guild (subject to server-configured limits).
- [x] Add captcha verification to account creation UX + backend registration flow to reduce automated signup abuse.
- [x] Add friendship system UX + backend support (requests, acceptance, list management, and permission-safe exposure).
- [x] Add robust username query/lookup system with client-side caching and smart invalidation so name resolution is dynamic without naive repeated re-fetching.

## In Progress
- [ ] None.

## Next
- [x] If backend adds list endpoints for guilds/channels, replace local workspace cache bootstrap with server-driven discovery.
