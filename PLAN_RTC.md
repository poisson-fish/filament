# PLAN_RTC.md

## Objective
Ship production-safe Discord-style RTC in the web client: voice channels with optional video/screen streams, using existing LiveKit-backed backend policy without weakening security controls.

## Code-State Audit (2026-02-11)
- Backend already issues scoped LiveKit tokens at `POST /guilds/{guild_id}/channels/{channel_id}/voice/token` with TTL cap, permission filtering, and abuse controls.
- Backend tests already cover Phase 5/6 token policy (`apps/filament-server/tests/phase5_livekit_voice.rs`, `apps/filament-server/tests/phase6_video_streams.rs`).
- Web client currently supports only manual token issuance UI in the operator panel (`AppShellPage.tsx`), not actual room join/publish/subscribe behavior.
- Web client has no LiveKit SDK dependency yet (`apps/filament-client-web/package.json`).
- Channel model has no channel kind/type in server or web domain (`text/voice` not represented), and no explicit stream capability modeling at the UI level.
- Gateway protocol/client has no RTC events beyond text/presence.
- Create-channel UX currently captures only name; no channel-type selector exists.
- Web client has no dedicated settings panel (no left-rail categories, no voice settings surface yet).
- Local compose now defaults `FILAMENT_LIVEKIT_URL=ws://localhost:7880`, and deployment docs define browser-reachable `ws://`/`wss://` patterns for local/dev/prod.

## Phase Rules
- Every phase must be finishable in one LLM context session.
- Every phase must land tests with the code.
- Never relax limits/rate controls/CSP/security validations to unblock UX.
- Append implementation notes to the completed phase before moving on.

## RTC Capability Model (Locked)
- Channel kinds for RTC are `voice` (plus `text` for non-RTC channels). No separate `video` channel kind.
- A `voice` channel always supports voice transport semantics.
- Video/screen share are optional stream capabilities on top of voice.
- Stream capabilities are governed by existing hierarchical permissions and channel overrides (`publish_video`, `publish_screen_share`, `subscribe_streams`).
- If stream permissions are denied, voice still works. Stream-only without voice is not a supported mode.

## Status Legend
- `NOT STARTED`
- `IN PROGRESS`
- `DONE`
- `BLOCKED`

## Phase 1 - Channel Kind Foundation
### Goal
Add explicit channel kinds so text and voice channels exist as first-class entities in API, storage, and UI.

### Completion Status
`DONE`

### Tasks
- [x] Add `ChannelKind` domain type on server (`text`, `voice`) with invariant conversions and tests.
- [x] Extend channel persistence schema to store channel kind (safe default: `text` for existing rows).
- [x] Update create/list channel APIs to accept/return `kind` with strict DTO parsing.
- [x] Update web domain models/cache parsing to include `kind` with backward-compatible fallback only for old cache entries.
- [x] Update app shell channel rail grouping (`TEXT CHANNELS`, `VOICE CHANNELS`) without RTC call behavior yet.
- [x] Update create-channel UX to include channel type selection (`text` or `voice`) with safe default (`text`) and strict client-side validation.
- [x] Add/adjust integration tests (server) and domain/UI tests (web) for channel kind flows.
- [x] Update `docs/API.md` for channel kind request/response shape.

### Exit Criteria
- Channel create/list endpoints round-trip channel kind correctly.
- Existing channels migrate safely to `text`.
- Web app renders grouped channels by kind.
- Create-channel flow allows explicit type selection and sends validated type to API.
- No separate `video` channel kind is introduced.
- All touched test suites pass.

### Implementation Notes (Fill After Completion)
- Date completed: 2026-02-11
- PR/commit: local changes pending commit
- Files changed:
  - `crates/filament-core/src/lib.rs`
  - `apps/filament-server/src/lib.rs`
  - `apps/filament-server/tests/postgres_phase1_flow.rs`
  - `apps/filament-client-web/src/domain/chat.ts`
  - `apps/filament-client-web/src/lib/api.ts`
  - `apps/filament-client-web/src/pages/AppShellPage.tsx`
  - `apps/filament-client-web/tests/domain-chat.test.ts`
  - `apps/filament-client-web/tests/app-shell-channel-kinds.test.tsx`
  - `apps/filament-client-web/tests/app-shell-workspace-visibility.test.tsx`
  - `apps/filament-client-web/tests/app-shell-reactions.test.tsx`
  - `apps/filament-client-web/tests/app-shell-public-discovery.test.tsx`
  - `apps/filament-client-web/tests/app-shell-operator-permissions.test.tsx`
  - `apps/filament-client-web/tests/app-shell-guild-creation-limits.test.tsx`
  - `apps/filament-client-web/tests/app-shell-friendships.test.tsx`
  - `apps/filament-client-web/tests/app-shell-composer-attachments.test.tsx`
  - `docs/API.md`
