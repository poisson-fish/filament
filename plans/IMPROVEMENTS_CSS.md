# IMPROVEMENTS_CSS.md

## Purpose
Define a safe, incremental plan to migrate `apps/filament-client-web` from hand-rolled global CSS to UnoCSS, while fixing chat layout behavior so:
- message timeline visually stacks from bottom to top
- composer/message bar remains pinned to the bottom of the chat viewport

## Direction (Locked)
- Styling engine: `UnoCSS`
- Migration style: incremental by surface area, not big-bang rewrite
- Keep security posture from `AGENTS.md` unchanged (no HTML rendering paths, no unsafe runtime style/script injection)

## Current Baseline (2026-02-18)
- Stylesheet manifest: `apps/filament-client-web/src/styles/app.css`
- Main CSS files:
  - `apps/filament-client-web/src/styles/app/base.css` (`1332` lines)
  - `apps/filament-client-web/src/styles/app/shell-refresh.css` (`1964` lines)
- Approx footprint:
  - `3301` CSS lines total
  - `476` class selectors
  - `340` TSX class usages
- Layout today:
  - heavy global cascade
  - chat behavior partly implemented via CSS + message window logic

## Migration Goals
1. Replace global cascade styling with UnoCSS utilities and reusable shortcuts/rules.
2. Preserve visual parity first, then improve UX polish.
3. Fix chat timeline/composer behavior with deterministic layout semantics.
4. Remove dead legacy CSS safely with tests and staged cleanup.

## Architecture Target
- UnoCSS utilities in TSX for layout/spacing/state.
- Central design tokens preserved as CSS variables (color, spacing, radii, elevation, motion).
- UnoCSS config contains:
  - `theme` token mapping
  - `shortcuts` for repeated UI primitives (buttons, rails, panels, chips)
  - optional `variants` for state patterns
- Temporary `legacy.css` bridge exists only for unmigrated surfaces.

## UnoCSS Class Conventions (Phase 0)
- Utility-first in component `class` attributes for local layout/state.
- Use `fx-*` shortcut prefix for reusable semantic primitives.
- Keep shortcuts composable and low-level (panel/button/chip primitives), not feature-specific.
- Prefer token aliases (`bg-bg-2`, `text-ink-1`, `border-line`) over raw color literals in TSX.
- Keep legacy CSS loaded during migration and delete selectors only when the owning surface is fully migrated.

## Chat Layout Fix Spec (Required)

### Spec A: Bottom-Anchored Timeline
- Newest message should appear nearest composer (bottom of scroll viewport).
- When user is at latest and a new message arrives:
  - keep viewport pinned to bottom without jump.
- When user scrolls up (history mode):
  - do not force snap to bottom on incoming messages.

### Spec B: Composer Pinned to Bottom
- Composer stays attached to bottom edge of chat panel/viewport area.
- Message list scroll area occupies remaining vertical space above composer.
- Keyboard/input growth must not push composer off-screen.

### Spec C: Load Older Behavior
- “Load older” / history pagination preserves scroll anchor.
- No visible jump after prepending older messages.

## Work Plan

## Phase 0 - UnoCSS Tooling and Guardrails
Status: `IN PROGRESS`

Tasks:
- [x] Install UnoCSS packages in `apps/filament-client-web`.
- [x] Add `uno.config.ts` with token mappings and initial shortcuts.
- [x] Wire UnoCSS plugin into Vite config.
- [x] Keep existing CSS imports active for parity.
- [x] Add migration doc section for class conventions and shortcut naming.

Exit Criteria:
- `dev`, `build`, `test`, and `typecheck` all pass unchanged.
- No unintended visual diffs.

Implementation Notes (2026-02-18):
- UnoCSS is wired via `unocss/vite`, and generated CSS is imported in `src/main.tsx` before legacy `app.css` to avoid cascade regressions.
- Validation status:
  - `build` passes.
  - `typecheck` currently fails on existing test typing issues in `tests/app-shell-identity-resolution-controller.test.ts` and `tests/app-shell-selectors.test.ts`.
  - `test` currently has an existing failing case in `tests/app-shell-message-history-scroll.test.tsx` (chronological ordering assertion).
- Dependency lockfiles:
  - Repo currently tracks `apps/filament-client-web/package-lock.json`.
  - `pnpm-lock.yaml` is intentionally not part of this migration slice to avoid mixed lockfile governance.

## Phase 1 - Token Normalization
Status: `COMPLETED`

Tasks:
- [x] Consolidate design tokens under one source of truth in existing CSS variables.
- [x] Map tokens into UnoCSS theme aliases.
- [x] Add rules: avoid raw hex values in migrated TSX.

Exit Criteria:
- Token map documented and consumed by UnoCSS config.
- New migrated components use tokens/aliases only.

Implementation Notes (2026-02-18):
- Token declarations were moved from `src/styles/app/base.css` into a dedicated `src/styles/app/tokens.css`, and `src/styles/app.css` now imports tokens first to lock manifest order.
- UnoCSS theme aliases now include the new danger surface tokens (`danger-panel`, `danger-panel-strong`, `danger-ink`) so migrated components can avoid raw literals.
- Important finding: there was still a raw hex inline danger style in `ChannelRail.tsx` (voice disconnect button). It is now tokenized, and a new test (`tests/app-style-token-manifest.test.ts`) guards import order, token definitions, and this token usage path.
- Important finding: current TS/TSX sources do not contain raw hex/rgb/hsl literals, so the new migrated-surface guard can remain strict without introducing compatibility exemptions.
- `tests/app-style-token-manifest.test.ts` now includes a migrated-TSX rule that rejects raw color literals in migrated files (starting with `ChannelRail.tsx`); additional migrated components should be added to that allowlist as migration continues.

## Phase 2 - Chat Layout Behavior Fix (Before Full Rewrite)
Status: `COMPLETED`

Scope:
- `apps/filament-client-web/src/features/app-shell/components/messages/MessageList.tsx`
- `apps/filament-client-web/src/features/app-shell/components/messages/MessageComposer.tsx`
- related shell container layout in chat panel components/styles
- message list controller/window logic as needed

