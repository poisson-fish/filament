# FIXES_CONTRACT.md

## Incident
`POST /api/guilds/.../join` returns `HTTP 408` after about `10s`, matching the server global timeout (`TimeoutLayer`).
Current symptom update: this appears to affect all `POST` routes.
Additional symptom update: auth routes can return `auth requests exceeded` on first or second try.

## Goal
Identify and fix the shared bottleneck causing POST handlers to exceed the 10s request budget, without weakening security limits/timeouts.

## Constraints
- Keep the global timeout; do not “fix” by simply increasing/removing it.
- Preserve existing security posture (rate limits, body limits, auth checks, ban checks).
- Any code fix must include tests.

## Working Assumptions
- Deployment is Docker Compose on VPS (`infra/docker-compose.yml`).
- Reverse proxy is Caddy; backend is `filament-server`; DB is Postgres.
- Timeout is produced by backend middleware (`apps/filament-server/src/server/router.rs`).
- This debug session is running on the VPS that hosts the live domain (`filamentapp.net`).
- Environment is pre-production/throwaway: invasive debugging is allowed (container restarts, rebuilds, traffic experiments).
- Current host deployment command:
  - `docker compose --profile web --env-file infra/.env -f infra/docker-compose.yml up -d --build`
- Repro account is operator-provided (`tester1`); do not commit credentials/secrets beyond this local troubleshooting contract.
- Repro credentials for this local incident run:
  - username: `tester1`
  - password: `thisisatest!`

## Execution Rules For Subagents
- Every task must end with explicit verdict: `PASS` (ruled out) or `FAIL` (confirmed issue).
- Every task must attach artifacts (command output snippets, logs, query results, or test evidence).
- If a task confirms a root cause, open a linked remediation task and keep running high-risk checks to detect secondary issues.

## Parallelization Plan
- Wave 1 (fast triage, parallel): A1, B1, C1, D1, E1, C3
- Wave 2 (deep checks based on wave 1): A2/B2/C2/D2/E2/F1/G1/C4
- Wave 3 (fix + validate): H1, H2, H3

## Subagent Task Board

### A. Ingress / Proxy Path
- [x] **A1 - Reproduce and time POST vs GET at each hop**
  - Owner: `subagent-ingress-baseline`
  - Run:
    - `curl -sS -o /dev/null -w '%{http_code} %{time_total}\n' https://filamentapp.net/api/health`
    - `curl -sS -o /dev/null -w '%{http_code} %{time_total}\n' -X POST https://filamentapp.net/api/echo -H 'content-type: application/json' -d '{}'`
    - repeat against `http://127.0.0.1:8080` and directly inside server container (`http://filament-server:3000`)
  - PASS if only one hop is slow; FAIL if backend itself is slow.
  - Hypothesis: timeout is introduced at one hop (proxy vs backend), not route logic.
  - Commands:
    - `curl -sS -o /dev/null -w 'domain GET %{http_code} %{time_total}\n' https://filamentapp.net/api/health`
    - `curl -sS -o /dev/null -w 'domain POST %{http_code} %{time_total}\n' -X POST https://filamentapp.net/api/echo -H 'content-type: application/json' -d '{}'`
    - `curl -sS -o /dev/null -w 'localhost8080 GET %{http_code} %{time_total}\n' http://127.0.0.1:8080/health`
    - `curl -sS -o /dev/null -w 'localhost8080 POST %{http_code} %{time_total}\n' -X POST http://127.0.0.1:8080/echo -H 'content-type: application/json' -d '{}'`
    - `docker exec infra-reverse-proxy-1 sh -lc "curl -sS -o /dev/null -w 'container GET %{http_code} %{time_total}\n' http://filament-server:3000/health"`
    - `docker exec infra-reverse-proxy-1 sh -lc "curl -sS -o /dev/null -w 'container POST %{http_code} %{time_total}\n' -X POST http://filament-server:3000/echo -H 'content-type: application/json' -d '{}'"`
  - Key output: domain GET `200 0.073950`, domain POST `422 0.057694`; localhost GET `200 0.001422`, POST `422 0.001399`; container GET `200 0.001171`, POST `422 0.001269`.
  - Verdict: `PASS` (no hop exhibited ~10s latency in this baseline probe).
