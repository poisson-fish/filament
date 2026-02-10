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
- Malicious markdown/link payloads targeting client execution.

## Mandatory Mitigations (Phase 0 baseline)
- Global request body cap and request timeout.
- Baseline per-IP rate limiting.
- Strict protocol envelope with version checks and max message size.
- Structured logging and request IDs for incident correlation.
- CI supply-chain gates (`cargo audit`, `cargo deny`, dependency review, SBOM).

## Out of Scope (Current)
- Federation trust relationships.
- End-to-end encryption for group channels/media.
- Multi-region distributed sharding.