Tasks:
- Implement explicit chat panel structure:
  - parent: fixed-height flex/grid container
  - list region: `min-h-0` scrollable area
  - composer: non-scrolling bottom region
- Align render order + scroll anchoring logic to Spec A/B/C.
- Keep existing bounded/full render-window behavior intact.
- [x] Normalize message list DOM flow to chronological order (oldest -> newest) and keep `Load older messages` anchored before rows.
- [x] Add targeted incoming-message scroll tests for pinned-to-latest and history-mode behavior (Spec A).
- [x] Add/expand tests for load-older anchor preservation.
- [x] Add/expand tests for composer bottom pinning.

Exit Criteria:
- Chat behavior matches Spec A/B/C.
- No regressions in existing message list/controller tests.

Implementation Notes (2026-02-18):
- Important finding: the previous list implementation combined `flex-direction: column-reverse` with an in-memory `reverse()` of the rendered message window. That made DOM order diverge from normalized chronology and caused the existing history refresh ordering test to fail.
- Applied slice:
  - `MessageList.tsx` now renders chronological rows directly and places the `Load older messages` affordance before message rows.
  - `shell-refresh.css` uses `.message-list { flex-direction: column; }` to keep scroll math in standard top-to-bottom semantics while preserving bottom stickiness behavior via existing controller logic.
  - Added test coverage in `tests/app-shell-message-list.test.tsx` to guard that load-older controls remain anchored before message rows in the normalized DOM flow.
- Applied slice (2026-02-18, later):
  - Added `tests/app-shell-gateway-controller.test.ts` coverage for the history-mode branch where incoming gateway messages must merge into state without forcing a scroll-to-bottom snap.
- Applied slice (2026-02-18, load-older anchor):
  - Extended `tests/app-shell-message-history-scroll.test.tsx` with an integration assertion that prepending older history preserves viewport anchor by restoring `scrollTop` with the `scrollHeight` delta.
  - Extended the test scroll metric harness to allow controlled `scrollHeight` growth during pagination, so Spec C is validated against the real controller path instead of only unit-level mocks.
- Applied slice (2026-02-18, composer bottom pinning):
  - Expanded `tests/app-shell-layout-components.test.tsx` so `ChatColumn` coverage now asserts `.chat-panel > .chat-body` and `.chat-panel > .composer` remain sibling regions, with composer attachment rows confined to the composer subtree and excluded from the scrollable body subtree.
  - Added `tests/app-shell-chat-layout-contract.test.ts` (node-environment stylesheet contract checks) to lock the CSS semantics that enforce Spec B: `.chat-panel` grid rows (`auto 1fr auto`), `.chat-body` containment, `.chat-body .message-list` as the sole vertical scroller, and direct-child composer constraints.
- Important finding:
  - Existing gateway tests only asserted pinned-to-latest auto-scroll behavior, which left Spec A's history-mode branch unguarded; this now has explicit coverage.
- Important finding:
  - `tests/app-shell-message-list-controller.test.ts` already covered delta-based scroll restoration at the unit level, but there was no integration proof that the App path preserved anchor after async history prepend. The new spec-level integration test now closes that gap.
- Important finding:
  - `shell-refresh.css` contains multiple `.chat-panel` selector blocks for different concerns; layout contract tests must match declarations semantically rather than assuming the first block is the layout block.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-message-history-scroll.test.tsx` passes (`580` tests total in run), including the new load-older anchor preservation coverage.
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-layout-components.test.tsx tests/app-shell-chat-layout-contract.test.ts` passes (`582` tests total in run), including composer bottom pinning structure and stylesheet contract assertions.
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated test typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).

## Phase 3 - Chat Surface UnoCSS Migration
Status: `COMPLETED`
Completion status (2026-02-18): `4/4 scoped surfaces migrated` (`MessageList`, `MessageComposer`, `ReactionPickerPortal`, `MessageRow` complete)

Scope:
- `MessageList.tsx`
- `MessageRow.tsx`
- `MessageComposer.tsx`
- `ReactionPickerPortal.tsx`

Tasks:
- Replace message-surface legacy classes with Uno utilities/shortcuts.
- Remove corresponding migrated selectors from `shell-refresh.css`.
- Keep behavior and accessibility parity (focus, hover, disabled, error).

Exit Criteria:
- Chat surface fully migrated to UnoCSS.
- Legacy CSS reduced for chat area by at least 25% from baseline.

Implementation Notes (2026-02-18):
- Applied slice (MessageList surface):
  - Migrated `MessageList.tsx` list container spacing/scrolling styles to Uno utility classes while retaining the `.message-list` hook for scroll/controller logic and existing tests.
  - Migrated the `Load older messages` button to Uno utility classes and removed the legacy `.load-older` selector dependency.
  - Removed migrated `.message-list` style blocks from `shell-refresh.css`, including the mobile padding override now represented in Uno responsive utilities.
- Important finding:
  - `shell-refresh.css` had two separate top-level `.message-list` blocks plus a mobile override; all three had to be removed together to avoid partial cascade leftovers during incremental migration.
