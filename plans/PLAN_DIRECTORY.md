# PLAN_DIRECTORY.md

## Objective
Implement a secure, auditable "public workspace directory -> join workspace" flow with owner/moderator controls, including privacy-preserving guild-scoped IP abuse controls and moderation UX in the web client.

## Scope
- Public workspace discovery (`GET /guilds/public`) remains the entry point.
- Authenticated users can self-join eligible public workspaces from directory UX.
- Workspace owners/moderators can audit joins, review join activity, and moderate joiners.
- Guild-scoped IP bans are enforceable and auditable without exposing raw IPs to workspace owners/moderators.

## Non-Goals
- Federation.
- E2EE changes.
- New trust assumptions for server-provided UI data.
- Relaxing any existing request-size, timeout, or rate-limit controls.

## Code-State Audit (2026-02-12)
- Public discovery exists:
  - Server: `GET /guilds/public` in `apps/filament-server/src/server/handlers/guilds.rs`.
  - Web UI: `PublicDirectoryPanel` + `public-directory-controller`.
- Self-join from directory does not exist yet.
- Membership add endpoint exists (`POST /guilds/{guild_id}/members/{user_id}`) but requires `manage_roles`, so users cannot join themselves.
- User-level guild bans exist (`guild_bans`), but guild-scoped IP bans do not.
- Audit write path exists (`write_audit_log`), but there is no owner-facing audit read API/UI.
- Client IP extraction currently trusts `x-forwarded-for` directly (`extract_client_ip`), which is insufficient for security-critical IP moderation policy.
- There is no user->known-IP observation model yet for privacy-preserving moderation actions.

## Locked Security Constraints
- No `unsafe`.
- No HTML rendering path in chat/UI.
- Strict DTO parsing at boundaries + domain `TryFrom`/invariant constructors.
- Hard caps:
  - request/response body sizes
  - query lengths and limits
  - per-route rate limits
  - moderation queue pagination/fetch limits
- All phases ship with tests and must pass:
  - `cargo fmt`
  - `cargo clippy`
  - `cargo test`
  - `pnpm --prefix apps/filament-client-web test`
  - security CI gates (`cargo audit`, `cargo deny`, dependency review, SBOM)

## Product/Policy Decisions (for this plan)
- Public workspace join mode (v1): immediate self-join for eligible public workspaces.
- Join requests (approval queue) are a later phase, not required for initial directory join UX.
- Permission hierarchy target model:
  - server owner: trusted global authority across all workspaces
  - workspace owner: untrusted globally, full authority only inside owned workspace realm
  - workspace-defined roles: configurable permission sets managed by workspace owners
- Workspace owners/moderators must never receive raw IP or CIDR values in API/UI responses.
- Workspace owners/moderators can trigger "ban all known IPs for user" actions; IP resolution happens server-side.
- Raw IP visibility is reserved for server-administrator-only operational tooling and is out of workspace owner scope.
- Owner/moderator audit visibility includes who joined, when, actor, and moderation actions, but not raw IP values.

## Status Legend
- `NOT STARTED`
- `IN PROGRESS`
- `DONE`
- `BLOCKED`

---

## Phase 0 - Threat Model + Contract Design
### Goal
Lock API contracts, trust boundaries, and invariants before implementation.

### Completion Status
`DONE`

### Tasks
- [x] Define directory join threat model update in `docs/THREAT_MODEL.md`:
  - join spam/DoS
  - workspace enumeration behavior
  - spoofed forwarded-IP headers
  - abusive rejoin loops after moderation
- [x] Define new domain invariants/newtypes (server + web):
  - `GuildIpBanId`
  - `IpNetwork` (CIDR or exact IP canonicalized)
  - `AuditCursor` (bounded pagination token)
- [x] Specify response/error semantics for:
  - private/nonexistent guild join attempts
  - banned-by-user vs banned-by-ip outcomes
  - unauthorized audit access
- [x] Document per-route limits for new endpoints (join, audit list, ip-ban CRUD).

