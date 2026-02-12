# Filament Threat Model (Phase 0)

## Trust Boundaries
- Internet clients to Filament server.
- Filament clients consuming server-provided data (malicious server model).
- Filament server to Postgres.
- Filament server to LiveKit.

## Primary Adversaries
- Unauthenticated internet attacker.
- Authenticated abusive user.
- Malicious or compromised server sending hostile payloads to clients.
- Supply-chain compromise in dependencies.

## Key Abuse Cases
- Oversized request/event DoS.
- Brute force and credential stuffing against auth endpoints.
- Refresh token replay.
- Fanout amplification and slow-consumer exhaustion.
- Upload abuse (zip bombs, MIME spoofing, path traversal attempts).
- Storage exhaustion via many small uploads from a single user.
- Orphaned attachment data causing quota/accounting drift.
- Malicious markdown/link payloads targeting client execution.
- Public directory join spam and burst join-fail probes across many workspaces.
- Public workspace enumeration attempts via join endpoint status probing.
- Spoofed `x-forwarded-for` chains to bypass IP-scoped abuse controls.
- Rejoin loops after moderation actions (user-ban and IP-ban evasion attempts).

## Directory Join + Guild IP Moderation Threats (Phase 0 contract)
- Join spam/DoS:
  - attacker continuously submits authenticated `POST /guilds/{guild_id}/join` attempts to overload
    membership and audit writes.
  - mitigation contract: explicit per-IP + per-user join caps, bounded audit page size, and bounded
    guild IP-ban record count.
- Workspace enumeration:
  - attacker probes random guild IDs and compares private/nonexistent behavior for membership oracle
    extraction.
  - mitigation contract: policy-consistent responses for private or nonexistent join targets
    (`404 not_found`) and no visibility disclosures outside public directory list results.
- Forwarded-IP spoofing:
  - attacker sets forged `x-forwarded-for` to avoid IP moderation/rate limits.
  - mitigation contract: trusted proxy mode is opt-in; default uses socket peer address; forwarded
    header parsing is strict and canonicalized.
- Rejoin abuse after moderation:
  - attacker cycles accounts and IPs to immediately rejoin after bans.
  - mitigation contract: join path checks both user bans and guild IP bans, records auditable
    rejection reason, and stores user-IP observations for server-side matching without owner IP
    exposure.

## Mandatory Mitigations (Phase 0 baseline)
- Global request body cap and request timeout.
- Baseline per-IP rate limiting.
- Strict protocol envelope with version checks and max message size.
- Structured logging and request IDs for incident correlation.
- CI supply-chain gates (`cargo audit`, `cargo deny`, dependency review, SBOM).
- Directory moderation contract hard limits:
  - join endpoint: bounded per-IP and per-user request rates
  - audit list endpoint: strict cursor format + max page limit
  - guild IP-ban endpoints: bounded list/apply/remove limits and strict reason/expiry validation

## Out of Scope (Current)
- Federation trust relationships.
- End-to-end encryption for group channels/media.
- Multi-region distributed sharding.