- Important finding:
  - The legacy `.load-older` selector was grouped with attachment/message action controls; migration required removing only the `.load-older` arm to avoid regressions in attachment control styling.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-message-list.test.tsx tests/app-shell-chat-layout-contract.test.ts` passes (`583` tests total in run), including updated load-older DOM-order assertions and message-list utility class checks.
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated test typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (MessageComposer surface):
  - Migrated `MessageComposer.tsx` internal layout/controls/attachment pills to Uno utility classes and retained only stable runtime hooks (`form.composer`, `.composer-file-input`) used by existing controllers/integration tests.
  - Removed migrated composer-internal selectors from both `src/styles/app/shell-refresh.css` and `src/styles/app/base.css`, including the previous broad `.composer input` and `.composer button` cascade rules.
  - Added `tests/app-shell-message-composer.test.tsx` to lock Uno class usage and composer behavior (disabled-state gating, attachment pill removal callbacks).
  - Extended `tests/app-shell-chat-layout-contract.test.ts` with a regression guard that composer internals no longer depend on legacy `shell-refresh.css` selectors.
  - Extended `tests/app-style-token-manifest.test.ts` migrated-surface guard to include `MessageComposer.tsx`.
- Important finding:
  - Keeping `.composer` for layout contracts while removing only internal selectors is the safest incremental step; previous generic `.composer input/button` legacy rules in `base.css` would otherwise override Uno utility declarations due to selector specificity.
- Applied slice (ReactionPickerPortal surface):
  - Migrated `ReactionPickerPortal.tsx` panel/header/grid/option styling to Uno utility classes while retaining only the `.reaction-picker-floating` runtime hook used by the reaction picker controller’s outside-click handling.
  - Removed migrated `reaction-picker*` selectors from `src/styles/app/base.css` so the portal no longer depends on legacy cascade styles.
  - Added `tests/app-shell-reaction-picker-portal.test.tsx` to lock utility-class rendering and interaction behavior (add reaction + close).
  - Extended `tests/app-style-token-manifest.test.ts` migrated-surface guard to include `ReactionPickerPortal.tsx` and to assert legacy reaction picker selectors remain removed from `base.css`.
- Important finding:
  - The controller contract depends on `.reaction-picker-floating` as a query selector boundary, so that class must remain as a stable non-visual hook even after utility migration.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-reaction-picker-portal.test.tsx tests/app-shell-reactions.test.tsx tests/app-shell-reaction-picker-controller.test.ts tests/app-style-token-manifest.test.ts` passes (`590` tests total in run), including the new portal utility/behavior assertions.
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated test typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (MessageRow surface):
  - Migrated `MessageRow.tsx` avatar/meta/content/edit state/attachment cards/reaction chips/hover actions to Uno utility classes while retaining only stable runtime hooks (`.message-row`, `.message-tokenized`, `.message-hover-actions`) needed by existing controller and integration contracts.
  - Removed migrated MessageRow selectors from `src/styles/app/base.css` and `src/styles/app/shell-refresh.css`, including duplicate legacy cascade blocks and stale MessageRow-specific overrides.
  - Added `tests/app-shell-message-row.test.tsx` to lock utility class rendering and key interaction behavior (profile open, reaction toggles, deleting busy state).
  - Extended `tests/app-style-token-manifest.test.ts` migrated-surface guard to include `MessageRow.tsx` and assert legacy MessageRow selectors remain removed from `base.css`/`shell-refresh.css`.
- Important finding:
  - `shell-refresh.css` still contained a trailing `@media (hover: none)` fallback for `.message-main`; because `MessageRow` no longer renders that class, touch-padding behavior had to move into the component via `[@media(hover:none)]:pr-[4.5rem]` on the message content container.
