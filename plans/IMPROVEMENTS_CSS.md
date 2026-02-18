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
Status: `NOT STARTED`

Scope:
- server rail, channel rail, member rail, header, overlays, settings panels, auth shell

Tasks:
- Migrate each surface in small PR slices.
- Use shortcuts for repeated panel/button/list patterns.
- Preserve responsive behavior and collapse modes.

Exit Criteria:
- Primary shell UI uses UnoCSS utilities/shortcuts.
- Legacy global selectors mostly removed.

## Phase 5 - Legacy CSS Removal and Governance
Status: `NOT STARTED`

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
