# Filament Protocol (Gateway + Compatibility)

## Envelope
All gateway events are encoded as a strict JSON envelope:

```json
{ "v": 1, "t": "event_name", "d": { "...": "payload" } }
```

- `v`: protocol version number.
- `t`: event type identifier. Allowed characters are `a-z`, `0-9`, `_`, `.` and max length is 64.
- `d`: payload object for the event.

## Compatibility Rules
- Current supported envelope version is `1`.
- Unknown or unsupported versions are rejected before event routing.
- Unknown top-level fields are rejected (`deny_unknown_fields`).
- Unknown event types are rejected at the boundary.

## Size Limits
- Maximum decoded event size: `64 KiB`.
- Payloads over this limit are rejected immediately.

## Parsing Rules
- Parse into DTOs first.
- Convert into domain types using `TryFrom` and invariant constructors.
- Handlers must only receive validated domain values.

## Security Requirements
- No ad-hoc JSON routing based on unchecked strings.
- No HTML rendering path in protocol payload handling.
- Log protocol rejections with request/session identifiers for abuse triage.