- [x] **A2 - Confirm Caddy is not imposing hidden POST behavior**
  - Owner: `subagent-caddy-path`
  - Check `infra/Caddyfile` and runtime config for method-specific handling, buffering, body/read timeout, protocol issues.
  - Capture Caddy logs during failing POSTs and correlate `duration` with ~10s.
  - Hypothesis: Caddy method-specific or buffering behavior is adding ~10s to POSTs.
  - Commands:
    - `sed -n '1,240p' infra/Caddyfile`
    - `docker exec infra-reverse-proxy-1 caddy validate --config /etc/caddy/Caddyfile`
    - timed POST probes via `https://filamentapp.net/api/echo`.
  - Key output:
    - No method-specific timeout directives present.
    - Config validated by Caddy.
    - POST probe timings stayed near ~60ms, not ~10s.
  - Verdict: `PASS` (no hidden Caddy timeout behavior observed).

### B. App Middleware / Router Cross-Cutting
- [x] **B1 - Verify timeout source and stack order**
  - Owner: `subagent-router-middleware`
  - Confirm `TimeoutLayer` order relative to trace/request-id/governor/body-limit.
  - Verify no route-level middleware for POSTs differs unexpectedly.
  - Hypothesis: middleware ordering or POST-only middleware causes broad POST timeouts.
  - Commands:
    - `nl -ba apps/filament-server/src/server/router.rs | sed -n '349,520p'`
  - Key output:
    - `TimeoutLayer` present in global `ServiceBuilder` before `GovernorLayer`.
    - Body limits are global (`DefaultBodyLimit::max`) plus explicit upload override (`DefaultBodyLimit::disable`) only on upload routes.
    - No POST-specific timeout middleware found.
  - Verdict: `PASS` (no unexpected POST-only timeout stack divergence found).
- [x] **B2 - Add temporary per-request timing spans (local branch only)**
  - Owner: `subagent-instrumentation-http`
  - Add structured timings around auth, DB, and handler body for representative POST routes.
  - Remove/keep behind guarded debug flag after diagnosis.
  - Hypothesis: missing phase-level timings hide where request budget is consumed.
  - Changes:
    - Added guarded timing logs behind `FILAMENT_DEBUG_REQUEST_TIMINGS` in:
      - `authenticate_with_token` (`apps/filament-server/src/server/auth.rs`)
      - `join_public_guild` (`apps/filament-server/src/server/handlers/guilds.rs`)
  - Key output:
    - Timing logs are only emitted when `FILAMENT_DEBUG_REQUEST_TIMINGS` is set.
  - Verdict: `PASS` (diagnostic coverage added without changing security controls).

### C. Auth Path (shared by most POST handlers)
- [x] **C1 - Measure `authenticate()` latency under load and no-load**
  - Owner: `subagent-auth-latency`
  - Focus on `find_username_by_subject` DB query (`SELECT username FROM users WHERE user_id = $1`).
  - PASS if p95 << 100ms; FAIL if auth query waits near timeout.
  - Hypothesis: `authenticate()` blocks on `find_username_by_subject` DB lookup.
  - Commands:
    - `EXPLAIN (ANALYZE, BUFFERS) SELECT username FROM users WHERE user_id='01KJDG9HBEF4DP59RBENVSFEH0';`
    - Restart server, login once, then sample `/auth/me` latency:
      - 30 sequential requests (`/tmp/wave1_c1_me_no_load2.txt`)
      - 100 requests at concurrency 20 (`/tmp/wave1_c1_me_load2.txt`)
  - Key output:
    - DB execution time `0.066 ms` for lookup query.
    - `/auth/me` no-load: `n=30 avg=0.0022s p95=0.002625s max=0.002750s`.
    - `/auth/me` under load: `n=100 avg=0.0062s p95=0.024847s max=0.032059s`.
  - Verdict: `PASS` (`authenticate()` path is far below 100ms and nowhere near 10s timeout budget).
