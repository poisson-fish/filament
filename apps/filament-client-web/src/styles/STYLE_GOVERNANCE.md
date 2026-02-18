# Web Style Governance (UnoCSS)

This document defines how styling changes are made in `apps/filament-client-web` during and after the UnoCSS migration.

## Scope

- Applies to all TSX styling in `src/`.
- Applies to legacy CSS cleanup in `src/styles/app/*.css`.
- Security posture from `AGENTS.md` is unchanged: no unsafe runtime style/script injection and no HTML rendering paths in chat.

## When To Use Inline Utilities vs Shortcuts

- Use inline utilities for local, one-off layout and spacing decisions that are only used by one component.
- Use `fx-*` shortcuts for repeated primitives used across surfaces, especially buttons, panels, chips, rails, and list rows.
- Promote to a shortcut when the same utility cluster appears in 3 or more places, or when the cluster represents a stable semantic primitive.
- Keep shortcuts low-level and composable. Do not create feature-specific shortcuts tied to a single screen workflow.
- Keep non-visual runtime hooks (for tests/controllers) separate from visual classes when needed.

## Variant And State Conventions

- Build classes in this order: base layout, typography, interaction, state, responsive overrides.
- Prefer utility variants for state (`hover:`, `focus-visible:`, `disabled:`, `aria-*`, `data-*`) instead of legacy global selectors.
- Represent destructive and warning states via token aliases, not special-case literal colors.
- Keep disabled and busy states explicit in markup and classes so behavior is testable.

## Token-Only Color Policy

- Use token aliases in TSX classes (`bg-bg-*`, `text-ink-*`, `border-line*`, `bg-danger-panel`, `text-danger-ink`).
- Do not use raw `#hex`, `rgb(a)`, or `hsl(a)` literals in migrated TSX surfaces.
- Do not use inline `style={{ color: ... }}` or `style={{ background: ... }}` for visual colors.
- If a needed color does not exist, add a CSS variable in `src/styles/app/tokens.css` and map it in `uno.config.ts` before use.

## Legacy CSS Bridge Rules

- Remove migrated selector families from `base.css` and `shell-refresh.css` in the same slice that migrates the owning surface.
- Keep only minimal reset, token, and temporary compatibility selectors required by unmigrated surfaces.
- Preserve deterministic tests for removed selectors and migrated class contracts in `tests/app-style-token-manifest.test.ts`.