- Security-impact notes:
  - Channel kind is now validated at the domain boundary (`text`/`voice` only) and persisted with server-side numeric enum mapping.
  - Schema migration backfills legacy rows to `text` and enforces `NOT NULL` to avoid ambiguous channel type behavior.
  - Web cache fallback defaults only missing legacy `kind` fields to `text`; API payload parsing remains strict for malformed kinds.
- Tests run:
  - `cargo test -p filament-core`
  - `cargo test -p filament-server`
  - `cargo clippy -p filament-core -p filament-server --all-targets`
  - `npm --prefix apps/filament-client-web test`
- Follow-ups/debt:
  - Phase 2 can consume `channel.kind` directly from `activeChannel()` and workspace cache without additional schema work.

### Handoff To Next Phase
- Confirm channel kind is available in `activeChannel()` state and persisted workspace cache.

## Phase 2 - RTC Client Core (LiveKit Wrapper)
### Goal
Add a hardened RTC client layer in web app that can connect/disconnect safely using issued tokens.

### Completion Status
`DONE`

### Tasks
- [x] Add `livekit-client` dependency to web app.
- [x] Introduce `apps/filament-client-web/src/lib/rtc.ts` abstraction (join, leave, toggle mic, subscribe hooks) so SDK usage is centralized.
- [x] Validate `livekit_url` before connect (`ws://` or `wss://` only); reject invalid/empty values.
- [x] Bound in-memory RTC state (participants/tracks) to avoid unbounded growth from hostile/malformed event streams.
- [x] Add deterministic teardown on logout/channel switch/page unmount.
- [x] Add unit tests for URL/token validation and lifecycle transitions using mocks.

### Exit Criteria
- RTC wrapper can join and leave a room with no leaked listeners/tracks.
- Invalid URL/token inputs fail closed.
- Wrapper behavior is covered by tests.

### Implementation Notes (Fill After Completion)
- Date completed: 2026-02-11
- PR/commit: local changes pending commit
- Files changed:
  - `apps/filament-client-web/package.json`
  - `apps/filament-client-web/package-lock.json`
  - `apps/filament-client-web/src/lib/rtc.ts`
  - `apps/filament-client-web/tests/rtc.test.ts`
  - `PLAN_RTC.md`
- Security-impact notes:
  - RTC join path now fail-closes with strict `ws://`/`wss://` URL parsing, credential/fragment rejection, and printable bounded token validation.
  - In-memory RTC participant and track state is explicitly bounded (default 256 participants, 32 tracks per participant), dropping overflow events to reduce hostile stream amplification risk.
  - `join`, `leave`, and `destroy` use serialized operations and deterministic listener unbinding/disconnect to avoid leaked subscriptions during logout/channel switches/unmount.
- Tests run:
  - `npm --prefix apps/filament-client-web test -- rtc.test.ts`
- Follow-ups/debt:
  - Phase 3 should wire `rtc.ts` lifecycle into `AppShellPage` channel switching and auth/session teardown paths as voice join UI replaces the temporary operator token flow.

### Handoff To Next Phase
- Expose stable UI-facing API from `rtc.ts` for connection status, local mute state, and participant list.

## Phase 3 - Voice UX MVP
### Goal
Make voice channels usable end-to-end (join, talk, leave) in app shell.

### Completion Status
`DONE`

### Tasks
- [x] Replace operator-style "Issue token" flow with channel-centric voice controls for `voice` channels.
- [x] Add header actions for `Join Voice`, `Leave`, `Mute/Unmute Mic`.
- [x] On join, request token from backend with voice-first sources (`microphone`) and connect via RTC wrapper.
- [x] Show call connection state (`connecting`, `connected`, `reconnecting`, `error`) and actionable error messages.
- [x] Display in-call participant roster from LiveKit participant events.
- [x] Implement VAD/active-speaker state from LiveKit audio-level events with debounce/hysteresis to avoid flicker.
- [x] Highlight currently speaking participant names in green in the in-call participant roster.
- [x] Ensure channel switch/logout always leaves room and clears media state.
- [x] Add UI tests for join/leave/mute state and API payload correctness.
- [x] Add tests for active-speaker highlighting transitions (`idle -> speaking -> idle`).

### Exit Criteria
- User can join voice channel and publish microphone audio.
- User can leave cleanly and rejoin without stale state.
- Voice controls are hidden/disabled when channel kind or permissions disallow them.
- Speaking participants are visually highlighted in green with stable VAD behavior (no rapid flicker).
- Tests cover primary voice paths.

