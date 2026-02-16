# IMPROVEMENTS_BE.md

## Purpose
This document tracks backend architecture improvements for:
1. `filament-server`
2. shared Rust crates (`crates/filament-core`, `crates/filament-protocol`)

## How To Iterate in Backend Context
- Update only backend sections in this file.
- Keep completed items immutable; append deltas in the "Iteration Log".
- Preserve security posture from `AGENTS.md` (no relaxed limits/timeouts/validation).
- For each proposed change, include: objective, risk, smallest safe increment, success metric.

## Shared Scoring Rubric
Use this rubric for objective comparison.

| Dimension | 1 (Weak) | 3 (Moderate) | 5 (Strong) |
| --- | --- | --- | --- |
| Modularity | Large mixed modules, unclear seams | Some separation, partial seams | Small focused modules with clear boundaries |
| Correctness Safety | Sparse guards/tests | Good tests in core paths | Broad characterization + boundary contract tests |
| Security Hardening | Inconsistent input constraints | Core constraints present | Strict boundary validation + limits + hostile-input assumptions throughout |
| Operability | Hard to diagnose failures | Basic status/error surfacing | Explicit metrics/logging/debug signals and deterministic failure modes |
| Change Velocity | Refactors risky/slower | Manageable with care | Incremental changes are predictable and low-risk |

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

Item Status
- BE-01: In progress status delta on 2026-02-15 (slices 1-2 completed: extracted gateway ingress rate-limit enforcement into `apps/filament-server/src/server/realtime/ingress_rate_limit.rs` with focused unit coverage, extracted gateway ingress websocket message decoding into `apps/filament-server/src/server/realtime/ingress_message.rs` with focused unit coverage, and rewired `apps/filament-server/src/server/realtime.rs` to delegate both paths while preserving fail-closed event-size and disconnect handling semantics).
- BE-01: In progress status delta on 2026-02-15 (slices 3-4 completed: extracted channel fanout dispatch helper into `apps/filament-server/src/server/realtime/fanout_dispatch.rs` with focused unit coverage for delivered/closed/full queue paths, extracted voice presence helper functions into `apps/filament-server/src/server/realtime/voice_presence.rs` with focused unit coverage for key derivation and snapshot mapping, and rewired `apps/filament-server/src/server/realtime.rs` to delegate both helpers while preserving existing disconnect and event emission behavior).
- BE-01: In progress status delta on 2026-02-15 (slices 5-8 completed: extracted guild fanout dispatch into `apps/filament-server/src/server/realtime/fanout_guild.rs`, extracted user fanout dispatch into `apps/filament-server/src/server/realtime/fanout_user.rs`, extracted presence subscribe transition logic into `apps/filament-server/src/server/realtime/presence_subscribe.rs`, extracted disconnect presence/offline-guild outcome logic into `apps/filament-server/src/server/realtime/presence_disconnect.rs`, and rewired `apps/filament-server/src/server/realtime.rs` to delegate these paths while preserving fail-closed queue handling and scoped presence fanout semantics; BE-01 remains open because `apps/filament-server/src/server/realtime.rs` is still 1595 lines vs. the <700 completion target).

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
- 2026-02-15: BE-01 slice 1 completed. Extracted ingress rate-limit window handling from `apps/filament-server/src/server/realtime.rs` into `apps/filament-server/src/server/realtime/ingress_rate_limit.rs` and added focused unit tests covering under-limit allow, at-limit reject, and expired-entry eviction behavior. Targeted validation passed (`cargo test -p filament-server ingress_rate_limit` -> 3 passed, 0 failed).
- 2026-02-15: BE-01 slice 2 completed. Extracted websocket ingress message decoding (text/binary payload caps plus close/ping handling) from `apps/filament-server/src/server/realtime.rs` into `apps/filament-server/src/server/realtime/ingress_message.rs` and added focused unit tests for payload decode, oversized binary fail-closed disconnect, close disconnect reason mapping, and ping passthrough. Targeted validation passed (`cargo test -p filament-server ingress_message` -> 4 passed, 0 failed).
- 2026-02-15: BE-01 slice 3 completed. Extracted channel gateway fanout dispatch helper from `apps/filament-server/src/server/realtime.rs` into `apps/filament-server/src/server/realtime/fanout_dispatch.rs` and added focused unit tests covering open-listener delivery, full-queue slow-consumer marking, and closed-listener pruning behavior. Targeted validation passed (`cargo test -p filament-server fanout_dispatch -- --nocapture` -> 2 passed, 0 failed) and existing guild fanout behavior remained green (`cargo test -p filament-server guild_broadcast_delivers_once_per_connection_and_skips_other_guilds -- --nocapture` -> 1 passed, 0 failed).
- 2026-02-15: BE-01 slice 4 completed. Extracted voice presence helper functions (`voice_channel_key`, `voice_snapshot_from_record`) from `apps/filament-server/src/server/realtime.rs` into `apps/filament-server/src/server/realtime/voice_presence.rs` and added focused unit tests for namespaced key derivation and snapshot field mapping integrity. Targeted validation passed (`cargo test -p filament-server voice_presence -- --nocapture` -> 2 passed, 0 failed) and a focused existing voice integration check remained green (`cargo test -p filament-server --test phase5_livekit_voice voice_token_is_scoped_signed_and_short_lived -- --nocapture` -> 1 passed, 0 failed).
- 2026-02-15: BE-01 slice 5 completed. Extracted guild fanout dispatch and stale-listener pruning from `apps/filament-server/src/server/realtime.rs` into `apps/filament-server/src/server/realtime/fanout_guild.rs` with focused unit tests for per-connection dedupe across guild channels and closed/full queue handling. Targeted validation passed (`cargo test -p filament-server fanout_guild -- --nocapture` -> 2 passed, 0 failed).
- 2026-02-15: BE-01 slice 6 completed. Extracted user fanout dispatch and sender pruning from `apps/filament-server/src/server/realtime.rs` into `apps/filament-server/src/server/realtime/fanout_user.rs` with focused unit tests for targeted delivery and closed/full queue handling. Targeted validation passed (`cargo test -p filament-server fanout_user -- --nocapture` -> 2 passed, 0 failed).
- 2026-02-15: BE-01 slice 7 completed. Extracted presence subscribe transition logic from `apps/filament-server/src/server/realtime.rs` into `apps/filament-server/src/server/realtime/presence_subscribe.rs` with focused unit tests for first-online detection, already-online behavior, and missing-connection fail-closed handling. Targeted validation passed (`cargo test -p filament-server presence_subscribe -- --nocapture` -> 3 passed, 0 failed).
- 2026-02-15: BE-01 slice 8 completed. Extracted disconnect presence outcome logic from `apps/filament-server/src/server/realtime.rs` into `apps/filament-server/src/server/realtime/presence_disconnect.rs` with focused unit tests for offline guild detection and same-user/other-user connection separation. Targeted validation passed (`cargo test -p filament-server presence_disconnect -- --nocapture` -> 3 passed, 0 failed) and focused existing fanout/presence behavior remained green (`cargo test -p filament-server guild_broadcast_delivers_once_per_connection_and_skips_other_guilds -- --nocapture && cargo test -p filament-server user_broadcast_targets_only_requested_authenticated_user -- --nocapture && cargo test -p filament-server --test phase4_roles_presence_reactions presence_events_do_not_leak_across_guilds -- --nocapture` -> 3 passed, 0 failed).