### Tests
- [x] Add/extend unit tests for invariant constructors and DTO validation.

### Refactor Notes
- Added `apps/filament-server/src/server/directory_contract.rs` with Phase 0 contract newtypes and invariant constructors:
  - `GuildIpBanId`
  - `IpNetwork` (host/CIDR canonicalization for IPv4/IPv6)
  - `AuditCursor`
- Added server-side typed DTO contract parsers for upcoming directory moderation endpoints:
  - audit list query
  - guild IP-ban list query
  - guild IP-ban by-user request
- Added matching web domain invariants and DTO parsers in `apps/filament-client-web/src/domain/chat.ts`:
  - newtypes: `GuildIpBanId`, `IpNetwork`, `AuditCursor`
  - typed parsers for directory join result, guild audit pages, and guild IP-ban pages/results
- Updated docs:
  - `docs/THREAT_MODEL.md` (directory join + IP moderation abuse cases/mitigations)
  - `docs/API.md` (locked error semantics + per-route limits for join/audit/ip-ban endpoints)
- Added/extended tests:
  - `apps/filament-server/src/server/directory_contract.rs` unit tests
  - `apps/filament-client-web/tests/domain-chat.test.ts` invariant/DTO tests
  - `apps/filament-client-web/tests/api-boundary.test.ts` deterministic directory join error mapping

### Security Outlook
- Prevents policy drift and ad-hoc JSON/string logic before coding.

### Exit Criteria
- Endpoint and type contracts are documented and accepted in this plan/docs.

---

## Phase 1 - Schema + Index + Runtime Config Foundation
### Goal
Add durable tables/config for IP bans and scalable join/audit queries.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Extend DB schema in `apps/filament-server/src/server/db.rs`:
  - `user_ip_observations` table:
    - `observation_id`, `user_id`, `ip_cidr`, `first_seen_at_unix`, `last_seen_at_unix`
    - canonicalized host CIDR (`/32` IPv4, `/128` IPv6)
  - `guild_ip_bans` table:
    - `ban_id`, `guild_id`, `ip_cidr`, `source_user_id NULL`, `reason`, `created_by_user_id`, `created_at_unix`, `expires_at_unix NULL`
  - indexes:
    - `idx_user_ip_observations_user_last_seen`
    - `idx_guild_ip_bans_guild_created`
    - `idx_audit_logs_guild_created`
    - `idx_audit_logs_guild_action_created`
- [ ] Add runtime config defaults + env overrides for:
  - `FILAMENT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_IP`
  - `FILAMENT_DIRECTORY_JOIN_REQUESTS_PER_MINUTE_PER_USER`
  - `FILAMENT_AUDIT_LIST_LIMIT_MAX`
  - `FILAMENT_GUILD_IP_BAN_MAX_ENTRIES`
- [ ] Ensure config validation rejects zero/invalid values.

### Tests
- [ ] Schema initialization tests for new tables/indexes and backward-compatible startup.
- [ ] Config parsing/validation tests for new env vars.

### Security Outlook
- Provides bounded storage and query performance for moderation surfaces.

### Exit Criteria
- Schema initializes cleanly in fresh + existing DBs and config is validated at startup.

---

## Phase 2 - Trusted Client IP Pipeline
### Goal
Harden client-IP derivation so IP bans are enforceable and non-spoofable.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Replace direct `x-forwarded-for` trust with a trusted proxy model:
  - default: use socket peer address
  - optional forwarded-header trust only when explicitly configured
- [ ] Introduce canonical IP parser (IPv4/IPv6) and reject malformed/suspicious values.
- [ ] Update rate-limit keying and moderation handlers to use normalized client IP output.
- [ ] Add structured tracing fields for `client_ip_source` (`peer|forwarded`) without logging unsafe raw header blobs.

### Tests
- [ ] Unit tests for IP extraction precedence and malformed header handling.
- [ ] Integration tests for trusted/untrusted proxy modes.

### Security Outlook
- Eliminates spoofed-header bypass for IP-based abuse controls.

