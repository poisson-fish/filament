# PLAN_MARKDOWN.md

## Objective
Ship secure markdown rendering in chat message text and user profile surfaces, while expanding profile settings with banner image upload and a bounded markdown about section.

## Scope
- Render markdown token streams in:
  - chat message list rows
  - user profile overlay
  - profile settings preview
- Add user profile banner upload/download + display.
- Keep and enforce a strict about-markdown length cap with clear UX feedback.
- Preserve existing hostile-server assumptions and fail-closed parsing.

## Locked Product Decisions (2026-02-25)
- Hostile-server model remains strict: no server-side HTML rendering path for markdown, and no client `innerHTML` rendering path.
- Banner uploads allow animated GIF in v1 (alongside existing image formats).
- Profile about markdown cap remains `2048` characters.
- Markdown scope should expand as far as safely possible this sprint, including fenced code blocks with language labels from triple-backtick syntax.
- Code-block highlighting must use a vetted library in a safe token/AST path (no raw HTML injection path).

## Tentative Dependency Plan
- Rust server/core markdown:
  - No new parser dependency required; continue using existing `pulldown-cmark` in `crates/filament-core`.
  - Extend tokenization in `filament-core` to emit fenced-code tokens from existing parser events.
- Web client syntax highlighting:
  - Add `lowlight` (MIT) for AST/token-based syntax highlighting.
  - Add `highlight.js` language modules via `lowlight` registration with a strict allowlist only (avoid full language packs by default).
  - Keep rendering as Solid JSX nodes from tokens/AST only; do not consume any highlighter HTML output.
- No dependency changes for crypto/auth/upload transport.

## Dependency Review Checklist (Plan Gate)
- Confirm licenses are permissive in `cargo-deny` / dependency review.
- Pin to actively maintained major versions.
- Keep language grammars bounded (tree-shaking/explicit imports).
- Add tests that prove highlight path cannot inject HTML/script URLs.

## Non-Goals
- No raw HTML rendering path for markdown.
- No custom crypto or token format changes.
- No federation/E2EE work.
- No relaxation of existing request limits, WS limits, or rate limiting.

## Code-State Audit (2026-02-25)
- Server already tokenizes markdown via `filament_core::tokenize_markdown` (`pulldown-cmark`) and emits `markdown_tokens` for messages and profiles.
- Profile markdown already renders through `SafeMarkdown` in:
  - `UserProfileOverlay.tsx`
  - `SettingsPanel.tsx`
- Message list currently does **not** render markdown structure; `MessageRow.tsx` flattens tokens with `tokenizeToDisplayText(...)` and renders plain text.
- Profile supports avatar upload/versioning, but there is no profile banner upload/storage/versioning endpoint or client model.
- Profile about limit is already enforced at `2048` chars in domain/core, but UX around the limit is minimal.

## Locked Security Constraints
- Markdown rendering stays token-driven (`MarkdownToken[]`) with no `innerHTML` or HTML passthrough.
- Server does not render markdown to HTML for transport; API returns markdown tokens only.
- Link allowlist remains strict (`http`, `https`, `mailto`) with explicit sanitization on server and client.
- Uploads remain raw binary with:
  - byte caps
  - MIME sniffing (`infer`)
  - declared `Content-Type` consistency checks when present
- All new DTOs keep strict validation (`deny_unknown_fields` where applicable, bounded fields in parser layer).
- Gateway compatibility stays additive and version-compatible (`{ v, t, d }`).

## Status Legend
- `NOT STARTED`
- `IN PROGRESS`
- `DONE`
- `BLOCKED`

---

## Phase 0 - Contract Lock and UX Decisions
### Goal
Lock markdown/profile-banner behavior before implementation.

### Completion Status
`IN PROGRESS`

### Tasks
- [x] Finalize markdown token contract expansion for fenced code blocks:
  - add fenced code token variants with explicit language field
  - cap language label length and character set
  - cap code-block token payload size/count to prevent abuse
