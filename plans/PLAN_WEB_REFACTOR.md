# PLAN_WEB_REFACTOR.md

## Objective
Refactor `apps/filament-client-web/src/pages/AppShellPage.tsx` into smaller, testable, security-preserving modules while keeping behavior stable.

## Code-State Audit (2026-02-12)
- `AppShellPage.tsx` is currently `2455` lines.
- Local reactive/state density is very high:
  - `109` `createSignal(...)` calls
  - `32` `createMemo(...)` calls
  - `16` `createEffect(...)` calls
  - `26` local `async` handlers
- The page is currently both:
  - Composition root (layout + panel wiring)
  - Domain orchestrator (message history, gateway, voice RTC lifecycle, profile/friends/directory, settings/device ops)
- Some extraction is already in place (controllers/components/helpers), but large orchestration and prop-assembly blocks remain in-page.

## Refactor Constraints (Locked)
- No behavior regressions in auth, permissions, message flow, or RTC lifecycle.
- Preserve hostile-server assumptions and current validation boundaries.
- Keep safe markdown rendering path only (`SafeMarkdown`), no HTML rendering path.
- Do not relax limits, timeouts, or permission checks.
- Every phase must ship with tests.

## Target End State
`AppShellPage.tsx` becomes a thin composition root that:
- Instantiates high-level state/hook modules.
- Wires top-level components.
- Delegates side effects and async operations to dedicated controllers.
- Delegates large prop-object assembly to typed adapters/builders.

Target size after completion: `<= 650` lines for `AppShellPage.tsx`.

## Status Legend
- `NOT STARTED`
- `IN PROGRESS`
- `DONE`
- `BLOCKED`

## Phase 0 - Safety Net + Baseline Metrics
### Goal
Freeze current behavior so later extraction can move quickly with low regression risk.

### Completion Status
`DONE`

### Tasks
- [x] Add/expand characterization tests for:
  - message list behavior (refresh, pagination, reaction picker open/close)
  - overlay panel open/close and permission-based visibility
  - voice join/leave/toggle flows and teardown on logout
  - profile modal open/close and load/error rendering
- [x] Record baseline metrics in this plan:
  - line count for `AppShellPage.tsx`
  - test command and pass status

### Baseline Metrics (2026-02-12)
- `AppShellPage.tsx` line count: `2454` (`apps/filament-client-web/src/pages/AppShellPage.tsx`)
- Test command: `pnpm test` (run from `apps/filament-client-web`)
- Pass status: `28` test files passed, `127` tests passed

### Refactor Notes
- Added characterization coverage for refresh/pagination/reaction-picker close, overlay permission gating/open-close, voice lifecycle flows, and profile modal loading/error/close paths.
- Hardened selected-profile fetch handling in `AppShellPage.tsx` so profile lookup failures are mapped to deterministic UI error state without unhandled promise rejections.

### Exit Criteria
- Existing behavior is covered by focused tests before any structural extraction.

---

## Phase 1 - Extract Static Config and Constants
### Goal
Move non-reactive constants out of the page.

### Completion Status
`DONE`

### Tasks
- [x] Create `apps/filament-client-web/src/features/app-shell/config/ui-constants.ts` for:
  - icon URLs
  - scroll/picker threshold constants
  - RTC disconnected snapshot default
- [x] Create `apps/filament-client-web/src/features/app-shell/config/reaction-options.ts` for `OPENMOJI_REACTION_OPTIONS`.
- [x] Create `apps/filament-client-web/src/features/app-shell/config/settings-menu.ts` for settings category/submenu constants.
- [x] Replace in-page constant declarations with imports.

### Tests
- [x] Add `apps/filament-client-web/tests/app-shell-config.test.ts` for:
  - reaction option integrity (non-empty labels, unique emoji)
  - settings menu IDs are unique and expected defaults exist

### Refactor Notes
- Moved static icon URLs, scroll/reaction-picker sizing thresholds, and the RTC disconnected snapshot to `features/app-shell/config/ui-constants.ts`.
- Moved reaction picker options to `features/app-shell/config/reaction-options.ts`.
- Added `features/app-shell/config/settings-menu.ts` for settings categories/submenu plus exported default selections, and reused those defaults in both `AppShellPage.tsx` and the overlay controller.
- Added `tests/app-shell-config.test.ts` to assert reaction option/menu invariant safety and default setting/menu alignment.