### Implementation Notes (Fill After Completion)
- Date completed: 2026-02-11
- PR/commit: local changes pending commit
- Files changed:
  - `apps/filament-client-web/src/pages/AppShellPage.tsx`
  - `apps/filament-client-web/tests/app-shell-voice-controls.test.tsx`
  - `PLAN_RTC.md`
- Security-impact notes:
  - RTC teardown now fail-closes in `releaseRtcClient()` and always clears local room/media state even if SDK `destroy()` fails.
  - Voice session cleanup is now explicitly covered in UI tests for channel switching and logout to reduce regression risk for stale microphone/session state.
  - Voice token request flow remains voice-first and least-privilege (`publish_sources: ["microphone"]`, `can_subscribe: true`) with test assertions.
- Tests run:
  - `npm --prefix apps/filament-client-web test -- app-shell-voice-controls.test.tsx`
  - `npm --prefix apps/filament-client-web test`
  - `npm --prefix apps/filament-client-web run build`
- Follow-ups/debt:
  - Phase 4 should reuse Phase 3 teardown semantics while layering device-selection state into join/publish behavior.

### Handoff To Next Phase
- Keep voice call state stable while layering settings UX for audio-device control.

## Phase 4 - Settings Panel Foundation + Voice Settings
### Goal
Add a baseline settings panel with left-rail categories and a `Voice` submenu containing audio device settings.

### Completion Status
`DONE`

### Tasks
- [x] Add a global Settings entry point in app shell (gear/action) with open/close behavior.
- [x] Build settings panel layout with left rail for categories and right content pane.
- [x] Add baseline categories structure in rail, including `Voice` and `Profile` (profile as placeholder only in this plan).
- [x] Implement `Voice -> Audio Devices` submenu page.
- [x] Add audio input/output device selectors (microphone, speaker) using browser media-device enumeration with strict error handling.
- [x] Add safe defaults and persistence for selected device IDs in local client state/storage.
- [x] Wire selected devices into RTC flow state (without changing backend permission model).
- [x] Add UI tests for panel navigation, `Voice` submenu selection, and device selector behavior with mocked media devices.

### Exit Criteria
- Settings panel opens from app shell and renders category rail + content pane.
- `Voice -> Audio Devices` page exists and allows selecting microphone/speaker devices.
- Device selections persist and are reapplied on reload when devices are still available.
- Profile appears only as non-functional placeholder/navigation stub for future plan scope.
- Tests cover settings navigation and audio-device selection flows.

### Implementation Notes (Fill After Completion)
- Date completed: 2026-02-11
- PR/commit: local changes pending commit
- Files changed:
  - `apps/filament-client-web/src/pages/AppShellPage.tsx`
  - `apps/filament-client-web/src/lib/rtc.ts`
  - `apps/filament-client-web/src/lib/voice-device-settings.ts`
  - `apps/filament-client-web/tests/app-shell-settings-entry.test.tsx`
  - `apps/filament-client-web/tests/app-shell-voice-controls.test.tsx`
  - `apps/filament-client-web/tests/rtc.test.ts`
  - `apps/filament-client-web/tests/voice-device-settings.test.ts`
  - `PLAN_RTC.md`
- Security-impact notes:
  - Added strict local validation for audio device IDs and bounded browser device enumeration (kind-filtered, deduped, capped) to reduce malformed-device state propagation.
  - Device preferences are now persisted with bounded local-storage parsing and fail-safe defaults (`system default`) when storage payloads or saved IDs are invalid/unavailable.
  - RTC wrapper now applies/stages preferred audio input/output device IDs through controlled `switchActiveDevice` calls and surfaces bounded errors without dropping active room state.
- Tests run:
  - `npm --prefix apps/filament-client-web test`
  - `npm --prefix apps/filament-client-web run build`
- Follow-ups/debt:
  - Stream-oriented Phase 5 controls should reuse the same preference/state persistence model for camera and screen-source selection semantics.

### Handoff To Next Phase
- Reuse settings-managed audio device preferences for stream publish UX.

## Phase 5 - Streams On Voice Channels (Video + Screen Share)
### Goal
Enable camera and screen share controls within voice channels, with permission-aware behavior.

### Completion Status
`DONE`

### Tasks
- [x] Add `Camera On/Off` and `Share Screen/Stop Share` controls for active calls.
- [x] Request publish sources based on desired capabilities and backend grants.
- [x] Render remote video tiles and local preview, with bounded visible stream count (client DoS guardrail).
- [x] Surface permission-denied states clearly when camera/screen grants are not allowed.
- [x] Keep stream controls capability-based on top of voice; do not add a separate video-channel execution path.
- [x] Add tests for source request mapping, permission-clamped UI, and tile rendering fallback behavior.

