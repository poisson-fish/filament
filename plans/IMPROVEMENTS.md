# IMPROVEMENTS.md

## Purpose
This document tracks architecture improvements in two independent passes:
1. Frontend context (`apps/filament-client-web`)
2. Backend context (`filament-server` + shared Rust crates)

Each pass should be actionable on its own while staying comparable through a shared structure.

## How To Iterate Across Separate Contexts
- Update only the relevant pass section for the current context.
- Keep completed items immutable; append deltas in the "Iteration Log".
- Preserve security posture from `AGENTS.md` (no relaxed limits/timeouts/validation).
- For each proposed change, include: objective, risk, smallest safe increment, success metric.

## Shared Scoring Rubric
Use this rubric in each pass for objective comparison.

| Dimension | 1 (Weak) | 3 (Moderate) | 5 (Strong) |
| --- | --- | --- | --- |
| Modularity | Large mixed modules, unclear seams | Some separation, partial seams | Small focused modules with clear boundaries |
| Correctness Safety | Sparse guards/tests | Good tests in core paths | Broad characterization + boundary contract tests |
| Security Hardening | Inconsistent input constraints | Core constraints present | Strict boundary validation + limits + hostile-input assumptions throughout |
| Operability | Hard to diagnose failures | Basic status/error surfacing | Explicit metrics/logging/debug signals and deterministic failure modes |
| Change Velocity | Refactors risky/slower | Manageable with care | Incremental changes are predictable and low-risk |

---

## Frontend Pass (Completed First Draft)
Date: 2026-02-12
Scope: `apps/filament-client-web`

### Evidence Sampled
- Routing/auth shell: `src/main.tsx`, `src/App.tsx`, `src/components/RouteGuards.tsx`
- Composition runtime: `src/pages/AppShellPage.tsx`, `src/features/app-shell/runtime/create-app-shell-runtime.ts`
- Boundaries/domain: `src/lib/api.ts`, `src/lib/gateway.ts`, `src/domain/auth.ts`
- State/selectors/controllers: `src/features/app-shell/state/*`, `src/features/app-shell/selectors/create-app-shell-selectors.ts`, `src/features/app-shell/controllers/*`
- Hardening + budget: `security/csp.json`, `scripts/check-app-shell-size.mjs`, `tests/api-boundary.test.ts`
- Refactor history: `plans/PLAN_WEB_REFACTOR.md`

### Objective Architecture Snapshot
- App uses a thin route/auth entry and a feature-centric app shell.
- `create-app-shell-runtime.ts` acts as the composition root wiring multiple controllers and state slices.
- Domain newtypes and invariant constructors exist in `src/domain/*` and are used at API/gateway parsing boundaries.
- Security-first client constraints are explicit (strict CSP, bounded response parsing, timeout defaults, no HTML rendering path in sampled files).
- Test coverage is broad and feature-oriented (48 test files).

### Baseline Metrics
- `src/pages/AppShellPage.tsx`: 312 lines
- `src/features/app-shell/runtime/create-app-shell-runtime.ts`: 1048 lines
- `src/lib/api.ts`: 1293 lines
- `src/lib/gateway.ts`: 2259 lines
- Frontend test files: 48

### Scorecard (Current)
| Dimension | Score | Rationale |
| --- | --- | --- |
| Modularity | 4/5 | Strong controller/state extraction, but runtime + transport layers remain very large |
| Correctness Safety | 4/5 | Extensive targeted tests, especially boundary/controller behavior |
| Security Hardening | 5/5 | Clear hostile-input posture with caps/timeouts/CSP and strict domain parsing |
| Operability | 3/5 | Good user-facing status/error mapping, limited explicit client diagnostics/telemetry seams |
| Change Velocity | 3/5 | Most UI changes are safe; large runtime/transport files still concentrate change risk |