### Exit Criteria
- Every path that enforces IP policy uses the trusted extraction pipeline.

---

## Phase 3 - Public Workspace Self-Join API + Audit Writes
### Goal
Enable authenticated self-join for public workspaces with strict policy checks and audit events.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add endpoint:
  - `POST /guilds/{guild_id}/join` (auth required)
- [ ] Join behavior:
  - allow only if guild visibility is `public`
  - deny if requester is user-banned or IP-banned
  - idempotent for already-member state
  - no leakage beyond policy-approved response shapes
- [ ] Upsert user IP observation on authenticated join attempt (success or policy rejection) with bounded write rate.
- [ ] Write audit events for all join outcomes:
  - `directory.join.accepted`
  - `directory.join.rejected.user_ban`
  - `directory.join.rejected.ip_ban`
  - `directory.join.rejected.visibility`
- [ ] Add explicit route-local rate limiting for join endpoint (user + IP).
- [ ] Ensure in-memory fallback mirrors DB behavior.

### Tests
- [ ] Integration tests:
  - public join success
  - private workspace join rejection
  - idempotent repeat join
  - user-banned rejection
  - rate-limit rejection
- [ ] Unit tests for join DTO parsing and state transitions.

### Security Outlook
- Adds join capability without broadening visibility or bypassing moderation controls.

### Exit Criteria
- Authenticated users can join eligible public workspaces and all outcomes are auditable.

---

## Phase 4 - Guild Audit Read API (Owner/Moderator)
### Goal
Expose auditable join/moderation history to authorized workspace operators.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add endpoint:
  - `GET /guilds/{guild_id}/audit?cursor=<...>&limit=<n>&action_prefix=<...>`
- [ ] Access policy:
  - owner/moderator only (or explicit permission if introduced)
  - non-members receive policy-consistent denial
- [ ] Pagination + caps:
  - cursor-based pagination
  - strict max limit (from config)
  - bounded action filter length/charset
- [ ] Return typed audit response DTOs (no raw arbitrary JSON passthrough).
- [ ] Redact IP data in owner/moderator audit responses:
  - no raw `ip`, `cidr`, or equivalent fields
  - optional non-sensitive derived metadata only (example: `ip_ban_match=true`)

### Tests
- [ ] Integration tests for authz, pagination, action filtering, and bounded limits.
- [ ] Domain parsing tests in web client for new audit DTOs.

### Security Outlook
- Improves accountability while keeping query cost and data exposure bounded.

### Exit Criteria
- Authorized operators can review join and moderation events safely and consistently.

---

## Phase 5 - Guild IP Ban Controls + Enforcement
### Goal
Allow workspace operators to trigger user-derived guild IP bans without IP visibility, and enforce them consistently.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add endpoints:
  - `GET /guilds/{guild_id}/ip-bans?cursor=<...>&limit=<n>` (redacted records only)
  - `POST /guilds/{guild_id}/ip-bans/by-user`
  - `DELETE /guilds/{guild_id}/ip-bans/{ban_id}`
- [ ] Request validation:
  - target user ULID validation
  - reason length cap
  - optional expiry cap
- [ ] Owner/moderator action semantics:
  - resolve all known observed IPs for target user server-side
  - create guild IP-ban entries for resolved IP set
  - response returns counts and ban IDs, never raw IP values
- [ ] Authorization split:
  - workspace owner/moderator: user-derived IP ban actions only
  - server administrator tooling for raw IP inspection (out-of-scope in this plan; no owner exposure)
- [ ] Enforcement points:
  - directory join endpoint
  - guild channel/message/search/media endpoints (guild-scoped access denial)
  - gateway subscription/auth checks for affected guild channels
- [ ] Audit events:
  - `moderation.ip_ban.add`
  - `moderation.ip_ban.remove`
  - `moderation.ip_ban.hit`

### Tests
- [ ] Unit tests for canonical host-IP observation + matching (IPv4 + IPv6).
- [ ] Integration tests for:
  - add/list/remove user-derived IP bans (redacted list payloads)
  - join rejection by IP
  - guild endpoint rejection when IP ban is active
  - expiry behavior