### Exit Criteria
- `AppShellPage.tsx` no longer contains large static tables/constant blocks.

---

## Phase 2 - Extract State Slices
### Goal
Move raw signal initialization into typed state factories.

### Completion Status
`DONE`

### Tasks
- [x] Add `apps/filament-client-web/src/features/app-shell/state/` modules by slice:
  - `workspace-state.ts`
  - `message-state.ts`
  - `voice-state.ts`
  - `profile-state.ts`
  - `overlay-state.ts`
  - `diagnostics-state.ts`
- [x] Each module exports `createXState()` returning grouped accessors/setters.
- [x] Keep defaults and invariant-safe initial values unchanged.
- [x] Replace flat signal declarations in-page with slice initializers.

### Tests
- [x] Add `apps/filament-client-web/tests/app-shell-state.test.ts` for default state shape and key defaults.

### Refactor Notes
- Added six state factories under `features/app-shell/state/` and moved all `createSignal(...)` initialization out of `AppShellPage.tsx`.
- Centralized `DEFAULT_VOICE_SESSION_CAPABILITIES` in `voice-state.ts` to keep voice defaults co-located with voice signal initialization.
- Added `tests/app-shell-state.test.ts` to lock key default values and ensure grouped accessor/setter state slices remain stable.

### Exit Criteria
- Signal declarations are grouped by domain slice and no longer in one large page block.

---

## Phase 3 - Extract Derived Selectors/View Models
### Goal
Move derived `createMemo` logic and pure derivations out of the page.

### Completion Status
`DONE`

### Tasks
- [x] Add `apps/filament-client-web/src/features/app-shell/selectors/create-app-shell-selectors.ts`.
- [x] Move derived access control booleans and channel/workspace selectors.
- [x] Move voice roster derivation and voice permission hint derivation into pure helpers.
- [x] Keep memo construction in one selector factory to preserve reactivity.

### Tests
- [x] Add `apps/filament-client-web/tests/app-shell-selectors.test.ts` for:
  - permission-derived flags
  - voice roster synthesis
  - active workspace/channel selection behavior

### Refactor Notes
- Added `features/app-shell/selectors/create-app-shell-selectors.ts` and moved the page-level derived selectors into a single memo factory that preserves existing `Accessor` usage at call sites.
- Added pure helpers in the selector module for voice roster synthesis and voice stream permission hint derivation, then consumed those helpers from selector memos.
- Updated `AppShellPage.tsx` to wire selector outputs from `createAppShellSelectors(...)` and removed inline `createMemo(...)` declarations.
- Added `tests/app-shell-selectors.test.ts` to lock permission flags, workspace/channel selection behavior, and voice roster/hint derivation parity.
- Metrics after Phase 3 (2026-02-12):
  - `AppShellPage.tsx` line count: `2226`
  - `createMemo(...)` count in `AppShellPage.tsx`: `0`
  - Test command: `pnpm --prefix apps/filament-client-web test`
  - Pass status: `31` test files passed, `136` tests passed

### Exit Criteria
- Most `createMemo` declarations are outside `AppShellPage.tsx`.

---

## Phase 4 - Extract UI Mechanics Controllers
### Goal
Move DOM/paint/listener mechanics out of page body.

### Completion Status
`DONE`

### Tasks
- [x] Add controllers:
  - `controllers/message-list-controller.ts` (scroll helpers, load-older button visibility, sticky bottom)
  - `controllers/reaction-picker-controller.ts` (overlay positioning + global listeners)
  - `controllers/profile-overlay-controller.ts` (Escape handling)
- [x] Keep controller APIs consistent with existing style (`Accessor`/`Setter` options object).
- [x] Maintain deterministic cleanup via `onCleanup`.

### Tests
- [x] Add controller-focused tests:
  - `apps/filament-client-web/tests/app-shell-message-list-controller.test.ts`
  - `apps/filament-client-web/tests/app-shell-reaction-picker-controller.test.ts`

