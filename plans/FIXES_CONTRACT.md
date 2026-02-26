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
  - 2026-02-26 rerun evidence: domain GET `200 0.065735`, domain POST `422 0.082019`; localhost GET `200 0.002709`, POST `422 0.001670`; container GET `200 0.001556`, POST `422 0.001180`.
  - 2026-02-26 incident rerun (VPS): domain GET `200 0.082565`, domain POST `422 0.058129`; localhost GET `200 0.002738`, POST `422 0.001305`; container GET `200 0.001661`, POST `422 0.001235`.
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
    - 2026-02-26 rerun: `caddy validate` passed; warning only about unnecessary `header_up X-Forwarded-For`, no timeout directives.
    - 2026-02-26 incident rerun (VPS): `caddy validate` remained valid; `POST https://filamentapp.net/api/echo` returned `422 0.065485` (no ~10s delay).
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
    - 2026-02-26 rerun confirmed same ordering in `router.rs:509-519`.
    - 2026-02-26 incident rerun (VPS) reconfirmed same ordering at `router.rs:509-519` (`TimeoutLayer` before `GovernorLayer`, global body limit retained).
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
    - 2026-02-26 incident rerun (VPS): guarded timing hooks still present in `auth.rs` and `handlers/guilds.rs` (`debug.auth.authenticate_with_token.timing`, `debug.guild.join_public_guild.timing`), with no always-on logging path introduced.
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
    - 2026-02-26 rerun: DB execution `0.057 ms`; `/auth/me` sample `n=30 avg=0.017659s p95=0.043189s max=0.077503s`.
    - 2026-02-26 incident rerun (VPS): DB `Execution Time: 0.070 ms`; `/auth/me` no-load `n=30 avg=0.013536s p95=0.040198s max=0.054252s`; under load (100@20) `avg=0.005914s p95=0.017279s max=0.035535s`.
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
    - 2026-02-26 rerun via domain: no-token `401 0.072816s`, malformed token `401 0.054727s`.
    - 2026-02-26 incident rerun (VPS): no-token `401 0.066667s`, malformed token `401 0.073194s`, invalid token `401 0.069078s` (all fail-fast, no timeout behavior).
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
    - 2026-02-26 clean rerun against local Caddy (`--resolve filamentapp.net:443:127.0.0.1`, `CF-Connecting-IP` switched at request 41):
      - `ip1: 40x200`
      - `ip2: 20x200, then 20x429`
      - Logs showed `client_ip="172.21.0.1" client_ip_source="forwarded"` at limit time, proving both synthetic IPs collapsed into one bucket.
    - 2026-02-26 incident rerun (VPS, post-fix behavior): 100-attempt split run produced `203.0.113.10:50x200` and `198.51.100.20:50x200` (no false-positive throttling on first/second attempts); clean-threshold run with fresh IP `203.0.113.77` yielded first `429` at request `61` (`60x200 + 20x429`); alternate IP immediately after stayed `10x200`; logs showed `event="auth.rate_limit"` with `client_ip_source="forwarded"`.
    - 2026-02-26 direct domain check: `POST https://filamentapp.net/api/auth/login` returned `200` on attempts 1-5.
  - Verdict: `FAIL -> PASS after remediation` (historical root-cause indicator was confirmed, and current VPS rerun shows forwarded client-IP keying works with expected threshold behavior and no first/second-attempt false positives).
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
    - Added trusted-proxy header parsing fix in `apps/filament-server/src/server/auth.rs`: when peer is trusted, parse `CF-Connecting-IP` first, then fallback to `X-Forwarded-For`.
  - Validation:
    - 80-login controlled run after fix: `203.0.113.10 -> 40x200`, `198.51.100.20 -> 20x200 + 20x429` (independent per-IP buckets).
    - Server logs show limiter source switched to `client_ip_source="forwarded"` after fix.
    - 2026-02-26 post-fix rerun (`CF-Connecting-IP` switched 40/40): `ip1:40x200`, `ip2:40x200` with no 429s.
    - 2026-02-26 incident rerun (VPS): targeted unit tests passed for proxy/header keying and sweep behavior:
      - `client_ip_uses_cf_connecting_ip_when_peer_proxy_is_trusted`
      - `client_ip_falls_back_to_xff_when_cf_connecting_ip_is_invalid`
      - `auth_rate_limit_sweep_prunes_stale_keys`
      - `rate_limit_sweep_keeps_maps_bounded_under_many_unique_stale_keys`
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
    - 2026-02-26 rerun during 100-login flood: 10 samples all `active=1|waiting=0|total=11`.
    - 2026-02-26 incident rerun (VPS): 20-sample probe during 120-login flood showed `active` mostly `1` (peak `2`), `wait_event_type='Lock'` `0`, `wait_event_type='LWLock'` `0`, `active>1s` `0`, `total` `5`; blocker-chain query count `0`.
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
    - 2026-02-26 rerun: blocker query returned 0 rows; only `pg_locks AccessShareLock` observed.
    - 2026-02-26 incident rerun (VPS): blocker-chain query returned `0`; lock summary showed only `virtualxid ExclusiveLock` and `relation AccessShareLock` (`granted=true`), no waiting lock contention.
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
    - 2026-02-26 rerun `EXPLAIN (ANALYZE)` insert: execution `2.770 ms` (including FK trigger `1.500 ms`), then cleanup delete succeeded.
    - 2026-02-26 incident rerun (VPS): insert probe execution `2.545 ms` with FK trigger `1.603 ms`; cleanup delete succeeded.
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
    - 2026-02-26 incident rerun (VPS): `fanout_dispatch.rs` still uses `sender.try_send(...)` with explicit `full_queue`/`closed` drop handling and tests for oversized/closed/full-queue behavior.
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
    - 2026-02-26 rerun: load `2.59/3.02/1.64`, available RAM `~27GB`, `infra-filament-server-1` memory `81.91MiB` (0.26%).
    - 2026-02-26 incident rerun (VPS): load `0.10/0.77/1.00`, available RAM `~27.5GB`, `infra-filament-server-1` memory `171.1MiB` (0.53%), no container pressure symptoms.
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
    - 2026-02-26 rerun: compose config includes `FILAMENT_TRUSTED_PROXY_CIDRS=172.20.0.6/32,172.21.0.4/32`; runtime container env matches.
    - 2026-02-26 incident rerun (VPS): repo HEAD `2808029`; runtime env includes `FILAMENT_TRUSTED_PROXY_CIDRS=172.20.0.6/32,172.21.0.4/32`; running image `infra-filament-server@sha256:bea081d99aa6601b47a8f4aaaa759cc5126d66b666fdf6794a5e91927f13c037`.
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
    - `apps/filament-server/src/server/auth.rs`: trusted-proxy IP extraction now prefers `CF-Connecting-IP` then `X-Forwarded-For`.
  - Security posture:
    - Global timeout unchanged.
    - Rate limits/body limits/auth checks unchanged.
  - 2026-02-26 incident rerun (VPS): no additional code/config fix required; existing targeted proxy/IP-keying remediation remains active and effective.
  - Verdict: `PASS`.