### Security Outlook
- Provides strong guild-level abuse controls with deterministic enforcement.

### Exit Criteria
- IP bans are enforceable across relevant guild surfaces and fully auditable.

---

## Phase 6 - Web Directory Join UX
### Goal
Upgrade directory panel from read-only listing to secure, actionable join UX.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Extend client domain/API types:
  - join result DTOs + audit DTOs + ip-ban DTOs with strict validation
- [ ] Update `public-directory-controller`:
  - add join action per workspace
  - per-row pending/loading/error state
  - stale-response cancellation semantics
- [ ] Update `PublicDirectoryPanel`:
  - "Join" action button
  - status chips (`joined`, `banned`, `join_failed`, `joining`)
  - deterministic error messaging from API code mapping
- [ ] On successful join:
  - trigger workspace/channel refresh
  - preserve existing selected workspace/channel safely

### Tests
- [ ] Add/expand tests:
  - `app-shell-public-directory-controller.test.ts`
  - `app-shell-public-discovery.test.tsx`
  - `domain-chat.test.ts` + `api-boundary.test.ts` for new DTOs/error mapping

### Security Outlook
- Keeps hostile-server parsing posture and avoids introducing unsafe rendering paths.

### Exit Criteria
- Users can join public workspaces from directory UX with robust, tested failure handling.

---

## Phase 7 - Hierarchical Permissions Model + Enforcement
### Goal
Implement Discord-like permission hierarchy and runtime permission resolution with clear separation between server owner authority and workspace-scoped authority.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add role model storage and invariants:
  - workspace role records (ID, name, position, managed/system flags)
  - mandatory workspace system roles: `@everyone`, `workspace_owner`
  - role-permission bindings
  - member-role assignments
  - required base role/default permissions for new members
- [ ] Introduce three-tier authority model:
  - filament server owner bypass for all workspace permission checks
  - workspace owners are full authority only in their own workspace realm
  - workspace owners cannot assign/escalate to filament server owner authority
- [ ] Implement permission resolution engine:
  - deterministic allow/deny precedence (locked below)
  - channel override interaction rules (locked below)
  - bounded role count and assignment count per member
- [ ] Add role-management API endpoints:
  - list/create/update/delete workspace roles
  - assign/unassign member roles
  - update role permission sets
  - reorder role hierarchy
- [ ] Migrate existing hardcoded role checks to resolved permission checks where appropriate, preserving least privilege and backward compatibility.
- [ ] Add audit events:
  - `role.create`
  - `role.update`
  - `role.delete`
  - `role.assign`
  - `role.unassign`
  - `role.permissions.update`
  - `role.reorder`

### Locked Resolution Spec (Phase 7)
- [ ] Lock canonical evaluation order for `resolve_effective_permissions(user_id, guild_id, channel_id?)`:
  - step 1: if user is filament server owner, grant all permission bits for the target scope
  - step 2: require active membership in guild (unless step 1 matched); non-members receive no permissions
  - step 3: seed guild permissions from `@everyone` role allow bitset
  - step 4: union allow bitsets from all assigned workspace roles
  - step 5: if member has `workspace_owner` system role, grant all guild-scoped bits (no role-position limits)
  - step 6: if `channel_id` is absent, return guild-scoped snapshot
  - step 7: if `channel_id` exists, apply channel overrides in strict order:
    - `@everyone` deny
    - `@everyone` allow
    - aggregate assigned-role denies
    - aggregate assigned-role allows
    - member-specific deny
    - member-specific allow
  - step 8: if member has `workspace_owner` role (or filament server owner), channel denies do not remove effective permissions
- [ ] Lock conflict policy:
  - for non-owner paths, deny overrides allow at the same precedence layer
  - unknown permission bits in persisted data are masked out and audited
  - unknown permission strings in API payloads are rejected as `invalid_request`
