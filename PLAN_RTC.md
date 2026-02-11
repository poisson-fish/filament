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
- Local compose default `FILAMENT_LIVEKIT_URL=ws://livekit:7880` is internal-network oriented; client reachability must be validated per environment.

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
`IN PROGRESS`

### Tasks
- [x] Replace operator-style "Issue token" flow with channel-centric voice controls for `voice` channels.
- [x] Add header actions for `Join Voice`, `Leave`, `Mute/Unmute Mic`.
- [x] On join, request token from backend with voice-first sources (`microphone`) and connect via RTC wrapper.
- [x] Show call connection state (`connecting`, `connected`, `reconnecting`, `error`) and actionable error messages.
- [x] Display in-call participant roster from LiveKit participant events.
- [ ] Implement VAD/active-speaker state from LiveKit audio-level events with debounce/hysteresis to avoid flicker.
- [ ] Highlight currently speaking participant names in green in the in-call participant roster.
- [ ] Ensure channel switch/logout always leaves room and clears media state.
- [ ] Add UI tests for join/leave/mute state and API payload correctness.
- [ ] Add tests for active-speaker highlighting transitions (`idle -> speaking -> idle`).

### Exit Criteria
- User can join voice channel and publish microphone audio.
- User can leave cleanly and rejoin without stale state.
- Voice controls are hidden/disabled when channel kind or permissions disallow them.
- Speaking participants are visually highlighted in green with stable VAD behavior (no rapid flicker).
- Tests cover primary voice paths.

### Implementation Notes (Fill After Completion)
- Date completed:
- PR/commit:
- Files changed:
- Security-impact notes:
- Tests run:
- Follow-ups/debt:

### Handoff To Next Phase
- Keep voice call state stable while layering settings UX for audio-device control.

## Phase 4 - Settings Panel Foundation + Voice Settings
### Goal
Add a baseline settings panel with left-rail categories and a `Voice` submenu containing audio device settings.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add a global Settings entry point in app shell (gear/action) with open/close behavior.
- [ ] Build settings panel layout with left rail for categories and right content pane.
- [ ] Add baseline categories structure in rail, including `Voice` and `Profile` (profile as placeholder only in this plan).
- [ ] Implement `Voice -> Audio Devices` submenu page.
- [ ] Add audio input/output device selectors (microphone, speaker) using browser media-device enumeration with strict error handling.
- [ ] Add safe defaults and persistence for selected device IDs in local client state/storage.
- [ ] Wire selected devices into RTC flow state (without changing backend permission model).
- [ ] Add UI tests for panel navigation, `Voice` submenu selection, and device selector behavior with mocked media devices.

### Exit Criteria
- Settings panel opens from app shell and renders category rail + content pane.
- `Voice -> Audio Devices` page exists and allows selecting microphone/speaker devices.
- Device selections persist and are reapplied on reload when devices are still available.
- Profile appears only as non-functional placeholder/navigation stub for future plan scope.
- Tests cover settings navigation and audio-device selection flows.

### Implementation Notes (Fill After Completion)
- Date completed:
- PR/commit:
- Files changed:
- Security-impact notes:
- Tests run:
- Follow-ups/debt:

### Handoff To Next Phase
- Reuse settings-managed audio device preferences for stream publish UX.

## Phase 5 - Streams On Voice Channels (Video + Screen Share)
### Goal
Enable camera and screen share controls within voice channels, with permission-aware behavior.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add `Camera On/Off` and `Share Screen/Stop Share` controls for active calls.
- [ ] Request publish sources based on desired capabilities and backend grants.
- [ ] Render remote video tiles and local preview, with bounded visible stream count (client DoS guardrail).
- [ ] Surface permission-denied states clearly when camera/screen grants are not allowed.
- [ ] Keep stream controls capability-based on top of voice; do not add a separate video-channel execution path.
- [ ] Add tests for source request mapping, permission-clamped UI, and tile rendering fallback behavior.

### Exit Criteria
- Camera and screen share can be started/stopped for authorized users.
- Unauthorized publish attempts do not expose enabled controls.
- Voice call behavior remains functional when stream permissions are denied.
- UI remains stable when remote participants publish/unpublish frequently.

### Implementation Notes (Fill After Completion)
- Date completed:
- PR/commit:
- Files changed:
- Security-impact notes:
- Tests run:
- Follow-ups/debt:

### Handoff To Next Phase
- Preserve media control state model for final hardening/docs phase.

## Phase 6 - RTC Hardening, Environment, and Release Gates
### Goal
Close operational/security gaps and ship with clear runbook + verification.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Verify `FILAMENT_LIVEKIT_URL` guidance and defaults so clients receive a reachable signaling URL in local/dev/prod.
- [ ] Add explicit troubleshooting UX for token expiry, connection failure, and permission rejection.
- [ ] Add/extend docs (`docs/API.md`, `docs/DEPLOY.md`, `README.md`) for channel kinds and RTC UX behavior.
- [ ] Add regression tests for reconnect/disconnect flows and permission edge-cases.
- [ ] Run and record required gates for touched areas (web tests, relevant Rust tests, fmt/lint as applicable).
- [ ] Update `PLAN_UX.md` and this file with completion notes.

### Exit Criteria
- Local/dev environment can perform a full voice+video call flow with documented setup.
- Security controls remain intact (no limit/cap/CSP relaxations).
- Documentation reflects final behavior and known limitations.

### Implementation Notes (Fill After Completion)
- Date completed:
- PR/commit:
- Files changed:
- Security-impact notes:
- Tests run:
- Follow-ups/debt:

### Handoff / Next Backlog (Post-Plan)
- Server-driven cross-client RTC occupancy indicators in channel list.
- Active-speaker events surfaced in gateway for non-joined observers.
- Desktop client RTC parity after web UX stabilizes.