- Important finding:
  - Keeping `.message-hover-actions` as a stable non-visual hook is still useful for touch-device fallback (`@media (hover: none)` forced visibility), even after hover panel visuals moved to Uno utilities.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-message-row.test.tsx tests/app-style-token-manifest.test.ts` passes (`593` tests total in run), including the new MessageRow migration assertions.
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated test typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).

## Phase 4 - Shell, Rails, Panels Migration
Status: `COMPLETED`
Completion status (2026-02-18): `7/7 scoped surfaces migrated` (`ServerRail`, `ChannelRail`, `MemberRail`, `ChatHeader`, `UserProfileOverlay`, `SettingsPanel`, `AuthShell` complete)

Scope:
- server rail, channel rail, member rail, header, overlays, settings panels, auth shell

Tasks:
- Migrate each surface in small PR slices.
- Use shortcuts for repeated panel/button/list patterns.
- Preserve responsive behavior and collapse modes.

Exit Criteria:
- Primary shell UI uses UnoCSS utilities/shortcuts.
- Legacy global selectors mostly removed.

Implementation Notes (2026-02-18):
- Applied slice (ServerRail surface):
  - Migrated `ServerRail.tsx` to Uno utility classes for rail container internals, workspace buttons, and footer action controls while retaining only `.server-rail` as the stable shell/layout hook.
  - Removed migrated ServerRail selector blocks from `src/styles/app/base.css` and `src/styles/app/shell-refresh.css` (`.rail-label`, `.server-list`, `.server-rail-footer`, `.server-action`, and legacy `.server-rail button*` cascades).
  - Added `tests/app-shell-server-rail.test.tsx` to lock utility-class rendering, callback behavior, and removal of legacy internal class hooks.
  - Extended `tests/app-style-token-manifest.test.ts` to include `ServerRail.tsx` in migrated raw-color literal guards and to assert removed ServerRail selectors stay absent from legacy stylesheets.
- Important finding:
  - ServerRail styles existed in two different `shell-refresh.css` regions plus `base.css`; migration needed removing all duplicated button/label/internal blocks together to avoid leftover cascade overrides.
- Important finding:
  - Keeping `.server-rail` as a non-visual hook remains necessary for shared shell layout/media-query behavior (`.app-shell` grid and mobile border-right rules), even after internal visual styles moved to Uno utilities.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-server-rail.test.tsx tests/app-shell-layout-components.test.tsx tests/app-style-token-manifest.test.ts` passes (`596` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated test typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (ChannelRail surface):
  - Migrated `ChannelRail.tsx` layout/menu/channel rows/voice dock/account controls to Uno utility classes while retaining only stable non-visual hooks required by existing contracts (`.channel-rail`, `.channel-group-header`, `.voice-tree-avatar`, `.voice-tree-avatar-speaking`).
  - Removed migrated ChannelRail selector blocks from `src/styles/app/base.css` and `src/styles/app/shell-refresh.css` (`workspace-menu*`, `channel-nav*`, voice dock/control classes, account bar classes, legacy channel row/group selectors, and duplicated legacy voice presence selector blocks).
  - Added `tests/app-shell-channel-rail.test.tsx` to lock Uno class rendering, tokenized disconnect-button classes, and preservation of speaking-avatar test hooks.
  - Extended `tests/app-style-token-manifest.test.ts` with ChannelRail legacy-selector removal assertions and a utility-class token guard for the disconnect button (`bg-danger-panel`, `border-danger-panel-strong`, `text-danger-ink`) without inline style usage.
- Important finding:
  - `shell-refresh.css` contained two separate ChannelRail style strata (legacy carry-over and refresh block) with duplicated voice presence/layout selectors; both had to be removed together to avoid hidden cascade dependencies.
- Important finding:
  - Existing voice integration tests depend on `.voice-tree-avatar` / `.voice-tree-avatar-speaking` as query hooks; these classes were intentionally retained as non-visual hooks while all visual styling moved to Uno utilities.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-channel-rail.test.tsx tests/app-style-token-manifest.test.ts tests/app-shell-layout-components.test.tsx tests/app-shell-channel-kinds.test.tsx tests/app-shell-voice-controls.test.tsx` passes (`600` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (MemberRail surface):
  - Migrated `MemberRail.tsx` structure/profile summary/member presence rows/panel launch actions to Uno utility classes while retaining only `.member-rail` as the stable shell/layout hook.
  - Removed migrated MemberRail-owned selectors from `src/styles/app/base.css` and `src/styles/app/shell-refresh.css` (`.profile-card*`, `.ops-launch-grid*`, and `.member-rail h4` contributions in mixed heading selectors).
  - Added `tests/app-shell-member-rail.test.tsx` to lock utility-class rendering, callback wiring, and removal of legacy internal hooks.
  - Extended `tests/app-style-token-manifest.test.ts` to include `MemberRail.tsx` in migrated raw-color literal guards and to assert removed MemberRail selectors remain absent from legacy stylesheets.
- Important finding:
  - `.member-group` and `.group-label` are still shared across multiple panel surfaces; removing those selectors during MemberRail migration would regress unmigrated panels, so this slice only removed MemberRail-specific selector families.
- Important finding:
  - Both `base.css` and `shell-refresh.css` declared a combined `.chat-header h3, .member-rail h4` margin rule; migration required splitting those selectors so MemberRail heading spacing is now owned by component utilities without changing ChatHeader behavior.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-member-rail.test.tsx tests/app-shell-layout-components.test.tsx tests/app-style-token-manifest.test.ts` passes (`603` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (ChatHeader surface):
  - Migrated `ChatHeader.tsx` heading/status badges/action controls to Uno utility classes while retaining only `.chat-header` as the stable shell/layout hook.
  - Removed migrated ChatHeader selectors from `src/styles/app/base.css` and `src/styles/app/shell-refresh.css` (`.chat-header*`, `.header-actions*`, `.gateway-badge*`, `.voice-badge*`, `.logout`, and `.header-icon-button*` blocks).
  - Added `tests/app-shell-chat-header.test.tsx` to lock utility-class rendering, action callback wiring, and refresh-session disabled-state behavior.
  - Extended `tests/app-style-token-manifest.test.ts` to include `ChatHeader.tsx` in migrated raw-color literal guards and assert removed ChatHeader selectors remain absent from legacy stylesheets.
- Important finding:
  - ChatHeader styles were duplicated across both the legacy carry-over region and the refresh block of `shell-refresh.css`, including separate mobile `@media (max-width: 900px)` overrides. All of those blocks had to be removed together to avoid partial cascade leftovers.
- Important finding:
  - Keeping `.chat-header` as a non-visual hook is still useful for shell composition tests and layout slot clarity, while all visual/responsive styling now lives in component utilities.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-chat-header.test.tsx tests/app-style-token-manifest.test.ts tests/app-shell-layout-components.test.tsx` passes (`606` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (UserProfileOverlay surface):
  - Migrated `UserProfileOverlay.tsx` backdrop/dialog/header/profile details/markdown rendering to Uno utility classes and removed reliance on legacy `.panel-window*` / `.profile-view*` visual class hooks.
  - Removed migrated UserProfileOverlay selectors from `src/styles/app/shell-refresh.css` (`.profile-view-*` blocks plus combined markdown selector arms).
  - Added `tests/app-shell-user-profile-overlay.test.tsx` to lock utility-class rendering, close interactions, loading/error visibility, and avatar fallback behavior.
  - Extended `tests/app-style-token-manifest.test.ts` to include `UserProfileOverlay.tsx` in migrated raw-color literal guards and assert removed UserProfileOverlay selectors remain absent from `shell-refresh.css`.
- Important finding:
  - `shell-refresh.css` had shared combined selectors for `.settings-profile-markdown` and `.profile-view-markdown`; migration required splitting those blocks so settings markdown styling remains intact after removing overlay-specific selectors.
- Important finding:
  - Avatar markup in `UserProfileOverlay` is intentionally nested inside an `aria-hidden` container, so tests should query avatar images by `alt` text rather than accessible role.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-user-profile-overlay.test.tsx tests/app-shell-layout-components.test.tsx tests/app-style-token-manifest.test.ts` passes (`609` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (SettingsPanel surface):
  - Migrated `SettingsPanel.tsx` category rail/submenu rail/profile preview to Uno utility classes and removed legacy `settings-*` class dependencies.
  - Removed migrated SettingsPanel selectors from `src/styles/app/shell-refresh.css`, including the `@media (max-width: 900px)` settings-layout overrides now represented in component utility classes.
  - Added `tests/app-shell-settings-panel.test.tsx` to lock utility-class rendering, callback wiring, and profile preview behavior (including avatar image fallback hiding).
  - Extended `tests/app-style-token-manifest.test.ts` to include `SettingsPanel.tsx` in migrated raw-color literal guards and assert removed SettingsPanel selectors remain absent from `shell-refresh.css`.
- Important finding:
  - Settings layout selectors existed in both the primary legacy carry-over block and the mobile media-query block; both blocks had to be removed together to avoid a hidden mobile-only cascade dependency.
- Important finding:
  - `SafeMarkdown` paragraph/list spacing for settings profile preview can be migrated safely via Uno descendant utility selectors (`[&_p]`, `[&_p+p]`, `[&_ul]`, `[&_ol]`) without reintroducing a dedicated legacy markdown class.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-settings-panel.test.tsx tests/app-shell-settings-entry.test.tsx tests/app-style-token-manifest.test.ts` passes (`612` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (AuthShell surface):
  - Migrated `LoginPage.tsx` auth shell layout/header/mode switch/form controls/captcha-state messaging to Uno utility classes while retaining `.auth-layout`, `.auth-panel`, `.auth-mode-switch`, `.auth-form`, and `.captcha-block` as stable non-visual hooks.
  - Removed migrated auth shell selectors from `src/styles/app/base.css` (`.auth-layout`, `.auth-panel`, `.auth-header h1`, `.eyebrow`, `.auth-mode-switch*`, `.auth-form*`, `.captcha-block*`).
  - Extended `tests/app-style-token-manifest.test.ts` to include `LoginPage.tsx` in migrated raw-color literal guards and to assert removed auth shell selectors stay absent from `base.css`.
- Important finding:
  - Existing route/auth integration tests rely on `.auth-form` as a stable submit-button query boundary, so those class hooks should remain available as non-visual contracts while visual styling is owned by Uno utilities.
- Important finding:
  - Captcha error feedback inside `.captcha-block` previously depended on legacy `.captcha-block .status` margin resets; migrating to utility-first status typography avoids margin-cascade coupling in this surface.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/routes-login.test.tsx tests/app-style-token-manifest.test.ts` passes (`613` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).

## Phase 5 - Legacy CSS Removal and Governance
Status: `IN PROGRESS`
Completion status (2026-02-18): `17/17 task tracks started` (shared overlay panel shell and public/friendship directory selector families migrated; governance doc added; dead selector cleanup expanded with global label-helper removal; stacked-meta/mono bridge cleanup added; stale voice roster/video-grid/reaction-trigger selectors removed; dead ops-overlay selector family removed; empty-workspace bridge selector removed; presence-indicator bridge selector removed; role-management helper selector family migrated; member-group list bridge selector cleanup added; workspace/channel-create form surfaces utility-migrated; utility/workspace-settings helper migration added; moderation panel helper migration added; search/attachments form helper migration added; role-management shared helper extraction with member-group/button-row selector removal added; legacy CSS reduction still in progress)

Tasks:
- Remove dead selectors and bridge styles.
- Keep only minimal reset/base/token CSS.
- Add style governance doc:
  - when to use inline utility classes vs shortcut
  - variant/state conventions
  - token-only color policy

Exit Criteria:
- `shell-refresh.css` removed or reduced to minimal compatibility patch.
- Migration complete with stable tests.

Implementation Notes (2026-02-18):
- Applied slice (Style governance documentation):
  - Added `src/styles/STYLE_GOVERNANCE.md` to codify UnoCSS governance for inline-utility vs `fx-*` shortcut usage, variant/state conventions, token-only color policy, and legacy bridge removal rules.
  - Extended `tests/app-style-token-manifest.test.ts` with a governance-doc contract check so required policy sections remain present during future migration slices.
- Important finding:
  - Governance requirements were only captured in this phase plan before this slice; adding a checked-in style policy doc plus a test guard reduces drift risk as more contributors touch migrated surfaces.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-style-token-manifest.test.ts` passes (`621` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (PanelHost shared shell):
  - Migrated `PanelHost.tsx` backdrop/window/header/body presentation to Uno utility classes while retaining `.panel-backdrop`, `.panel-window`, `.panel-window-medium`, `.panel-window-compact`, `.panel-window-header`, and `.panel-window-body` as stable non-visual hooks for compatibility.
  - Added deterministic width utility mapping in `PanelHost.tsx` so overlay size variants remain tied to existing `overlayPanelClassName` outputs without relying on legacy CSS.
  - Removed migrated shared panel selectors from `src/styles/app/shell-refresh.css` (`.panel-backdrop`, `.panel-window*`, `.panel-window-header*`, `.panel-window-body`, and their mobile width/padding override block).
  - Extended `tests/app-shell-panel-host-props.test.tsx` with a layout contract check for PanelHost utility classes and hook preservation.
  - Extended `tests/app-style-token-manifest.test.ts` to assert removed panel host selectors remain absent from `shell-refresh.css`, and added `PanelHost.tsx` to migrated raw-color literal guard coverage.
- Important finding:
  - `overlayPanelClassName` is consumed as both behavior contract and sizing signal; preserving these class hooks while moving visuals into Uno avoids runtime/controller churn and keeps panel-size routing deterministic.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-panel-host-props.test.tsx tests/app-style-token-manifest.test.ts` passes (`615` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (PublicDirectoryPanel + FriendshipsPanel surfaces):
  - Migrated `PublicDirectoryPanel.tsx` and `FriendshipsPanel.tsx` list/form/action/status presentation to Uno utility classes while retaining `.public-directory` as a stable non-visual panel hook.
  - Removed dead legacy public-directory selectors from `src/styles/app/base.css` (`.public-directory*`, `.public-directory-row-*`, `.directory-status-chip*`) plus the unused `.unread-count` selector.
  - Added `tests/app-shell-public-directory-panel.test.tsx` and `tests/app-shell-friendships-panel.test.tsx` to lock utility-class rendering, callback wiring, and removal of legacy internal class hooks in both panel surfaces.
  - Extended `tests/app-style-token-manifest.test.ts` to include both panel TSX files in migrated raw-color literal guard coverage and to assert the removed public-directory selector family stays absent from `base.css`.
- Important finding:
  - `.public-directory` list/container selectors were shared by both Public Directory and Friendships overlays; selector removal is only safe after migrating both surfaces in the same slice.
- Important finding:
  - `.unread-count` had no remaining TSX usage and was safe to delete as dead CSS.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-public-directory-panel.test.tsx tests/app-shell-friendships-panel.test.tsx tests/app-style-token-manifest.test.ts tests/app-shell-public-discovery.test.tsx tests/app-shell-friendships.test.tsx` passes (`620` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (Dead legacy selector cleanup: panel-note + stale single-surface rules):
  - Migrated remaining `panel-note` usage in `ChatColumn.tsx`, `PanelHost.tsx`, and `UtilityPanel.tsx` to equivalent Uno margin utilities (`m-[0.5rem_1rem_0]`), removing dependency on a global helper class for transient-note spacing.
  - Removed dead selectors from `src/styles/app/base.css`: `.panel-note`, `.load-older`, `.workspace-create-panel`, and `.workspace-create-panel h4`.
  - Extended `tests/app-style-token-manifest.test.ts` with a regression assertion that these removed selector blocks remain absent, and added `UtilityPanel.tsx` + `ChatColumn.tsx` to migrated raw-color guard coverage.
- Important finding:
  - `.load-older` and `.workspace-create-panel*` had no remaining TSX usage; they were safe deletions and represented stale bridge CSS left behind from earlier migration slices.
- Important finding:
  - `panel-note` acted as a cross-surface spacing helper for chat transient notices and panel diagnostics/loading fallbacks; utility migration removes this hidden cascade coupling while preserving spacing parity.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-style-token-manifest.test.ts tests/app-shell-layout-components.test.tsx tests/app-shell-utility-panel.test.tsx tests/app-shell-panel-host-props.test.tsx` passes (`622` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (Dead legacy selector cleanup: `group-label` bridge removal):
  - Replaced all remaining `group-label` usage in migrated panel surfaces (`SettingsPanel.tsx`, `FriendshipsPanel.tsx`, `WorkspaceSettingsPanel.tsx`) with equivalent Uno utility class strings (`m-0`, uppercase tracking, tokenized ink color).
  - Removed dead `group-label` selector blocks from `src/styles/app/base.css` and `src/styles/app/shell-refresh.css`, plus the now-unreachable `.ops-overlay-header .group-label` rule in `base.css`.
  - Added `tests/app-shell-workspace-settings-panel.test.tsx` and extended `tests/app-shell-settings-panel.test.tsx` + `tests/app-shell-friendships-panel.test.tsx` with regression assertions that `group-label` legacy hooks are absent.
  - Extended `tests/app-style-token-manifest.test.ts` with `group-label` selector-removal assertions and added `WorkspaceSettingsPanel.tsx` to migrated raw-color guard coverage.
- Important finding:
  - `group-label` persisted as a cross-surface typography helper even after the related surfaces were utility-migrated; converting labels in those surfaces first allowed safe CSS selector deletion without touching shared runtime hooks.
- Important finding:
  - `base.css` still contained a scoped `.ops-overlay-header .group-label` rule despite no live `ops-overlay` TSX usage; removing it with the global helper avoids a dead selector re-entry path.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-settings-panel.test.tsx tests/app-shell-friendships-panel.test.tsx tests/app-shell-workspace-settings-panel.test.tsx tests/app-style-token-manifest.test.ts` passes.
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (Dead legacy selector cleanup: `stacked-meta` + `mono` bridge removal):
  - Replaced remaining `stacked-meta`/`mono` helper usage in `PublicDirectoryPanel.tsx`, `FriendshipsPanel.tsx`, `AttachmentsPanel.tsx`, and `SettingsPanel.tsx` with equivalent Uno utility classes (`grid min-w-0 gap-[0.16rem]`, `text-[0.78rem]`, `font-code`).
  - Removed dead `.stacked-meta` and `.mono` selectors from `src/styles/app/base.css`.
  - Added `tests/app-shell-attachments-panel.test.tsx` for AttachmentsPanel rendering and handler wiring, and extended panel tests (`app-shell-public-directory-panel.test.tsx`, `app-shell-friendships-panel.test.tsx`, `app-shell-settings-panel.test.tsx`) to assert utility-class usage and absence of legacy hooks.
  - Extended `tests/app-style-token-manifest.test.ts` selector-removal assertions for `.stacked-meta`/`.mono` and added `AttachmentsPanel.tsx` to migrated raw-color guard coverage.
- Important finding:
  - `stacked-meta`/`mono` persisted as cross-panel bridge helpers across directory, friendships, attachments, and settings-profile preview surfaces; selector removal was only safe after migrating all remaining usages in one slice.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-friendships-panel.test.tsx tests/app-shell-attachments-panel.test.tsx tests/app-shell-public-directory-panel.test.tsx tests/app-shell-settings-panel.test.tsx tests/app-style-token-manifest.test.ts` passes (`627` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (Dead legacy selector cleanup: voice roster + video-grid + reaction trigger removal):
  - Removed dead voice roster and voice video grid selector families from `src/styles/app/base.css` (`.voice-roster*`, `.voice-stream-hints*`, `.voice-video-grid*`, `.voice-video-tile*`) along with stale `.reaction-add-trigger` styles.
  - Extended `tests/app-style-token-manifest.test.ts` with regression assertions that these selector families remain absent from `base.css`.
- Important finding:
  - The removed voice roster/video-grid selectors had zero live TSX/test references and existed only in `base.css`, making them safe bridge-CSS deletions in this phase.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-style-token-manifest.test.ts` passes.
  - `pnpm -C apps/filament-client-web run lint` passes.
- Applied slice (Dead legacy selector cleanup: `ops-overlay` family removal):
  - Removed dead `.ops-overlay`, `.ops-overlay-header`, and `.ops-overlay-header button` selectors from `src/styles/app/base.css`.
  - Extended `tests/app-style-token-manifest.test.ts` with regression assertions that the removed `ops-overlay` selector family remains absent from `base.css`.
- Important finding:
  - The `ops-overlay` selector family had zero live TSX/test references and persisted only as stale bridge CSS after earlier `group-label` cleanup, so deletion is safe and reduces dead cascade surface.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-style-token-manifest.test.ts` passes (`629` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (Dead legacy selector cleanup: `empty-workspace` bridge removal):
  - Replaced the `empty-workspace` fallback hook in `ChatColumn.tsx` with equivalent Uno utility classes (`grid gap-[0.72rem] p-[1rem]`) while preserving the same fallback copy and layout structure.
  - Removed dead `.empty-workspace` selector from `src/styles/app/base.css`.
  - Extended `tests/app-shell-layout-components.test.tsx` with a fallback-layout regression assertion that utility classes render and the legacy `empty-workspace` class hook stays absent.
  - Extended `tests/app-style-token-manifest.test.ts` selector-removal assertions to lock `.empty-workspace` absence from `base.css`.
- Important finding:
  - `empty-workspace` styling was isolated to a single fallback section in `ChatColumn.tsx`; converting that section directly to utilities made selector removal low-risk and avoided introducing another shared bridge helper.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-layout-components.test.tsx tests/app-style-token-manifest.test.ts` passes.
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (Dead legacy selector cleanup: `presence` bridge removal):
  - Replaced remaining `presence` helper usage in `MemberRail.tsx`, `PublicDirectoryPanel.tsx`, `AttachmentsPanel.tsx`, and `SearchPanel.tsx` with equivalent Uno utility classes backed by shared tokens (`bg-presence-online`, `bg-presence-idle`).
  - Added `--presence-online` and `--presence-idle` to `src/styles/app/tokens.css` and mapped them in `uno.config.ts` so migrated surfaces avoid raw color literals while preserving prior status-dot colors.
  - Removed dead `.presence`, `.presence.online`, and `.presence.idle` selectors from `src/styles/app/base.css`.
  - Extended `tests/app-style-token-manifest.test.ts` with token/selector-removal guards and added component-level regression assertions in `app-shell-member-rail.test.tsx`, `app-shell-public-directory-panel.test.tsx`, and `app-shell-attachments-panel.test.tsx`.
  - Added `tests/app-shell-search-panel.test.tsx` to lock SearchPanel rendering/handler behavior and utility-class status-dot usage.
- Important finding:
  - `presence` persisted as a cross-surface bridge helper spanning both migrated and partially migrated panels; tokenizing its Uno replacement first allowed safe selector deletion without reintroducing raw color literals.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-member-rail.test.tsx tests/app-shell-public-directory-panel.test.tsx tests/app-shell-attachments-panel.test.tsx tests/app-shell-search-panel.test.tsx tests/app-style-token-manifest.test.ts` passes.
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (Dead legacy selector cleanup: role-management helper family removal):
  - Migrated `RoleManagementPanel.tsx` role hierarchy and permission-matrix presentation from legacy helper hooks (`role-hierarchy-*`, `permission-*`, `role-preview`, `checkbox-row`, `status-chip`, `role-reorder-row`) to Uno utility classes while preserving existing behavior and accessibility contracts.
  - Removed dead role-management helper selector blocks from `src/styles/app/base.css` (`.role-hierarchy-grid`, `.role-hierarchy-item*`, `.permission-grid`, `.permission-toggle*`, `.role-preview`, `.role-reorder-row*`, `.status-chip`, `.checkbox-row*`).
  - Extended `tests/app-shell-role-management-panel.test.tsx` with regression assertions that utility classes render and legacy helper hooks remain absent.
  - Extended `tests/app-style-token-manifest.test.ts` with selector-removal assertions for the role-management helper family and added `RoleManagementPanel.tsx` to migrated raw-color guard coverage.
- Important finding:
  - `RoleManagementPanel` still depends on shared `.inline-form` and `.button-row` helpers that are also used by unmigrated panels; this slice intentionally removed only role-management-specific helper families to avoid cross-panel regressions.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-role-management-panel.test.tsx tests/app-style-token-manifest.test.ts` passes (`636` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (Dead legacy selector cleanup: `member-group` list bridge removal):
  - Migrated `SearchPanel.tsx` and `AttachmentsPanel.tsx` search-result/attachment list rows to explicit Uno utility classes (`list-none`, tokenized row surfaces, utility action buttons), removing reliance on inherited `.member-group ul/li` bridge styling.
  - Removed dead `.member-group ul` and `.member-group li` selectors from `src/styles/app/base.css` while retaining `.member-group` section layout as an interim helper for still-unmigrated panel wrappers.
  - Extended `tests/app-shell-search-panel.test.tsx` and `tests/app-shell-attachments-panel.test.tsx` with list-row utility assertions, and extended `tests/app-style-token-manifest.test.ts` with selector-removal guards plus `SearchPanel.tsx` raw-color guard coverage.
- Important finding:
  - `.member-group ul/li` selectors were only still affecting `SearchPanel` and `AttachmentsPanel`; migrating those two list surfaces in the same slice enabled safe selector deletion without touching broader `.inline-form`/`.button-row` helper usage that remains shared by unmigrated panels.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-search-panel.test.tsx tests/app-shell-attachments-panel.test.tsx tests/app-style-token-manifest.test.ts` passes (`637` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (WorkspaceCreatePanel + ChannelCreatePanel utility migration and lingering `member-group` shell bridge cleanup):
  - Migrated `WorkspaceCreatePanel.tsx` and `ChannelCreatePanel.tsx` form/field/button/status presentation from shared legacy helper hooks (`member-group`, `inline-form`, `button-row`, `status`) to explicit Uno utility classes while preserving submit/cancel and input-binding behavior.
  - Removed lingering `.member-group li` bridge styling from `src/styles/app/shell-refresh.css` and extended manifest tests to guard that this selector remains absent from both `base.css` and `shell-refresh.css`.
  - Added `tests/app-shell-workspace-create-panel.test.tsx` and `tests/app-shell-channel-create-panel.test.tsx` to lock utility-class rendering, callback wiring, and removal of legacy helper hooks.
  - Extended `tests/app-style-token-manifest.test.ts` migrated-surface raw-color guards to include both create-panel surfaces.
- Important finding:
  - Even after base-layer `member-group` list selector cleanup, `shell-refresh.css` still carried a high-specificity `.member-group li` rule that could silently override utility row styles in nested panel surfaces.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-channel-create-panel.test.tsx tests/app-shell-workspace-create-panel.test.tsx tests/app-style-token-manifest.test.ts` passes (`642` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (UtilityPanel + WorkspaceSettingsPanel helper migration):
  - Migrated `UtilityPanel.tsx` and `WorkspaceSettingsPanel.tsx` form/field/button/status presentation from shared legacy helper hooks (`member-group`, `inline-form`, `button-row`, `status`) to explicit Uno utility classes while preserving existing callback and permission-gating behavior.
  - Extended `tests/app-shell-utility-panel.test.tsx` and `tests/app-shell-workspace-settings-panel.test.tsx` with utility-class contract assertions and regression checks that legacy helper hooks are absent from these surfaces.
- Important finding:
  - Shared helper selectors (`.inline-form`, `.button-row`, `.status`, `.member-group`) are still used by other unmigrated panels (notably Moderation and parts of RoleManagement), so this slice intentionally avoids selector deletion and only removes those hooks from the migrated panel surfaces.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-utility-panel.test.tsx tests/app-shell-workspace-settings-panel.test.tsx` passes (`643` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (ModerationPanel helper migration):
  - Migrated `ModerationPanel.tsx` form/field/button/status presentation from shared legacy helper hooks (`member-group`, `inline-form`, `button-row`, `status`) to explicit Uno utility classes while preserving moderation/override action gating and callback wiring.
  - Added `tests/app-shell-moderation-panel.test.tsx` to lock utility-class rendering, interaction behavior, and absence of legacy helper hooks.
  - Extended `tests/app-style-token-manifest.test.ts` migrated-surface raw-color guards to include `ModerationPanel.tsx`.
- Important finding:
  - Moderation action rows need `flex-1` button utilities to preserve prior equal-width button behavior that legacy `.button-row button { flex: 1; }` provided.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-moderation-panel.test.tsx tests/app-shell-moderation-panel-props.test.ts tests/app-shell-moderation-controller.test.ts tests/app-style-token-manifest.test.ts` passes (`646` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (SearchPanel + AttachmentsPanel helper migration):
  - Migrated `SearchPanel.tsx` and `AttachmentsPanel.tsx` form/field/button/status presentation from shared legacy helper hooks (`member-group`, `inline-form`, `button-row`, `status`, `muted`) to explicit Uno utility classes while preserving submit handlers, maintenance actions, and attachment operations.
  - Extended `tests/app-shell-search-panel.test.tsx` and `tests/app-shell-attachments-panel.test.tsx` with regression assertions for utility-class rendering and absence of legacy helper hooks in both surfaces.
- Important finding:
  - `SearchPanel` still relied on legacy `.button-row button { flex: 1; }` for equal-width maintenance controls; utility migration needed explicit `flex-1` on maintenance action buttons to preserve layout parity.
- Important finding:
  - `AttachmentsPanel` file and filename controls inherited shared `.inline-form` input styling; utility migration required applying the same border/background/disabled semantics directly to both inputs to avoid regressions when helper selectors are eventually removed.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-search-panel.test.tsx tests/app-shell-attachments-panel.test.tsx` passes.
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).
- Applied slice (RoleManagementPanel shared-helper extraction + dead selector cleanup):
  - Migrated `RoleManagementPanel.tsx` away from shared legacy helper hooks (`member-group`, `button-row`, `inline-form`, `muted`) to explicit Uno utility classes for panel layout, form fields, and action rows while preserving role create/edit/reorder/assignment behavior.
  - Removed dead `.member-group` and `.button-row*` selectors from `src/styles/app/base.css`; retained `.inline-form*` because `SettingsPanel.tsx` still depends on that helper family.
  - Extended `tests/app-shell-role-management-panel.test.tsx` with regression assertions for utility action-row classes and absence of `member-group`/`button-row`/`inline-form` hooks in the RoleManagement surface.
  - Extended `tests/app-style-token-manifest.test.ts` with selector-removal assertions for `.member-group {` and `.button-row*`.
- Important finding:
  - `.button-row` and `.member-group` are now unused after this slice, but `.inline-form` remains actively used by `SettingsPanel.tsx`; full removal of the inline-form helper family should be done with a SettingsPanel migration slice.
- Validation for this slice:
  - `pnpm -C apps/filament-client-web run test -- tests/app-shell-role-management-panel.test.tsx tests/app-style-token-manifest.test.ts` passes (`646` tests total in run).
  - `pnpm -C apps/filament-client-web run lint` passes.
  - `pnpm -C apps/filament-client-web run build` passes.
  - `pnpm -C apps/filament-client-web run typecheck` still fails on pre-existing unrelated typing issues (`tests/app-shell-identity-resolution-controller.test.ts`, `tests/app-shell-selectors.test.ts`).

## Testing and Validation Gates
Run on every migration phase:
- `pnpm -C apps/filament-client-web run typecheck`
- `pnpm -C apps/filament-client-web run test`
- `pnpm -C apps/filament-client-web run build`

Add/maintain targeted tests:
- message list pinning + history behavior
- composer placement and chat-panel sizing
- reaction/editing state visuals and interaction behavior

Recommended:
- add screenshot/visual regression coverage for chat shell and rails to catch subtle layout drift.

## Risks and Mitigations
- Risk: utility sprawl and inconsistent patterns.
  - Mitigation: enforce shortcuts + naming conventions early.
- Risk: chat scroll regressions during layout changes.
  - Mitigation: Phase 2 behavior fix with focused tests before broad visual rewrite.
- Risk: partial migration leaves hard-to-reason cascade interactions.
  - Mitigation: surface-by-surface CSS deletion immediately after each migrated slice.

## Effort Estimate
- Phase 0-1: 1-3 days
- Phase 2 (chat behavior fix): 2-4 days
- Phase 3 (chat Uno migration): 3-6 days
- Phase 4-5 (full shell migration + cleanup): 2-4 weeks

## Execution Order
1. Phase 0
2. Phase 1
3. Phase 2 (layout behavior correctness first)
4. Phase 3
5. Phase 4
6. Phase 5

## Decision
Proceed with UnoCSS migration, with chat layout behavior fixes treated as a blocking prerequisite before broader UI rewrite.