### Frontend Improvement Backlog
| ID | Priority | Area | Observation | Proposed Increment | Success Metric |
| --- | --- | --- | --- | --- | --- |
| FE-01 | P0 | Runtime composition | `create-app-shell-runtime.ts` is a >1k-line orchestration hub with many responsibilities | Split runtime into bounded domain composers (`workspace`, `messaging`, `voice`, `overlay`) with a thin top-level factory | Runtime file < 700 lines and no behavior regressions in existing controller/runtime tests |
| FE-02 | P0 | Gateway boundary | `lib/gateway.ts` is a very large mixed parser/transport/event-routing unit | Extract versioned event decoder registry (`{v,t,d}`) and per-domain event modules; keep strict caps | Gateway core < 900 lines; add contract tests for unknown event type/version handling |
| FE-03 | P0 | API boundary | `lib/api.ts` centralizes all endpoints and response handling into one large module | Keep shared bounded request primitives but split endpoint groups (`auth`, `messages`, `workspace`, `friends`, `voice`) | API modules independently testable; unchanged boundary-hardening tests pass |
| FE-04 | P1 | Async state model | Many independent status/error strings increase implicit state combinations | Introduce typed async operation states for high-churn flows (message send/history refresh/voice join) | Fewer impossible UI states; add reducer/state transition tests |
| FE-05 | P1 | Dependency boundaries | Feature internals are mostly clean, but imports rely on convention | Add lint/import-boundary rules to enforce `domain -> lib -> feature UI` layering | CI fails on boundary violations; no new cross-layer leaks |
| FE-06 | P1 | Operability | Failure mapping is deterministic, but diagnostics are mostly local UI status text | Add explicit client diagnostics hooks (structured event counters + dev-only debug panel wiring) without exposing sensitive data | Faster triage of gateway/API failures; no token leakage in logs/tests |
| FE-07 | P2 | Rendering scalability | Message/voice surfaces can grow in dense channels | Evaluate targeted list virtualization for message list while preserving scroll/history semantics | Stable scroll behavior with large histories and no regression in history scroll tests |

Item Status
- FE-01: Completed on 2026-02-13 (all planned slices 1–32 completed; runtime composition root `src/features/app-shell/runtime/create-app-shell-runtime.ts` reduced to 645 lines, meeting the FE-01 `<700 lines` success metric while preserving prior focused runtime/controller test coverage).
- FE-02: In progress (slices 1–12 completed on 2026-02-14: extracted strict `{v,t,d}` envelope parsing into `src/lib/gateway-envelope.ts`, extracted presence-domain decoder registry/parsing into `src/lib/gateway-presence-events.ts`, extracted friend-domain decoder registry/parsing into `src/lib/gateway-friend-events.ts`, extracted profile-domain decoder registry/parsing into `src/lib/gateway-profile-events.ts`, extracted message-domain decoder registry/parsing into `src/lib/gateway-message-events.ts`, extracted workspace-domain decoder registry/parsing into `src/lib/gateway-workspace-events.ts`, extracted voice-domain dispatch routing into `src/lib/gateway-voice-dispatch.ts`, extracted presence-domain dispatch routing into `src/lib/gateway-presence-dispatch.ts`, extracted friend-domain dispatch routing into `src/lib/gateway-friend-dispatch.ts`, extracted profile-domain dispatch routing into `src/lib/gateway-profile-dispatch.ts`, extracted message-domain dispatch routing into `src/lib/gateway-message-dispatch.ts`, and extracted workspace-domain dispatch routing into `src/lib/gateway-workspace-dispatch.ts` with `src/lib/gateway.ts` rewired to consume dedicated dispatch modules while preserving fail-closed behavior).

### Recommended Iteration Order (Frontend)
1. FE-01 Runtime decomposition
2. FE-02 Gateway module split + decoder registry
3. FE-03 API module split
4. FE-04 Typed async state transitions
5. FE-05/FE-06 guardrails + diagnostics
6. FE-07 scalability optimization

### Frontend Target Architecture
- Keep a single top-level composition root, but keep it thin and wiring-only.
- Move behavior/policy/state transition logic into domain composers and focused boundary modules.
- Preserve strict boundary hardening (fail-closed parsing, payload caps, timeout expectations, and hostile-input assumptions).

### Frontend Exit Criteria Addendum
- FE-02 exit criteria:
	- Gateway dispatch uses an explicit decoder registry for versioned `{v,t,d}` events.
	- Event decoder concerns are split into domain-focused modules; `src/lib/gateway.ts` remains transport/wiring focused.
	- Contract tests for unknown event type/version and oversized envelope handling remain green.