### Exit Criteria
- Camera and screen share can be started/stopped for authorized users.
- Unauthorized publish attempts do not expose enabled controls.
- Voice call behavior remains functional when stream permissions are denied.
- UI remains stable when remote participants publish/unpublish frequently.

### Implementation Notes (Fill After Completion)
- Date completed: 2026-02-11
- PR/commit: local changes pending commit
- Files changed:
  - `apps/filament-client-web/src/lib/rtc.ts`
  - `apps/filament-client-web/src/pages/AppShellPage.tsx`
  - `apps/filament-client-web/src/styles/app.css`
  - `apps/filament-client-web/tests/rtc.test.ts`
  - `apps/filament-client-web/tests/app-shell-voice-controls.test.tsx`
  - `PLAN_RTC.md`
- Security-impact notes:
  - Stream publish controls are now explicitly dual-gated by channel permission snapshot and effective token grants, so unauthorized camera/screen publishes never render enabled controls client-side.
  - Voice token requests now include only capability-derived publish sources (`microphone` baseline + permitted camera/screen), with backend clamping still authoritative and surfaced in explicit in-call denial messaging.
  - RTC wrapper now tracks local/remote camera + screen-share stream state via bounded per-identity entries (max two sources per identity), preventing unbounded tile-state growth under hostile publish churn.
  - Stream tile rendering is capped to a fixed visible bound (`12`) with overflow messaging, limiting DOM amplification from high-frequency or large participant stream sets.
- Tests run:
  - `npm --prefix apps/filament-client-web test -- rtc.test.ts app-shell-voice-controls.test.tsx`
  - `npm --prefix apps/filament-client-web test`
  - `npm --prefix apps/filament-client-web run build`
- Follow-ups/debt:
  - Phase 6 should add dedicated reconnect/token-expiry stream UX (camera/screen state recovery messaging when reconnecting with clamped or expired grants).

### Handoff To Next Phase
- Preserve media control state model for final hardening/docs phase.

## Phase 6 - RTC Hardening, Environment, and Release Gates
### Goal
Close operational/security gaps and ship with clear runbook + verification.

### Completion Status
`DONE`

### Tasks
- [x] Verify `FILAMENT_LIVEKIT_URL` guidance and defaults so clients receive a reachable signaling URL in local/dev/prod.
- [x] Add explicit troubleshooting UX for token expiry, connection failure, and permission rejection.
- [x] Add/extend docs (`docs/API.md`, `docs/DEPLOY.md`, `README.md`) for channel kinds and RTC UX behavior.
- [x] Add regression tests for reconnect/disconnect flows and permission edge-cases.
- [x] Run and record required gates for touched areas (web tests, relevant Rust tests, fmt/lint as applicable).
- [x] Update `PLAN_UX.md` and this file with completion notes.

### Exit Criteria
- Local/dev environment can perform a full voice+video call flow with documented setup.
- Security controls remain intact (no limit/cap/CSP relaxations).
- Documentation reflects final behavior and known limitations.

### Implementation Notes (Fill After Completion)
- Date completed: 2026-02-11
- PR/commit: local changes pending commit
- Files changed:
  - `apps/filament-client-web/src/pages/AppShellPage.tsx`
  - `apps/filament-client-web/tests/app-shell-voice-controls.test.tsx`
  - `apps/filament-client-web/tests/rtc.test.ts`
  - `infra/docker-compose.yml`
  - `docs/API.md`
  - `docs/DEPLOY.md`
  - `README.md`
  - `PLAN_RTC.md`
  - `PLAN_UX.md`
- Security-impact notes:
  - Voice join failures now surface explicit, fail-closed operator messages for permission rejection (`forbidden`), session/token expiry (`invalid_credentials` and token-expired join failures), and signaling failures, reducing ambiguous retry behavior.
  - Runtime reconnect/disconnect handling now reports reconnect attempts and clears stale active-call capability state when transport drops, preventing stale grant assumptions after unclean disconnects.
  - Compose/deploy defaults now document browser-reachable LiveKit signaling URLs, reducing a common misconfiguration where tokens are valid but clients cannot resolve/connect to signaling hosts.
- Tests run:
  - `npm --prefix apps/filament-client-web test -- rtc.test.ts app-shell-voice-controls.test.tsx`
  - `npm --prefix apps/filament-client-web test`
  - `npm --prefix apps/filament-client-web run build`
- Follow-ups/debt:
  - Add optional runtime health surface to preflight LiveKit signaling reachability before first join attempt in large deployments with split DNS.

### Handoff / Next Backlog (Post-Plan)
- Server-driven cross-client RTC occupancy indicators in channel list.
- Active-speaker events surfaced in gateway for non-joined observers.
- Desktop client RTC parity after web UX stabilizes.