- [ ] Lock hierarchy mutation rules:
  - role `position` defines manageability boundaries
  - actor may only edit/assign roles strictly below actor's highest manageable role
  - workspace owner may manage all non-system roles in their workspace
  - `workspace_owner` and `@everyone` are system roles and cannot be deleted
  - at least one workspace owner must always exist; transfer flow must be atomic
- [ ] Lock cross-scope escalation rules:
  - only filament server owner can grant/revoke `workspace_owner`
  - workspace owners cannot create roles that imply filament server owner powers
  - permission checks for global/admin endpoints must ignore workspace role grants
- [ ] Add a migration adapter:
  - map current `owner|moderator|member` model into system/default roles
  - preserve current behavior during rollout with dual-read parity checks
  - remove legacy direct role branching only after parity tests pass

### Permission Bit Matrix (Phase 7)
Use this matrix as the canonical target for permission surface, default grants, and endpoint authorization gates.

| Permission Bit | Scope | Purpose | Default `@everyone` | Default `moderator` | Default `workspace_owner` | Filament Server Owner | Primary Endpoint/Feature Gates |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `create_message` | guild/channel | Create chat messages and basic participation | allow | allow | allow | allow (bypass) | `POST /messages`, message composer visibility |
| `delete_message` | guild/channel | Delete/edit others' messages | deny | allow | allow | allow (bypass) | `PATCH/DELETE /messages/{message_id}` (non-author paths) |
| `manage_channel_overrides` | guild/channel | Configure channel role overrides | deny | allow | allow | allow (bypass) | `POST /channels/{channel_id}/overrides/{role}` |
| `ban_member` | guild | Kick/ban members (user-level moderation) | deny | allow | allow | allow (bypass) | `POST /members/{user_id}/kick`, `POST /members/{user_id}/ban` |
| `manage_member_roles` | guild | Assign/unassign non-owner workspace roles | deny | allow | allow | allow (bypass) | member role assignment endpoints in Phase 7 |
| `manage_workspace_roles` | guild | Create/update/delete/reorder workspace roles | deny | deny | allow | allow (bypass) | role CRUD/reorder endpoints in Phase 7 |
| `view_audit_log` | guild | View workspace audit trail (redacted) | deny | allow | allow | allow (bypass) | `GET /guilds/{guild_id}/audit` |
| `manage_ip_bans` | guild | Trigger/remove user-derived IP bans | deny | allow | allow | allow (bypass) | `/guilds/{guild_id}/ip-bans/*` endpoints |
| `publish_video` | channel/voice | Publish camera tracks | deny | allow | allow | allow (bypass) | voice token `can_publish`/`publish_sources` camera |
| `publish_screen_share` | channel/voice | Publish screen-share tracks | deny | allow | allow | allow (bypass) | voice token `can_publish`/`publish_sources` screen_share |
| `subscribe_streams` | channel/voice | Subscribe to remote media streams | allow | allow | allow | allow (bypass) | voice token `can_subscribe` and call UX |

Matrix guardrails:
- New bits introduced in Phase 7: `manage_member_roles`, `manage_workspace_roles`, `view_audit_log`, `manage_ip_bans`.
- `workspace_owner` is modeled as a system role grant in workspace scope; filament server owner remains global bypass.
- Deny-by-default applies to all permission bits not explicitly granted by resolved role set.
- Any endpoint not mapped above must declare its required bit(s) before Phase 7 exit.

### Tests
- [ ] Unit tests:
  - permission resolution precedence and invariants
  - channel override order with explicit golden fixtures
  - anti-escalation rules (workspace owner cannot cross scope)
  - role hierarchy mutation safety (no orphaned ownership)
  - unknown permission bit/input rejection and masking behavior
- [ ] Integration tests:
  - role CRUD and assignment flows
  - endpoint authorization under resolved permissions
  - regression coverage for existing moderation/channel/search/media gates
  - legacy-role parity tests against pre-Phase-7 behavior for existing fixtures

### Security Outlook
- Reduces privilege confusion and blocks cross-scope escalation with explicit hierarchical policy.