- FE-03 exit criteria:
	- API client is split into domain modules (`auth`, `messages`, `workspace`, `friends`, `voice`) behind shared bounded request primitives.
	- Existing API boundary hardening behavior is preserved with focused boundary tests.

### Frontend Anti-Regression Guardrail
- Composition-root rule: no new domain behavior/policy logic is added to `create-app-shell-runtime.ts`; only orchestration/wiring is allowed.

### Frontend Iteration Log
- 2026-02-12: Initial objective assessment captured; no code behavior changed in this pass.
- 2026-02-12: FE-01 slice 1 completed. Extracted overlay/settings panel action orchestration into `src/features/app-shell/runtime/overlay-panel-actions.ts`, wired runtime to use it, and added targeted tests in `tests/app-shell-overlay-panel-actions.test.ts` (3 passing).
- 2026-02-12: FE-01 slice 2 completed. Extracted workspace settings save orchestration into `src/features/app-shell/runtime/workspace-settings-actions.ts`, wired `create-app-shell-runtime.ts` to consume the new helper, and added targeted tests in `tests/app-shell-workspace-settings-actions.test.ts` (3 passing).
- 2026-02-12: FE-01 slice 3 completed. Extracted voice device inventory/preference orchestration from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/voice-device-actions.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-voice-device-actions.test.ts` (3 passing).
- 2026-02-12: FE-01 slice 4 completed. Extracted gateway workspace permission refresh orchestration from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/workspace-permission-actions.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-workspace-permission-actions.test.ts` (3 passing).
- 2026-02-12: FE-01 slice 5 completed. Extracted workspace/channel selection panel actions from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/workspace-selection-actions.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-workspace-selection-actions.test.ts` (3 passing).
- 2026-02-12: FE-01 slice 6 completed. Extracted workspace settings panel prop composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/workspace-settings-panel-props.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-workspace-settings-panel-props.test.ts` (3 passing).
- 2026-02-12: FE-01 slice 7 completed. Extracted runtime side-effect registrations from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/runtime-effects.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-runtime-effects.test.ts` (4 passing).
- 2026-02-12: FE-01 slice 7 follow-up completed. Aligned `overlay-panel-actions.ts` option typings with Solid setter/accessor contracts, updated `tests/app-shell-overlay-panel-actions.test.ts` to match strict `OverlayPanel` signal typing, and re-ran targeted validation (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-overlay-panel-actions.test.ts tests/app-shell-runtime-effects.test.ts`) with all checks passing.
- 2026-02-12: FE-01 slice 8 completed. Extracted role-management panel prop composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/role-management-panel-props.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-role-management-panel-props.test.ts` (2 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-role-management-panel-props.test.ts`).
- 2026-02-12: FE-01 slice 9 completed. Extracted friendships panel prop composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/friendships-panel-props.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-friendships-panel-props.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-friendships-panel-props.test.ts`).
- 2026-02-12: FE-01 slice 10 completed. Extracted search panel prop composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/search-panel-props.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-search-panel-props.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-search-panel-props.test.ts`).
- 2026-02-12: FE-01 slice 11 completed. Extracted moderation panel prop composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/moderation-panel-props.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-moderation-panel-props.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-moderation-panel-props.test.ts`).
- 2026-02-12: FE-01 slice 12 completed. Extracted attachments panel prop composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/attachments-panel-props.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-attachments-panel-props.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-attachments-panel-props.test.ts`).
- 2026-02-12: FE-01 slice 13 completed. Extracted client settings panel prop composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/client-settings-panel-props.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-client-settings-panel-props.test.ts` (2 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-client-settings-panel-props.test.ts`).
- 2026-02-12: FE-01 slice 14 completed. Extracted utility panel prop composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/utility-panel-props.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-utility-panel-props.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-utility-panel-props.test.ts`).
- 2026-02-12: FE-01 slice 15 completed. Extracted public-directory panel prop composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/public-directory-panel-props.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-public-directory-panel-props.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-public-directory-panel-props.test.ts`).
- 2026-02-12: FE-01 slice 16 completed. Extracted workspace-create panel prop composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/workspace-create-panel-props.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-workspace-create-panel-props.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-workspace-create-panel-props.test.ts`).
- 2026-02-12: FE-01 slice 17 completed. Extracted channel-create panel prop composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/channel-create-panel-props.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-channel-create-panel-props.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-channel-create-panel-props.test.ts`).
- 2026-02-12: FE-01 slice 18 completed. Extracted session diagnostics orchestration wiring from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/session-diagnostics-actions.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-session-diagnostics-actions.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-session-diagnostics-actions.test.ts tests/app-shell-session-diagnostics-controller.test.ts`).
- 2026-02-12: FE-01 slice 19 completed. Extracted workspace/channel create panel group composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/workspace-channel-create-panel-groups.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-workspace-channel-create-panel-groups.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-workspace-channel-create-panel-groups.test.ts`).
- 2026-02-12: FE-01 slice 20 completed. Extracted grouped collaboration panel prop composition (friendships/search/attachments/moderation) from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/collaboration-panel-prop-groups.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-collaboration-panel-prop-groups.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-collaboration-panel-prop-groups.test.ts`).
- 2026-02-12: FE-01 slice 21 completed. Extracted grouped support panel prop composition (public directory/settings/workspace settings/role management/utility) from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/support-panel-prop-groups.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-support-panel-prop-groups.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-support-panel-prop-groups.test.ts`).
- 2026-02-12: FE-01 slice 22 completed. Extracted gateway-controller registration wiring from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/gateway-controller-registration.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-gateway-controller-registration.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-gateway-controller-registration.test.ts`).
- 2026-02-12: FE-01 slice 23 completed. Extracted panel-host prop-group composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/panel-host-prop-groups.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-panel-host-prop-groups.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-panel-host-prop-groups.test.ts`).
- 2026-02-13: FE-01 slice 24 completed. Extracted workspace/channel-create panel-group state option composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/workspace-channel-create-panel-groups-options.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-workspace-channel-create-panel-groups-options.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-workspace-channel-create-panel-groups-options.test.ts tests/app-shell-workspace-channel-create-panel-groups.test.ts`).
- 2026-02-13: FE-01 slice 25 completed. Extracted support panel-group state option composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/support-panel-prop-groups-options.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-support-panel-prop-groups-options.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-support-panel-prop-groups-options.test.ts tests/app-shell-support-panel-prop-groups.test.ts`).
- 2026-02-13: FE-01 slice 26 completed. Extracted collaboration panel-group state option composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/collaboration-panel-prop-groups-options.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-collaboration-panel-prop-groups-options.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-collaboration-panel-prop-groups-options.test.ts tests/app-shell-collaboration-panel-prop-groups.test.ts`).
- 2026-02-13: FE-01 slice 27 completed. Extracted panel-host grouped state option composition from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/panel-host-prop-groups-options.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-panel-host-prop-groups-options.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-panel-host-prop-groups-options.test.ts tests/app-shell-panel-host-prop-groups.test.ts`).
- 2026-02-13: FE-01 slice 28 completed. Extracted workspace/channel-create panel-host state option wiring from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/workspace-channel-create-panel-host-state-options.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-workspace-channel-create-panel-host-state-options.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-workspace-channel-create-panel-host-state-options.test.ts tests/app-shell-workspace-channel-create-panel-groups-options.test.ts`).
- 2026-02-13: FE-01 slice 29 completed. Extracted collaboration panel-host state option wiring from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/collaboration-panel-host-state-options.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-collaboration-panel-host-state-options.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-collaboration-panel-host-state-options.test.ts tests/app-shell-panel-host-prop-groups-options.test.ts`).
- 2026-02-13: FE-01 slice 30 completed. Extracted support panel-host state option wiring from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/support-panel-host-state-options.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-support-panel-host-state-options.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-support-panel-host-state-options.test.ts tests/app-shell-support-panel-prop-groups-options.test.ts`).
- 2026-02-13: FE-01 slice 31 completed. Extracted panel-host prop-group factory wiring from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/panel-host-prop-groups-factory.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-panel-host-prop-groups-factory.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-panel-host-prop-groups-factory.test.ts`).
- 2026-02-13: FE-01 slice 32 completed. Extracted workspace/channel operations controller wiring from `src/features/app-shell/runtime/create-app-shell-runtime.ts` into `src/features/app-shell/runtime/workspace-channel-operations-actions.ts`, wired runtime to consume the new helper, and added targeted tests in `tests/app-shell-workspace-channel-operations-actions.test.ts` (1 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/app-shell-workspace-channel-operations-actions.test.ts`).
- 2026-02-13: FE-01 marked complete after verifying `src/features/app-shell/runtime/create-app-shell-runtime.ts` is 645 lines (`wc -l`) and confirming FE-01 success criteria are met.
- 2026-02-13: FE-02 slice 1 completed. Extracted strict gateway envelope parsing from `src/lib/gateway.ts` into `src/lib/gateway-envelope.ts`, rewired gateway dispatch to use `parseGatewayEventEnvelope`, and added focused tests in `tests/gateway-envelope.test.ts` (5 passing). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/gateway-envelope.test.ts tests/gateway.test.ts`).
- 2026-02-13: FE-02 slice 2 completed. Extracted presence-domain decoder registry and payload parsing from `src/lib/gateway.ts` into `src/lib/gateway-presence-events.ts`, rewired gateway dispatch to consume `decodePresenceGatewayEvent` for `presence_sync`/`presence_update`, and added focused tests in `tests/gateway-presence-events.test.ts` (3 passing, including unknown-type fail-closed behavior). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/gateway-presence-events.test.ts tests/gateway.test.ts`).
- 2026-02-13: FE-02 slice 3 completed. Extracted friend-domain decoder registry and payload parsing from `src/lib/gateway.ts` into `src/lib/gateway-friend-events.ts`, rewired gateway dispatch to consume `decodeFriendGatewayEvent` for `friend_request_create`/`friend_request_update`/`friend_request_delete`/`friend_remove`, and added focused tests in `tests/gateway-friend-events.test.ts` (4 passing, including unknown-type and invalid-payload fail-closed behavior). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/gateway-friend-events.test.ts tests/gateway.test.ts`).
- 2026-02-13: FE-02 slice 4 completed. Extracted profile-domain decoder registry and payload parsing from `src/lib/gateway.ts` into `src/lib/gateway-profile-events.ts`, rewired gateway dispatch to consume `decodeProfileGatewayEvent` for `profile_update`/`profile_avatar_update`, and added focused tests in `tests/gateway-profile-events.test.ts` (4 passing, including unknown-type and invalid-payload fail-closed behavior). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/gateway-profile-events.test.ts tests/gateway.test.ts`).
- 2026-02-13: FE-02 slice 5 completed. Extracted message-domain decoder registry and payload parsing from `src/lib/gateway.ts` into `src/lib/gateway-message-events.ts`, rewired gateway dispatch to consume `decodeMessageGatewayEvent` for `message_create`/`message_update`/`message_delete`/`message_reaction`, and added focused tests in `tests/gateway-message-events.test.ts` (5 passing, including unknown-type and invalid-payload fail-closed behavior). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/gateway-message-events.test.ts tests/gateway.test.ts`).
- 2026-02-13: FE-02 slice 6 completed. Extracted workspace-domain decoder registry and payload parsing (`channel_create` + `workspace_*`) from `src/lib/gateway.ts` into `src/lib/gateway-workspace-events.ts`, rewired gateway dispatch to consume `decodeWorkspaceGatewayEvent`, and added focused tests in `tests/gateway-workspace-events.test.ts` (5 passing, including unknown-type and invalid-payload fail-closed behavior plus reorder-cap enforcement). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/gateway-workspace-events.test.ts tests/gateway.test.ts`).
- 2026-02-14: FE-02 slice 7 completed. Extracted voice-domain dispatch routing from `src/lib/gateway.ts` into `src/lib/gateway-voice-dispatch.ts`, rewired gateway message handling to use `dispatchVoiceGatewayEvent`, and added focused tests in `tests/gateway-voice-dispatch.test.ts` (3 passing, including non-voice-type passthrough and invalid-payload fail-closed behavior). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck && pnpm -C apps/filament-client-web exec vitest run tests/gateway-voice-dispatch.test.ts tests/gateway.test.ts` -> 18/18 tests passing).
- 2026-02-14: FE-02 slice 8 completed. Extracted presence-domain dispatch routing from `src/lib/gateway.ts` into `src/lib/gateway-presence-dispatch.ts`, rewired gateway message handling to use `dispatchPresenceGatewayEvent`, and added focused tests in `tests/gateway-presence-dispatch.test.ts` (3 passing, including non-presence-type passthrough and invalid-payload fail-closed behavior). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/gateway-presence-dispatch.test.ts tests/gateway.test.ts` -> 18/18 tests passing).
- 2026-02-14: FE-02 slice 9 completed. Extracted friend-domain dispatch routing from `src/lib/gateway.ts` into `src/lib/gateway-friend-dispatch.ts`, rewired gateway message handling to use `dispatchFriendGatewayEvent`, and added focused tests in `tests/gateway-friend-dispatch.test.ts` (3 passing, including non-friend-type passthrough and invalid-payload fail-closed behavior). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/gateway-friend-dispatch.test.ts tests/gateway.test.ts` -> 18/18 tests passing).
- 2026-02-14: FE-02 slice 10 completed. Extracted profile-domain dispatch routing from `src/lib/gateway.ts` into `src/lib/gateway-profile-dispatch.ts`, rewired gateway message handling to use `dispatchProfileGatewayEvent`, and added focused tests in `tests/gateway-profile-dispatch.test.ts` (3 passing, including non-profile-type passthrough and invalid-payload fail-closed behavior). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck && pnpm -C apps/filament-client-web exec vitest run tests/gateway-profile-dispatch.test.ts tests/gateway.test.ts` -> 18/18 tests passing).
- 2026-02-14: FE-02 slice 11 completed. Extracted message-domain dispatch routing from `src/lib/gateway.ts` into `src/lib/gateway-message-dispatch.ts`, rewired gateway message handling to use `dispatchMessageGatewayEvent`, and added focused tests in `tests/gateway-message-dispatch.test.ts` (3 passing, including non-message-type passthrough and invalid-payload fail-closed behavior). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/gateway-message-dispatch.test.ts tests/gateway.test.ts` -> 18/18 tests passing).
- 2026-02-14: FE-02 slice 12 completed. Extracted workspace-domain dispatch routing from `src/lib/gateway.ts` into `src/lib/gateway-workspace-dispatch.ts`, rewired gateway message handling to use `dispatchWorkspaceGatewayEvent`, and added focused tests in `tests/gateway-workspace-dispatch.test.ts` (3 passing, including non-workspace-type passthrough and invalid-payload fail-closed behavior). Targeted validation passed (`pnpm -C apps/filament-client-web run typecheck`, `pnpm -C apps/filament-client-web exec vitest run tests/gateway-workspace-dispatch.test.ts tests/gateway.test.ts` -> 18/18 tests passing).