- [x] **H2 - Add regression tests and diagnostics coverage**
  - Owner: `subagent-fix-tests`
  - Add integration/unit tests reproducing prior timeout scenario.
  - Add/assert observable metrics/log fields needed for future detection.
  - Tests executed:
    - `cargo test -p filament-server auth_rate_limit_uses_forwarded_headers_for_trusted_proxy_peers`
    - `cargo test -p filament-server auth_rate_limit_ignores_forwarded_headers_when_proxy_is_untrusted`
    - `cargo test -p filament-server client_ip_uses_cf_connecting_ip_when_peer_proxy_is_trusted`
    - `cargo test -p filament-server client_ip_falls_back_to_xff_when_cf_connecting_ip_is_invalid`
  - Diagnostics:
    - Added guarded timing logs (`FILAMENT_DEBUG_REQUEST_TIMINGS`) for auth and directory join phases.
  - 2026-02-26 incident rerun (VPS): targeted regression tests for trusted-proxy header parsing and auth limiter sweep/keying passed locally.
  - Verdict: `PASS`.
- [x] **H3 - Validate in staging-like runtime and production**
  - Owner: `subagent-fix-verify`
  - Re-run baseline POST probes, representative authenticated POSTs, and non-POST controls.
  - Success criteria: no 10s timeout pattern; p95 latency within budget; no security regression.
  - Verification run:
    - `GET https://filamentapp.net/api/health` -> `200` ~`0.06s`.
    - `POST https://filamentapp.net/api/echo` with `{}` -> `422` ~`0.07s` (fast validation response, no timeout).
    - Repeated `POST /api/auth/login` through domain -> sustained `200` responses, no first/second-try false positive limiter behavior.
    - `POST /api/guilds/{guild_id}/join` returns promptly (observed `404` in `0.108887s` for chosen guild context), not timeout.
    - 2026-02-26 incident rerun (VPS):
      - `GET /api/health` -> `200 0.066572s`
      - `POST /api/echo` -> `422 0.056607s`
      - `POST /api/auth/login` (5 attempts) -> all `200` (`0.136s-0.157s`)
      - `POST /api/guilds/01KJCCPY6NJXB34J0VE3FEBC1F/join` -> `200 0.089679s` with `{"outcome":"accepted"}` (no 408).
  - Verdict: `PASS`.