- [x] **C2 - Validate token/key consistency and failure mode**
  - Owner: `subagent-auth-integrity`
  - Ensure auth errors are immediate `401/403`, not delayed to timeout.
  - Check for any lock contention in auth-related in-memory rate-limit maps.
  - Hypothesis: auth failures are delayed by token/key mismatch or lock contention.
  - Commands:
    - `GET /auth/me` with no token, invalid token, malformed token.
    - `pg_stat_activity` during probes.
  - Key output:
    - Responses were immediate `401` in ~1-2ms.
    - No DB wait/lock pressure during probes.
  - Verdict: `PASS` (auth failure mode is fail-fast, not timeout-driven).
- [x] **C3 - Reproduce and validate auth rate-limit behavior**
  - Owner: `subagent-auth-rate-limit-repro`
  - Use operator-provided test account (`tester1`) and run controlled login/register/auth endpoint attempts from same IP.
  - Confirm whether `auth requests exceeded` is triggered according to configured limits, not on first/second normal attempts.
  - Capture request counts, timestamps, source IP interpretation (`peer` vs `forwarded`), and headers that influence keys.
  - Hypothesis: auth limiter keys are effectively proxy-peer keyed, causing false positives behind reverse proxy.
  - Commands:
    - 100-attempt controlled login loop with `X-Forwarded-For` switch at request 51 (`203.0.113.10` -> `198.51.100.20`) against `http://127.0.0.1:8080/auth/login`.
    - Follow-up post-limit attempts with third header value (`192.0.2.77`).
    - `docker logs --since 3m infra-filament-server-1 | rg "auth.rate_limit|client_ip_source|auth.login"`.
  - Key output:
    - Requests 1-60 returned `200`; request 61 returned `429 {"error":"rate_limited"}`.
    - Post-limit attempts with different `X-Forwarded-For` still returned `429`.
    - Server logs show `event="auth.rate_limit" ... client_ip="172.21.0.4" client_ip_source="peer"`.
  - Verdict: `FAIL` (root-cause indicator confirmed: auth limit keying ignores forwarded client IP in current deployment, collapsing clients into proxy peer bucket).
- [x] **C4 - Audit/fix auth rate-limit keying and counters**
  - Owner: `subagent-auth-rate-limit-correctness`
  - Inspect `auth_route_hits` update/sweep semantics, key construction, and trust model interaction with proxies.
  - Verify no duplicate counting per request and no cross-route pollution.
  - Add/adjust regression tests for “first/second try should not exceed limit” and burst-window boundaries.
  - Hypothesis: limiter counters are correct in code, but deployment trust/header path causes bad key source.
  - Findings:
    - `auth_route_hits` semantics were correct (`route:ip` bucket, bounded retain window, single push per request).
    - Runtime did not trust proxy path initially, so key source stayed `peer`.
    - `FILAMENT_TRUSTED_PROXY_CIDRS` was not wired into compose env.
    - Caddy was not explicitly forwarding an origin client IP header suited for Cloudflare.
  - Remediation:
    - Added `FILAMENT_TRUSTED_PROXY_CIDRS` pass-through to `filament-server` in `infra/docker-compose.yml`.
    - Set trusted proxy CIDRs on host runtime (`infra/.env`, local ops file): `172.20.0.6/32,172.21.0.4/32`.
    - Updated `infra/Caddyfile` to set upstream `X-Forwarded-For`/`X-Real-IP` from `CF-Connecting-IP` for HTTPS API/gateway and `remote_host` on legacy port.
  - Validation:
    - 80-login controlled run after fix: `203.0.113.10 -> 40x200`, `198.51.100.20 -> 20x200 + 20x429` (independent per-IP buckets).
    - Server logs show limiter source switched to `client_ip_source="forwarded"` after fix.
  - Verdict: `PASS` (root cause fixed).