---

## Backend Pass (Completed First Draft)
Date: 2026-02-12
Scope: `apps/filament-server`, `crates/filament-core`, `crates/filament-protocol`

### Evidence Sampled
- Runtime composition and config: `apps/filament-server/src/main.rs`, `src/server/mod.rs`, `src/server/core.rs`
- Boundary/router surface: `apps/filament-server/src/server/router.rs`
- Auth/security boundaries: `apps/filament-server/src/server/auth.rs`, `docs/SECURITY.md`
- Realtime and protocol: `apps/filament-server/src/server/realtime.rs`, `src/server/gateway_events.rs`, `crates/filament-protocol/src/lib.rs`, `docs/GATEWAY_EVENTS.md`
- Persistence and domain logic: `apps/filament-server/src/server/db.rs`, `src/server/domain.rs`, `src/server/permissions.rs`
- Tests/contracts: `apps/filament-server/tests/security_limits.rs`, `src/server/tests.rs`, `docs/API.md`

### Objective Architecture Snapshot
- Server follows a layered split (`router` -> `handlers` -> `domain`/`db`/`realtime`) with shared `AppState` and runtime security config.
- Security controls are explicit and centralized in config defaults + router validation (body caps, timeouts, route/IP rate limits, gateway ingress caps, token TTL caps).
- Protocol boundary is strongly versioned via `filament-protocol` (`{v,t,d}`, strict event type validation, max payload enforcement).
- Persistence supports Postgres runtime with in-memory fallback paths retained for tests/non-DB flows.
- Search and voice integrations are policy-driven by server-side checks (permissions, quotas, scoped token issuance), consistent with hostile-client assumptions.