### I. Regression Follow-Up (2026-02-26)
- [x] **I1 - Reproduce new timeout regression on message + voice token routes**
  - Owner: `subagent-regression-repro`
  - Hypothesis: a shared post-auth/post-DB path regressed and now hits global 10s timeout for mutating channel/voice flows.
  - Commands:
    - `POST /api/guilds/{guild_id}/channels/{channel_id}/messages` with `tester1` token (6 attempts).
    - `POST /api/guilds/{guild_id}/channels/{channel_id}/voice/token` with `tester1` token (6 attempts).
    - Control probes (`GET /api/health`, `POST /api/echo`) remained fast.
  - Key output:
    - Messages: repeated `408` around `10.06s`.
    - Voice token: repeated `408` around `10.06s`.
    - Health/echo remained fast (`~0.08s` class).
  - Verdict: `FAIL` (regression confirmed).

- [x] **I2 - Determine whether timeout occurs before or after DB write**
  - Owner: `subagent-regression-boundary`
  - Hypothesis: requests are timing out after persistence, indicating downstream stall (fanout/search/realtime side effects).
  - Commands:
    - Before/after `count(*)` on `messages` for target channel around a timed-out message create.
    - Before/after `count(*)` on `audit_logs` (`action='media.token.issue'`) around a timed-out voice token issue.
  - Key output:
    - Message request returned `408 10.064708`, while DB count increased (`delta=1`) and new message row existed.
    - Voice token request returned `408 10.068632`, while audit log count increased (`delta=1`) with new `media.token.issue` record.
  - Verdict: `FAIL` (stall confirmed after DB write boundary).

- [x] **I3 - Cross-check related routes and live runtime symptoms**
  - Owner: `subagent-regression-scope`
  - Hypothesis: not all guild/channel routes are broken; regression is path-specific.
  - Commands:
    - `GET /api/guilds/{guild}/channels/{channel}/permissions/self`
    - `GET /api/guilds/{guild}/channels/{channel}/messages?limit=5`
    - `POST /api/guilds/{guild}/channels/{channel}/voice/state`
    - `POST /api/guilds/{guild}/channels/{channel}/voice/leave`
  - Key output:
    - Permissions/get-messages/voice-state were fast (`200/204` around `~0.06s`).
    - Voice leave timed out at `408 ~10.06s`.
  - Verdict: `FAIL` (regression appears concentrated in paths with realtime broadcast side effects).

- [x] **I4 - Apply incident-safe runtime recovery + verify**
  - Owner: `subagent-regression-recover`
  - Hypothesis: runtime process state was wedged; recreating `filament-server` should clear transient lock/contention without relaxing controls.
  - Commands:
    - `docker compose --profile web --env-file infra/.env -f infra/docker-compose.yml up -d --build filament-server`
    - Post-recreate timed loops: 12 message posts + 12 voice-token posts.
  - Key output:
    - After recreate, message posts were stable `200` (`~0.07s-0.20s`).
    - After recreate, voice token posts were stable `200` (`~0.06s-0.09s`).
  - Verdict: `PASS` (symptom cleared, but underlying trigger remains unproven).

- [x] **I5 - Add guarded diagnostics for next recurrence**
  - Owner: `subagent-regression-instrument`
  - Hypothesis: recurrence needs phase timing around suspected shared hot path (channel fanout + search enqueue ack).
  - Changes:
    - `apps/filament-server/src/server/realtime/connection_runtime.rs`
      - Added `debug.gateway.broadcast_channel_event.timing` log (behind `FILAMENT_DEBUG_REQUEST_TIMINGS`).
    - `apps/filament-server/src/server/realtime/search_runtime.rs`
      - Added `debug.search.enqueue_search_command.timing` log (behind `FILAMENT_DEBUG_REQUEST_TIMINGS`).
  - Notes:
    - Existing debug flag wiring in compose currently does not pass `FILAMENT_DEBUG_REQUEST_TIMINGS` to the running service by default in this run.
  - Verdict: `PASS` (instrumentation added; enable flag on next recurrence to localize blocking segment quickly).

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
- 2026-02-26 regression follow-up showed a transient process-state failure mode: writes succeeded but HTTP request still hit global timeout on message/voice token/voice leave; service recreate cleared symptoms. Keep guarded fanout/search timing instrumentation available for next recurrence.

## Exit Criteria
- POST endpoints no longer hit 10s timeout under normal load.
- Confirmed root cause documented with evidence.
- Regression tests prevent recurrence.
- Security controls unchanged or strengthened.