### Refactor Notes
- Added `features/app-shell/controllers/message-list-controller.ts` and moved message-list paint scheduling, sticky-bottom detection, load-older visibility, and autoload-on-scroll mechanics behind a typed controller.
- Added `features/app-shell/controllers/reaction-picker-controller.ts` and moved reaction picker overlay placement + global resize/scroll/keydown/pointer listeners out of `AppShellPage.tsx` with deterministic listener cleanup.
- Added `features/app-shell/controllers/profile-overlay-controller.ts` and moved selected-profile Escape handling into a dedicated controller.
- Rewired `AppShellPage.tsx` to consume controller APIs for message list and reaction picker mechanics; removed inline DOM/listener math and related local refs.
- Added `tests/app-shell-message-list-controller.test.ts` and `tests/app-shell-reaction-picker-controller.test.ts` for controller behavior and positioning/listener parity.
- Metrics after Phase 4 (2026-02-12):
  - `AppShellPage.tsx` line count: `2053`
  - `createEffect(...)` count in `AppShellPage.tsx`: `11`
  - Test command: `pnpm --prefix apps/filament-client-web test`
  - Pass status: `33` test files passed, `141` tests passed

### Exit Criteria
- Scroll math and reaction-picker positioning/listener logic are no longer inline in the page.

---

## Phase 5 - Extract Data/Boundary Controllers
### Goal
Move async resource orchestration and boundary fetch flows into domain-specific controllers.

### Completion Status
`DONE`

### Tasks
- [x] Add controllers:
  - `controllers/profile-controller.ts` (fetchMe, fetchUserProfile, profile save/avatar upload)
  - `controllers/friendship-controller.ts` (friend list + requests + actions)
  - `controllers/public-directory-controller.ts` (public guild search/load)
  - `controllers/identity-resolution-controller.ts` (username cache/resolve flows)
  - `controllers/message-history-controller.ts` (refresh/load older + reset on channel/permission changes)
  - `controllers/gateway-controller.ts` (connect/cleanup presence + message events)
- [x] Ensure each controller preserves:
  - auth-null reset behavior
  - cancellation guards
  - bounded, deterministic state transitions

### Tests
- [x] Add targeted controller tests for success/error/cancellation paths.
- [x] Update existing `app-shell-*` integration-like tests to ensure no behavior drift.

### Refactor Notes
- Added six boundary/data controllers under `features/app-shell/controllers/`:
  - `profile-controller.ts`
  - `friendship-controller.ts`
  - `public-directory-controller.ts`
  - `identity-resolution-controller.ts`
  - `message-history-controller.ts`
  - `gateway-controller.ts`
- Rewired `AppShellPage.tsx` to consume the new controller APIs and removed inline async orchestration for profile resources/settings, friendship/public-directory flows, username resolution, message history refresh/pagination/reset, and gateway session wiring.
- Added controller-focused tests:
  - `tests/app-shell-profile-controller.test.ts`
  - `tests/app-shell-friendship-controller.test.ts`
  - `tests/app-shell-public-directory-controller.test.ts`
  - `tests/app-shell-identity-resolution-controller.test.ts`
  - `tests/app-shell-message-history-controller.test.ts`
  - `tests/app-shell-gateway-controller.test.ts`
- Extended integration-like coverage in `tests/app-shell-friendships.test.tsx` with outgoing friend-request submission/refresh assertions.
- Tightened test typing and cleanup contracts by correcting `tests/app-shell-reaction-picker-controller.test.ts` dispose typing and aligning new controller tests to branded domain IDs.
- Metrics after Phase 5 (2026-02-12):
  - `AppShellPage.tsx` line count: `1655`
  - `createEffect(...)` count in `AppShellPage.tsx`: `3`
  - Test command: `pnpm --prefix apps/filament-client-web test`
  - Pass status: `39` test files passed, `154` tests passed
  - Typecheck command: `pnpm --prefix apps/filament-client-web typecheck`
  - Typecheck status: pass

### Exit Criteria
- Most `createEffect` and async fetch orchestration leave `AppShellPage.tsx`.

---

## Phase 6 - Extract Voice Session Operations
### Goal
Fully isolate voice operational handlers from page orchestration.

### Completion Status
`DONE`

### Tasks
- [x] Add `controllers/voice-operations-controller.ts` for:
  - `joinVoiceChannel`
  - `leaveVoiceChannel`
  - mic/camera/screen-share toggles
  - `ensureRtcClient` / `releaseRtcClient` lifecycle