### Baseline Metrics
- `apps/filament-server/src/server/realtime.rs`: 1712 lines
- `apps/filament-server/src/server/gateway_events.rs`: 1501 lines
- `apps/filament-server/src/server/domain.rs`: 1583 lines
- `apps/filament-server/src/server/db.rs`: 941 lines
- `apps/filament-server/src/server/tests.rs`: 2404 lines
- `apps/filament-server/tests/*.rs`: 10 files
- Shared crate test files (`crates/filament-core/tests`, `crates/filament-protocol/tests`): 1 file

### Scorecard (Current)
| Dimension | Score | Rationale |
| --- | --- | --- |
| Modularity | 3/5 | Clear module boundaries exist, but realtime/domain/event building remain concentrated in very large files |
| Correctness Safety | 4/5 | Strong contract/security tests and explicit policy docs; some monolithic test files reduce local change confidence |
| Security Hardening | 5/5 | Consistent hard limits, strict auth/protocol checks, hostile-input posture across HTTP/WS/token paths |
| Operability | 4/5 | Structured logging, request IDs, metrics endpoints, and gateway-specific counters are in place |
| Change Velocity | 3/5 | Feature work is possible, but large hotspots increase merge risk and refactor cost |

### Backend Improvement Backlog
| ID | Priority | Area | Observation | Proposed Increment | Success Metric |
| --- | --- | --- | --- | --- | --- |
| BE-01 | P0 | Realtime orchestration | `realtime.rs` mixes WS lifecycle, ingress parsing, message ops, presence/voice/search flows | Split into bounded modules (`connection`, `ingress`, `fanout`, `voice_presence`, `search_ops`) under `server/realtime/` | `realtime` root module < 700 lines; existing gateway tests remain green |
| BE-02 | P0 | Event emission model | `gateway_events.rs` centralizes many payload builders and event constants in one file | Introduce per-domain event emitter modules and shared typed envelope adapter | Event module files are domain-scoped; event contract tests added/updated per domain |
| BE-03 | P0 | Domain service concentration | `domain.rs` combines permission resolution, moderation/IP-ban enforcement, attachments/reactions flows | Extract service-focused submodules (`permissions_eval`, `moderation`, `attachments`, `reactions`) with narrow APIs | Domain root file < 800 lines and no permission regression in role/override tests |
| BE-04 | P1 | DB schema/migration ownership | `db.rs` contains many DDL/backfill concerns alongside runtime helpers | Move schema bootstrap and backfills into versioned migration units and keep runtime DB access in focused repository modules | Reduced churn in `db.rs`; migration tests verify idempotent startup and role/override backfills |
| BE-05 | P1 | Test topology | `src/server/tests.rs` is a large integration omnibus | Split tests by capability (`auth`, `guilds`, `roles`, `gateway`, `directory`, `voice`) with shared fixture helpers | Smaller test files, easier targeted runs, unchanged total behavior coverage |
| BE-06 | P1 | API surface governance | Router currently declares many routes inline; growth risks drift against docs/contracts | Add generated/validated route manifest checks tied to `docs/API.md` and gateway contract docs | CI catches undocumented route/event drift before merge |
| BE-07 | P2 | Runtime state abstraction | `AppState` holds many maps/queues directly, increasing cognitive load and lock-scope risk | Introduce bounded state components (auth/session store, membership store, realtime registry) behind typed facades | Lower lock contention in hotspots and clearer ownership in code review |

### Recommended Iteration Order (Backend)
1. BE-01 Realtime module decomposition
2. BE-02 Event emitter modularization
3. BE-03 Domain service extraction
4. BE-05 Test topology split (parallel with BE-01..03)
5. BE-04 DB migration/repository separation
6. BE-06 Contract drift guardrails
7. BE-07 AppState componentization

### Backend Iteration Log
- 2026-02-12: Initial objective backend assessment captured; no server behavior changed in this pass.