- [x] Lock profile about cap at `2048` chars across server/client validation + UX counter.
- [ ] Lock banner media policy:
  - cap: `6 MiB`
  - MIME allowlist: `image/jpeg`, `image/png`, `image/webp`, `image/avif`, `image/gif`
- [ ] Lock profile API/gateway additions:
  - `banner_version` on profile responses
  - `POST /users/me/profile/banner`
  - `GET /users/{user_id}/banner`
  - new gateway event `profile_banner_update` (or equivalent additive field update contract)
- [ ] Choose secure code highlight strategy for fenced blocks:
  - vetted dependency with permissive license
  - no HTML string rendering
  - bounded language allowlist (unknown languages degrade to plain text)

### Tentative File Touch List
- `crates/filament-core/src/lib.rs`
  - extend `MarkdownToken` enum for fenced code blocks and language metadata
  - update tokenizer + invariant tests
- `apps/filament-client-web/src/domain/chat.ts`
  - extend markdown token DTO parsing/newtypes for fenced code tokens
- `apps/filament-server/src/server/types.rs`
  - ensure profile/message response token contract carries new variants
- `docs/API.md`
  - document fenced code token schema and language constraints
- `docs/GATEWAY_EVENTS.md`
  - verify any profile-related payload contract updates remain additive

### Progress Notes
- 2026-02-26: Added `fenced_code { language, code }` markdown token contract in core + web domain parsing.
- 2026-02-26: Enforced fenced-code bounds:
  - language label max length `32`, charset `[A-Za-z0-9_.+-]`, normalized lowercase
  - max fenced code tokens per payload `64`
  - max fenced code payload per token `16384` chars
- 2026-02-26: Updated `docs/API.md` profile contract to `about_markdown` with `2048` max and documented fenced-code token limits.
- 2026-02-26: Locked profile about cap at `2048` across web domain parsing + save-path validation + settings UX counter.
- 2026-02-26: Added web tests for profile about cap enforcement, zero-remaining counter behavior, and local rejection before API call.
- 2026-02-26: Remaining Phase 0 items still open: banner media/API contract lock, and highlight dependency decision.

### Exit Criteria
- Endpoint/payload/limit decisions are documented and accepted before coding.

---