### Exit Criteria
- Permission checks are hierarchy-based, tested, and enforce server-owner/global vs workspace-owner/local boundaries.

---

## Phase 8 - Workspace Role Management UX
### Goal
Ship owner-facing UX to configure workspace roles/permissions and member role assignments safely.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add role-management panel UX:
  - role list with hierarchy visualization
  - create/edit/delete role actions
  - permission matrix editor with explicit toggles and guardrails
  - member role assignment controls
- [ ] UX guardrails:
  - prevent dangerous self-lockout actions
  - explicit confirmation for destructive role changes
  - clear capability previews before apply
- [ ] Enforce strict permission gating in UI:
  - only authorized workspace owners/managers can see/edit role controls
  - no server-owner-only controls in workspace owner panels
- [ ] Integrate permission UX with existing moderation and channel override panels.

### Tests
- [ ] Add/expand:
  - role panel rendering and authorization gating tests
  - role permission matrix interaction tests
  - controller tests for assignment/update/reorder actions
  - integration-like tests for end-to-end role-change behavior impact

### Security Outlook
- Makes permission changes auditable, explicit, and less error-prone for workspace operators.

### Exit Criteria
- Workspace owners can safely manage role hierarchy and permissions with full regression coverage.

---

## Phase 9 - Owner Moderation UX for Joins + IP Bans
### Goal
Deliver owner/moderator UI for auditing joins and managing IP bans.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Add new operator panel(s):
  - join/audit activity feed
  - IP ban management (ban user's known IPs, list/remove redacted bans)
- [ ] Permission gating:
  - visible only for authorized roles
  - controls disabled on missing active workspace/session
- [ ] Safe rendering and bounded UI behavior:
  - cap page size
  - explicit refresh controls
  - no unbounded polling
- [ ] Integrate with existing moderation panel flow and status/error surfaces.

### Tests
- [ ] Add/expand:
  - panel rendering and permission gating tests
  - moderation controller tests for ip-ban actions
  - operator permissions integration tests

### Security Outlook
- Exposes moderation power only to authorized operators with auditable action trails.

### Exit Criteria
- Workspace owners/moderators can audit joins and manage IP bans from web UI.

---

## Phase 10 - Docs, Ops, and Security Gate Closure
### Goal
Finalize docs/runbooks and verify full test + security gate coverage.

### Completion Status
`NOT STARTED`

### Tasks
- [ ] Update `docs/API.md` for new join/audit/ip-ban endpoints and limits.
- [ ] Update `docs/SECURITY.md` and `docs/THREAT_MODEL.md` for:
  - trusted proxy IP policy
  - guild IP moderation model
  - audit retention guidance
- [ ] Update deploy docs with new env vars and safe reverse-proxy guidance.
- [ ] Ensure CI includes any new checks required by dependency/config changes.

### Tests
- [ ] Full pass of server + web tests and security gates.

### Security Outlook
- Makes operational posture explicit and repeatable for self-hosters.

### Exit Criteria
- Documentation and CI accurately reflect shipped directory moderation capabilities.

---

## Deferred (Post-v1)
- Join approval queue for public workspaces:
  - `pending/approved/rejected` workflow
  - optional "require approval to join" guild policy
- Automated abuse heuristics (risk scoring, cooldown windows).
- Advanced audit exports for compliance tooling.
- Server-administrator operational console for raw-IP investigation (strictly separate from workspace owner/moderator APIs/UIs).

## Rollout Strategy
1. Ship backend contract + enforcement behind a feature flag.
2. Enable web join UX for controlled internal testing.
3. Ship hierarchical permissions engine and migrate permission checks.
4. Enable workspace role-management UX.
5. Enable owner moderation UI and IP bans.
6. Remove flag after telemetry and audit validation pass.

## PR Checklist Template (Directory Work)
- What changed (1-3 bullets)
- Threat model impact
- Limits added/updated (sizes/rates/queues)
- Tests added/updated
- Dependency/config changes and license/security justification