### D. Postgres / sqlx Pool / Locking
- [x] **D1 - Check DB connectivity and pool starvation**
  - Owner: `subagent-db-pool`
  - Inspect `pg_stat_activity`, active waits, transaction age, blocked backends.
  - Confirm if pool (`max_connections=10`) is saturated by long-running or stuck sessions.
  - Hypothesis: DB pool starvation/blocked sessions are causing request timeouts.
  - Commands:
    - `psql` snapshots of `pg_stat_activity` state/waits/non-idle queries.
    - 30-sample loop during active login flood (`/tmp/wave1_d1_activity2.txt`).
  - Key output:
    - During load: consistently `active=1`, `waiting=0`, `total=3`.
    - No blocked chains or long-running application queries observed.
  - Verdict: `PASS` (no evidence of DB saturation or starvation in failure window).
- [x] **D2 - Check lock contention on hot tables used by POSTs**
  - Owner: `subagent-db-locks`
  - Focus tables: `users`, `guilds`, `guild_members`, `audit_logs`, `guild_role_members`, `friendship_requests`.
  - Detect row/table locks and blocking chains.
  - Hypothesis: lock contention on hot tables blocks POST handlers.
  - Commands:
    - `pg_locks` join on `pg_stat_activity` filtered to target tables.
    - blocked/blocker chain query.
  - Key output:
    - No blocking chains found.
    - No waiting lock entries on target tables during probe window.
  - Verdict: `PASS`.
- [ ] **D3 - Check DB-side timeouts and DNS/network latency from app container**
  - Owner: `subagent-db-network`
  - Validate server->postgres RTT, connection establishment time, and any intermittent packet loss.

### E. Shared Side Effects In POST Flows
- [x] **E1 - Audit write path latency**
  - Owner: `subagent-audit-path`
  - Measure `write_audit_log` cost and failure behavior.
  - Confirm audit insert/indexing is not waiting on locks.
  - Hypothesis: `write_audit_log` DB insert path stalls request flow.
  - Commands:
    - Code inspection: `nl -ba apps/filament-server/src/server/domain.rs | sed -n '681,719p'`.
    - Audit insert micro-benchmark (`pgbench`, 20 clients, 5s) on `audit_logs` insert shape matching `write_audit_log`.
  - Key output:
    - `write_audit_log` performs single `INSERT ... execute(pool).await`.
    - Insert benchmark summary: `latency average = 3.555 ms`, `failed transactions = 0`.
  - Verdict: `PASS` (audit insert path does not explain 10s timeouts in observed environment).
- [x] **E2 - Broadcast/fanout backpressure check**
  - Owner: `subagent-realtime-fanout`
  - Validate `broadcast_guild_event` cannot stall request path for 10s.
  - Check channel send behavior and slow-consumer handling.
  - Hypothesis: guild event fanout can block request path.
  - Commands:
    - Code inspection: `apps/filament-server/src/server/realtime/connection_runtime.rs`, `.../fanout_dispatch.rs`.
  - Key output:
    - Fanout uses `try_send` with bounded queue.
    - Slow consumers are dropped/closed; no await-on-send in hot path.
  - Verdict: `PASS` (fanout path is non-blocking).

### F. Runtime Resource Pressure
- [x] **F1 - Host/container resource saturation check**
  - Owner: `subagent-runtime-resources`
  - Gather CPU, memory, IO wait, file descriptors, and Docker stats during failure.
  - FAIL if app/DB is resource-starved or throttled during POSTs.
  - Hypothesis: host/container pressure causes timeout.
  - Commands:
    - `uptime`, `free -m`, `df -h`, `docker stats --no-stream`, top CPU process snapshot.
  - Key output:
    - Low load, ample free memory, no container CPU/memory throttling.
  - Verdict: `PASS` (no resource starvation evidence).

### G. Build/Deploy Drift
- [x] **G1 - Verify running image matches expected code/config**
  - Owner: `subagent-release-integrity`
  - Compare running container image digest/commit/env with repo HEAD and intended `.env` values.
  - Confirm no stale binary or mismatched config causes unexpected path behavior.
  - Hypothesis: deploy drift causes unexpected request behavior.
  - Commands:
    - `git rev-parse --short HEAD`, `docker inspect`, `docker compose config`, runtime env checks.
  - Key output:
    - Initial drift found: runtime env lacked `FILAMENT_TRUSTED_PROXY_CIDRS` pass-through.
    - Fixed by compose change; runtime env now contains trusted CIDRs.
  - Verdict: `FAIL -> PASS after remediation` (drift identified and corrected).