## Phase 1 - Message Markdown Rendering in Message List
### Goal
Render safe markdown tokens in message rows instead of flattening to plain text.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Update `MessageRow` display path to render `message.markdownTokens` with `SafeMarkdown`.
- [ ] Keep edit-mode input behavior unchanged (plain text textarea/input).
- [ ] Add message-markdown styling hooks for readable paragraphs/lists/links/inline code/fenced code blocks.
- [ ] Ensure message link rendering remains sandboxed (`target="_blank"`, `rel="noopener noreferrer"`, sanitized href).
- [ ] Remove or reduce `tokenizeToDisplayText` usage where no longer needed for message rows.
- [ ] Add language-labelled fenced code rendering UI (` ```lang ` style) with safe syntax highlighting.

### Tests
- [ ] Component tests for message markdown rendering:
  - emphasis/strong/list/inline code/fenced code blocks/line breaks/links
- [ ] Security tests for message markdown:
  - malicious raw HTML content stays inert
  - `javascript:`/`data:` links never render clickable anchors
  - hostile/invalid code-block language labels are rejected or safely downgraded
- [ ] Regression test for existing message-row interactions (reactions/edit/delete/profile click).

### Tentative Dependency Changes
- `apps/filament-client-web/package.json`
  - add `lowlight`
  - add `highlight.js` (explicit language module imports only)

### Tentative File Touch List
- `apps/filament-client-web/src/features/app-shell/components/SafeMarkdown.tsx`
  - add fenced code block rendering path and highlighter token->JSX mapper
- `apps/filament-client-web/src/features/app-shell/components/messages/MessageRow.tsx`
  - replace plaintext markdown projection with `SafeMarkdown` for message body
- `apps/filament-client-web/src/features/app-shell/helpers.ts`
  - keep `tokenizeToDisplayText` only where still needed (or narrow scope)
- `apps/filament-client-web/src/styles/app/*.css` (or Uno utility classes in components)
  - code block container, horizontal overflow, language label styles
- `apps/filament-client-web/tests/app-shell-message-row.test.tsx`
- `apps/filament-client-web/tests/domain-chat.test.ts`
- New focused markdown/security tests:
  - `apps/filament-client-web/tests/safe-markdown.test.tsx` (tentative)

### Exit Criteria
- Message rows render markdown structure securely with no XSS-capable path.

---

## Phase 2 - Profile Banner Backend and Contracts
### Goal
Add server-side profile banner storage, retrieval, validation, and event propagation.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add schema migration for banner metadata on `users` table:
  - `banner_object_key`, `banner_mime_type`, `banner_size_bytes`, `banner_sha256_hex`, `banner_version`
- [ ] Extend server/core models and typed responses (`MeResponse`, `UserProfileResponse`).
- [ ] Add endpoints:
  - `POST /users/me/profile/banner` (binary upload)
  - `GET /users/{user_id}/banner` (binary download)
- [ ] Reuse hardened upload pipeline semantics from avatar path (size cap, sniffing, hash, object-store write/abort).
- [ ] Emit gateway event for banner updates to keep clients cache-coherent.
- [ ] Add router manifest entries + compatibility-safe event wiring.

### Tests
- [ ] Integration tests:
  - banner upload/download round-trip
  - oversized upload rejection
  - MIME mismatch rejection
  - unauthenticated/unauthorized behavior
- [ ] Unit tests for new migration SQL constants/backfills.
- [ ] Gateway event payload tests for banner update event.

### Tentative File Touch List
- DB/migrations:
  - `apps/filament-server/src/server/db/migrations/v5_identity_schema.rs` (or new migration file if preferred by current migration strategy)
- Server core/domain:
  - `apps/filament-server/src/server/core.rs` (new banner metadata constants/model fields)
  - `apps/filament-server/src/server/auth_repository.rs` (profile reads include banner version)
  - `apps/filament-server/src/server/handlers/profile.rs` (banner upload/download handlers)
  - `apps/filament-server/src/server/router.rs` (new route wiring/manifest)
  - `apps/filament-server/src/server/types.rs` (response DTOs include `banner_version`)
  - `apps/filament-server/src/server/gateway_events/profile.rs` (banner update event)
- Tests:
  - `apps/filament-server/src/server/tests/tests/profile.rs`
  - gateway event tests in `apps/filament-server/src/server/gateway_events/profile.rs` test module

### Exit Criteria
- Banner upload/download/versioning works with strict validation and event propagation.

---

## Phase 3 - Profile Model + Settings/Overlay UX
### Goal
Expose banner and improved about-markdown UX in web client profile surfaces.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Extend client domain/API contracts with `bannerVersion`.
- [ ] Add client API methods:
  - `uploadMyProfileBanner(...)`
  - `profileBannerUrl(userId, bannerVersion)`
- [ ] Extend app-shell profile state/controller with banner file selection + upload busy/error/status states.
- [ ] Update `SettingsPanel` profile section:
  - banner file picker + upload action
  - remaining-character indicator for about markdown cap
- [ ] Update `UserProfileOverlay` and profile preview to render banner with safe image fallback behavior.

### Tests
- [ ] API boundary tests for banner DTO parsing.
- [ ] Profile controller tests for banner upload transitions and error mapping.
- [ ] Settings/overlay component tests for:
  - banner rendering/fallback
  - banner upload controls
  - about limit UX behavior

### Tentative File Touch List
- Domain/API:
  - `apps/filament-client-web/src/domain/chat.ts` (`ProfileRecord.bannerVersion`)
  - `apps/filament-client-web/src/lib/api-auth.ts` (banner upload + URL builder)
  - `apps/filament-client-web/src/lib/api.ts` and auth client facade exports
- Controllers/state/runtime wiring:
  - `apps/filament-client-web/src/features/app-shell/state/profile-state.ts`
  - `apps/filament-client-web/src/features/app-shell/controllers/profile-controller.ts`
  - `apps/filament-client-web/src/features/app-shell/runtime/client-settings-panel-props.ts`
- UI:
  - `apps/filament-client-web/src/features/app-shell/components/panels/SettingsPanel.tsx`
  - `apps/filament-client-web/src/features/app-shell/components/overlays/UserProfileOverlay.tsx`
- Tests:
  - `apps/filament-client-web/tests/api-auth.test.ts`
  - `apps/filament-client-web/tests/app-shell-profile-controller.test.ts`
  - `apps/filament-client-web/tests/app-shell-user-profile-overlay.test.tsx`
  - `apps/filament-client-web/tests/app-shell-settings-panel.test.tsx`

### Exit Criteria
- Users can upload banners and see consistent profile markdown/banner rendering in settings and profile overlay.

---

## Phase 4 - Hardening, Documentation, and Rollout Gates
### Goal
Close security/test/docs gaps and lock rollout criteria.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add focused markdown safety regression vectors (server + web):
  - raw HTML payloads
  - obfuscated/disallowed link schemes
  - malformed token payloads
  - oversized or malformed fenced code tokens/language labels
- [ ] Add/refresh docs:
  - `docs/API.md` profile and banner contracts
  - `docs/API.md` markdown token contract updates (including fenced code/lang tokens)
  - gateway event docs for profile banner updates
- [ ] Update active planning logs (`plans/PLAN_UX.md` progress entry after implementation).

### Tentative File Touch List
- Docs:
  - `docs/API.md`
  - `docs/GATEWAY_EVENTS.md`
  - `docs/SECURITY.md` (if markdown/code highlighting trust model needs explicit note)
- Plans/log:
  - `plans/PLAN_UX.md`
- CI/security policy (only if needed for new dependency allowlist):
  - `cargo-deny.toml` (Rust-side only if touched; expected none for this sprint)

### Validation Gate
- [ ] `cargo fmt`
- [ ] `cargo clippy`
- [ ] `cargo test`
- [ ] `pnpm -C apps/filament-client-web run typecheck`
- [ ] `pnpm -C apps/filament-client-web run test`

### Exit Criteria
- Security posture is unchanged or improved, tests are green, and contracts/docs match implementation.

---

## Risks and Mitigations
- Risk: markdown rendering drift between profile and messages.
  - Mitigation: one shared `SafeMarkdown` rendering path and shared test vectors.
- Risk: fenced code highlighting introduces HTML/XSS regression.
  - Mitigation: choose tokenizer/AST highlighter only; render spans via JSX, never inject highlighter HTML output.
- Risk: image upload attack surface expansion (banner route).
  - Mitigation: reuse avatar hardening pattern exactly (cap, sniff, hash, fail-closed writes).
- Risk: stale cached images after update.
  - Mitigation: versioned URLs + gateway banner update event to bump client cache keys.

## Open Questions
- For highlighted fenced code blocks, do we want a strict v1 allowlist (for example 20-30 common languages) or full library language packs with a cap on enabled grammars?

## Execution Slicing (Tentative PR Breakdown)
1. PR-A: Markdown token contract extension + parser tests (core/domain/docs contract draft).
2. PR-B: Message list markdown rendering + secure fenced code highlighting + frontend tests.
3. PR-C: Banner backend (schema/routes/events/tests).
4. PR-D: Banner/profile settings + overlay wiring/tests.
5. PR-E: Final docs/security hardening pass + full validation gate run.
