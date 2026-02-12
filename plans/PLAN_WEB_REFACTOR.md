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
`NOT STARTED`

### Tasks
- [ ] Create `apps/filament-client-web/src/features/app-shell/config/ui-constants.ts` for:
  - icon URLs
  - scroll/picker threshold constants
  - RTC disconnected snapshot default
- [ ] Create `apps/filament-client-web/src/features/app-shell/config/reaction-options.ts` for `OPENMOJI_REACTION_OPTIONS`.
- [ ] Create `apps/filament-client-web/src/features/app-shell/config/settings-menu.ts` for settings category/submenu constants.
- [ ] Replace in-page constant declarations with imports.

### Tests
- [ ] Add `apps/filament-client-web/tests/app-shell-config.test.ts` for:
  - reaction option integrity (non-empty labels, unique emoji)
  - settings menu IDs are unique and expected defaults exist

### Exit Criteria
- `AppShellPage.tsx` no longer contains large static tables/constant blocks.

---

## Phase 2 - Extract State Slices
### Goal
Move raw signal initialization into typed state factories.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add `apps/filament-client-web/src/features/app-shell/state/` modules by slice:
  - `workspace-state.ts`
  - `message-state.ts`
  - `voice-state.ts`
  - `profile-state.ts`
  - `overlay-state.ts`
  - `diagnostics-state.ts`
- [ ] Each module exports `createXState()` returning grouped accessors/setters.
- [ ] Keep defaults and invariant-safe initial values unchanged.
- [ ] Replace flat signal declarations in-page with slice initializers.

### Tests
- [ ] Add `apps/filament-client-web/tests/app-shell-state.test.ts` for default state shape and key defaults.

### Exit Criteria
- Signal declarations are grouped by domain slice and no longer in one large page block.

---

## Phase 3 - Extract Derived Selectors/View Models
### Goal
Move derived `createMemo` logic and pure derivations out of the page.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add `apps/filament-client-web/src/features/app-shell/selectors/create-app-shell-selectors.ts`.
- [ ] Move derived access control booleans and channel/workspace selectors.
- [ ] Move voice roster derivation and voice permission hint derivation into pure helpers.
- [ ] Keep memo construction in one selector factory to preserve reactivity.

### Tests
- [ ] Add `apps/filament-client-web/tests/app-shell-selectors.test.ts` for:
  - permission-derived flags
  - voice roster synthesis
  - active workspace/channel selection behavior

### Exit Criteria
- Most `createMemo` declarations are outside `AppShellPage.tsx`.

---

## Phase 4 - Extract UI Mechanics Controllers
### Goal
Move DOM/paint/listener mechanics out of page body.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add controllers:
  - `controllers/message-list-controller.ts` (scroll helpers, load-older button visibility, sticky bottom)
  - `controllers/reaction-picker-controller.ts` (overlay positioning + global listeners)
  - `controllers/profile-overlay-controller.ts` (Escape handling)
- [ ] Keep controller APIs consistent with existing style (`Accessor`/`Setter` options object).
- [ ] Maintain deterministic cleanup via `onCleanup`.

### Tests
- [ ] Add controller-focused tests:
  - `apps/filament-client-web/tests/app-shell-message-list-controller.test.ts`
  - `apps/filament-client-web/tests/app-shell-reaction-picker-controller.test.ts`

### Exit Criteria
- Scroll math and reaction-picker positioning/listener logic are no longer inline in the page.

---

## Phase 5 - Extract Data/Boundary Controllers
### Goal
Move async resource orchestration and boundary fetch flows into domain-specific controllers.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add controllers:
  - `controllers/profile-controller.ts` (fetchMe, fetchUserProfile, profile save/avatar upload)
  - `controllers/friendship-controller.ts` (friend list + requests + actions)
  - `controllers/public-directory-controller.ts` (public guild search/load)
  - `controllers/identity-resolution-controller.ts` (username cache/resolve flows)
  - `controllers/message-history-controller.ts` (refresh/load older + reset on channel/permission changes)
  - `controllers/gateway-controller.ts` (connect/cleanup presence + message events)
- [ ] Ensure each controller preserves:
  - auth-null reset behavior
  - cancellation guards
  - bounded, deterministic state transitions

### Tests
- [ ] Add targeted controller tests for success/error/cancellation paths.
- [ ] Update existing `app-shell-*` integration-like tests to ensure no behavior drift.

### Exit Criteria
- Most `createEffect` and async fetch orchestration leave `AppShellPage.tsx`.

---

## Phase 6 - Extract Voice Session Operations
### Goal
Fully isolate voice operational handlers from page orchestration.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add `controllers/voice-operations-controller.ts` for:
  - `joinVoiceChannel`
  - `leaveVoiceChannel`
  - mic/camera/screen-share toggles
  - `ensureRtcClient` / `releaseRtcClient` lifecycle
- [ ] Keep existing lifecycle controller (`createVoiceSessionLifecycleController`) and integrate with the new operations controller.
- [ ] Preserve permission checks and least-privilege token request construction.

### Tests
- [ ] Extend `apps/filament-client-web/tests/app-shell-voice-controller.test.ts` and `apps/filament-client-web/tests/app-shell-voice-controls.test.tsx` for:
  - teardown guarantees
  - denied publish-source paths
  - device preference application behavior

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