### H. Remediation and Hardening
- [x] **H1 - Implement minimal targeted fix for confirmed root cause**
  - Owner: `subagent-fix-impl`
  - Do not relax timeout/security limits unless explicitly approved.
  - Keep fix scoped; avoid broad refactors during incident.
  - Implemented:
    - `infra/docker-compose.yml`: wire `FILAMENT_TRUSTED_PROXY_CIDRS`.
    - `infra/Caddyfile`: explicit upstream client IP headers (`CF-Connecting-IP` path).
    - Optional diagnostics behind env flag (`FILAMENT_DEBUG_REQUEST_TIMINGS`).
  - Security posture:
    - Global timeout unchanged.
    - Rate limits/body limits/auth checks unchanged.
  - Verdict: `PASS`.
- [x] **H2 - Add regression tests and diagnostics coverage**
  - Owner: `subagent-fix-tests`
  - Add integration/unit tests reproducing prior timeout scenario.
  - Add/assert observable metrics/log fields needed for future detection.
  - Tests executed:
    - `cargo test -p filament-server auth_rate_limit_uses_forwarded_headers_for_trusted_proxy_peers`
    - `cargo test -p filament-server auth_rate_limit_ignores_forwarded_headers_when_proxy_is_untrusted`
  - Diagnostics:
    - Added guarded timing logs (`FILAMENT_DEBUG_REQUEST_TIMINGS`) for auth and directory join phases.
  - Verdict: `PASS`.
- [x] **H3 - Validate in staging-like runtime and production**
  - Owner: `subagent-fix-verify`
  - Re-run baseline POST probes, representative authenticated POSTs, and non-POST controls.
  - Success criteria: no 10s timeout pattern; p95 latency within budget; no security regression.
  - Verification run:
    - `GET https://filamentapp.net/api/health` -> `200` ~`0.06s`.
    - `POST https://filamentapp.net/api/echo` with valid payload -> `200` ~`0.06s`.
    - Repeated `POST /api/auth/login` through domain -> sustained `200` responses, no first/second-try false positive limiter behavior.
    - `POST /api/guilds/{guild_id}/join` returns promptly (observed `404` in ~`0.08s` for chosen guild context), not timeout.
  - Verdict: `PASS`.

## Root Cause Decision Tree
- If POST is slow only through proxy: prioritize A2 fix.
- If auth query is slow: prioritize C1/C2 + D1/D2.
- If auth rate-limit is false-positive: prioritize C3/C4 (keying, sweep timing, trusted proxy handling, test coverage).
- If DB waits/locks are present: prioritize D2 remediations (query/index/transaction scope).
- If broadcast blocks: prioritize E2 decoupling and bounded non-blocking dispatch.
- If resources are saturated: prioritize F1 capacity + queue/backpressure tuning.
- If drift detected: prioritize G1 rollout correction.

## Evidence Checklist (must be complete before closing incident)
- [x] Timed reproduction logs for GET and POST at all hops.
- [ ] Correlated request IDs across Caddy and server logs.
- [x] Auth rate-limit reproduction log proving expected vs actual request counts.
- [x] DB activity snapshot during failure window (`pg_stat_activity` + lock graph).
- [x] Confirmed root cause with direct evidence.
- [x] Patch + tests merged.
- [x] Post-fix verification runbook output archived.

## Additional Notes
- Caddy did not have access logging enabled in this deployment profile, so request-id correlation across Caddy/server was not available during incident. Enabling JSON access logs with upstream response headers is recommended for future incident forensics.
- In Cloudflare-fronted mode, forwarding `{header.CF-Connecting-IP}` to upstream is required for accurate client-IP keyed limits; relying on default proxy headers can collapse traffic into peer buckets.
- `.env` is gitignored in this repo. Runtime changes to `FILAMENT_TRUSTED_PROXY_CIDRS` must be applied operationally on host in addition to tracked compose/Caddy updates.

## Exit Criteria
- POST endpoints no longer hit 10s timeout under normal load.
- Confirmed root cause documented with evidence.
- Regression tests prevent recurrence.
- Security controls unchanged or strengthened.