- [x] Keep existing lifecycle controller (`createVoiceSessionLifecycleController`) and integrate with the new operations controller.
- [x] Preserve permission checks and least-privilege token request construction.

### Tests
- [x] Extend `apps/filament-client-web/tests/app-shell-voice-controller.test.ts` and `apps/filament-client-web/tests/app-shell-voice-controls.test.tsx` for:
  - teardown guarantees
  - denied publish-source paths
  - device preference application behavior

### Refactor Notes
- Added `features/app-shell/controllers/voice-operations-controller.ts` and moved inline voice operations from `AppShellPage.tsx` into a dedicated controller: RTC client lifecycle (`ensureRtcClient`/`releaseRtcClient`), voice join/leave, and mic/camera/screen-share toggles.
- Preserved existing least-privilege token request behavior by building requested publish sources from effective channel permissions and continuing to scope requests to microphone plus only explicitly-allowed optional sources.
- Kept lifecycle parity by wiring `createVoiceSessionLifecycleController` to the extracted `leaveVoiceChannel` operation and retaining deterministic local teardown behavior even when RTC leave/destroy transport calls fail.
- Extended `tests/app-shell-voice-controller.test.ts` with controller-level checks for teardown failure handling, denied publish-source toggle paths, and device-preference application during join.
- Extended `tests/app-shell-voice-controls.test.tsx` with logout teardown regression coverage when RTC leave/destroy rejects, validating local auth/session cleanup still completes.
- Metrics after Phase 6 (2026-02-12):
  - `AppShellPage.tsx` line count: `1510`
  - `createEffect(...)` count in `AppShellPage.tsx`: `3`
  - Test command: `pnpm --prefix apps/filament-client-web test`
  - Pass status: `39` test files passed, `158` tests passed
  - Typecheck command: `pnpm --prefix apps/filament-client-web typecheck`
  - Typecheck status: pass

### Exit Criteria
- Voice command handlers are no longer implemented inline in `AppShellPage.tsx`.

---

## Phase 7 - Extract Composition Components + Panel Prop Adapters
### Goal
Shrink JSX and giant prop-object wiring by moving composition and adapters to dedicated modules.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add composition components:
  - `components/layout/AppShellLayout.tsx`
  - `components/layout/ChatColumn.tsx`
  - `components/overlays/UserProfileOverlay.tsx`
- [ ] Add typed panel prop adapter module:
  - `adapters/panel-host-props.ts` to build `PanelHost` prop groups from state/actions.
- [ ] Keep `AppShellPage.tsx` responsible only for wiring composed modules.

### Tests
- [ ] Add/update UI tests validating:
  - panel wiring still triggers correct handlers
  - profile overlay interaction parity
  - rail collapse and chat rendering parity

### Exit Criteria
- Return JSX in `AppShellPage.tsx` is substantially reduced and no longer contains huge inline prop-object definitions.

---

## Phase 8 - Cleanup, Documentation, and Enforcement
### Goal
Finalize the refactor with cleanup and guardrails.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Remove dead code and stale imports created during extractions.
- [ ] Update `plans/PLAN_UX.md` progress log with refactor milestones.
- [ ] Record final size metrics and compare against target.
- [ ] Optional: add a lightweight CI check/script to warn if `AppShellPage.tsx` exceeds agreed line threshold.

### Validation Gate
- [ ] `npm --prefix apps/filament-client-web test`
- [ ] `npm --prefix apps/filament-client-web run build`

### Exit Criteria
- Refactor complete, tests green, no functional regressions observed.

---

## Suggested Execution Cadence
- Land phases in small PRs (1 phase per PR).
- Prefer extraction-only commits first, behavior-adjustment commits second (if needed).
- Re-run the high-risk suites after each phase:
  - `app-shell-voice-controls.test.tsx`
  - `app-shell-message-history-scroll.test.tsx`
  - `app-shell-settings-entry.test.tsx`
  - `app-shell-operator-permissions.test.tsx`

## Risks and Mitigations
- Reactivity regressions from moving signals/memos.
  - Mitigation: keep `Accessor`/`Setter` signatures explicit and test derived behavior.
- Cleanup/listener leaks during effect extraction.
  - Mitigation: dedicated controller tests for `onCleanup` behavior.
- Panel wiring breakage from prop adapter extraction.
  - Mitigation: add explicit UI tests for each panel entry path.
