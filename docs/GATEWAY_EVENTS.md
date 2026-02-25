# Filament Gateway Events

This document is the canonical contract for server-emitted realtime events.

## Envelope
All events use the versioned envelope:

```json
{ "v": 1, "t": "event_name", "d": { "...": "payload" } }
```

- `v` must be `1`.
- `t` must match `[a-z0-9_.]{1,64}`.
- `d` is a JSON object payload validated per event schema.

## Compatibility
- Clients must ignore unknown event types to support mixed-version rollout.
- Clients must reject malformed envelopes/payloads.
- Server must emit additive payload changes only (new optional fields), never breaking required fields.

## Scope and Visibility Rules
- Channel-scoped events are delivered only to authenticated connections subscribed to the matching
  `{guild_id, channel_id}` and authorized for that channel.
- Guild-scoped events are delivered only to authenticated guild members. Event payloads must not
  include privileged-only fields unless every recipient in that fanout is authorized to see them.
- User-scoped events are delivered only to the authenticated target user unless explicitly marked
  as observer-visible.

## Actor Metadata Policy
- `actor_user_id` is optional and omitted by default.
- Include `actor_user_id` only for mutation/audit-useful events where the actor identity is already
  visible to recipients under existing permissions.
- Never include hidden moderator/admin identifiers in broadly fanned-out payloads.
- User-scoped self events (`profile_update`, `profile_avatar_update`) do not require
  `actor_user_id` because `user_id` is the target identity.

## Event Catalog

### Connection Events

#### `ready`
- Scope: user connection
- Visibility: authenticated connection only
- Minimum payload:
  - `user_id`

#### `subscribed`
- Scope: user connection
- Visibility: authenticated connection only
- Minimum payload:
  - `guild_id`
  - `channel_id`

### Channel-Scoped Events

#### `message_create`
- Scope: channel
- Visibility: authorized channel subscribers
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `message` (full message snapshot, including `message_id`, `author_user_id`, `content`,
    `created_at_unix`, and attachment/reaction snapshots)

#### `message_update`
- Scope: channel
- Visibility: authorized channel subscribers
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `message_id`
  - `updated_fields` (object containing only changed fields)
  - `updated_at_unix`
- Optional:
  - `actor_user_id`

#### `message_delete`
- Scope: channel
- Visibility: authorized channel subscribers
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `message_id`
  - `deleted_at_unix`
- Optional:
  - `actor_user_id`

#### `message_reaction`
- Scope: channel
- Visibility: authorized channel subscribers
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `message_id`
  - `emoji`
  - `count`

#### `channel_create`
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `channel` (`channel_id`, `name`, `kind`)
- Optional:
  - `actor_user_id`

#### `channel_update` (planned)
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `updated_fields`
  - `updated_at_unix`
- Optional:
  - `actor_user_id`

#### `channel_delete` (planned)
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `deleted_at_unix`
- Optional:
  - `actor_user_id`

### Guild-Scoped Presence and Workspace Events

#### `presence_sync`
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `user_ids` (currently online users)

#### `presence_update`
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `user_id`
  - `status` (`online` or `offline`)

### Voice Realtime Events

#### `voice_participant_sync`
- Scope: channel
- Visibility: authorized channel subscribers only
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `participants` (bounded snapshot array)
    - `user_id`
    - `identity`
    - `joined_at_unix`
    - `updated_at_unix`
    - `is_muted`
    - `is_deafened`
    - `is_speaking`
    - `is_video_enabled`
    - `is_screen_share_enabled`
  - `synced_at_unix`
- Notes:
  - emitted on channel subscribe to repair drift after reconnect/resubscribe
  - no token/session secrets, no network metadata

#### `voice_participant_join`
- Scope: channel
- Visibility: authorized channel subscribers only
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `participant` (same shape as `voice_participant_sync.participants[]`)

#### `voice_participant_leave`
- Scope: channel
- Visibility: authorized channel subscribers only
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `user_id`
  - `identity`
  - `left_at_unix`

#### `voice_participant_update`
- Scope: channel
- Visibility: authorized channel subscribers only
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `user_id`
  - `identity`
  - `updated_fields` (additive object; one or more)
    - `is_muted`
    - `is_deafened`
    - `is_speaking`
    - `is_video_enabled`
    - `is_screen_share_enabled`
  - `updated_at_unix`

#### `voice_stream_publish`
- Scope: channel
- Visibility: authorized channel subscribers only
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `user_id`
  - `identity`
  - `stream` (`microphone` | `camera` | `screen_share`)
  - `published_at_unix`

#### `voice_stream_unpublish`
- Scope: channel
- Visibility: authorized channel subscribers only
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `user_id`
  - `identity`
  - `stream` (`microphone` | `camera` | `screen_share`)
  - `unpublished_at_unix`

#### `workspace_update`
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `updated_fields` (`name`, `visibility`, and future safe workspace settings)
  - `updated_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_member_add`
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `user_id`
  - `role`
  - `joined_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_member_update`
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `user_id`
  - `updated_fields` (for example `role`)
  - `updated_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_member_remove`
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `user_id`
  - `reason` (`kick` or `ban` or `leave`)
  - `removed_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_member_ban`
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `user_id`
  - `banned_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_role_create` (planned)
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `role`
- Optional:
  - `actor_user_id`

#### `workspace_role_update` (planned)
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `role_id`
  - `updated_fields`
  - `updated_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_role_delete` (planned)
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `role_id`
  - `deleted_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_role_reorder` (planned)
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `role_ids` (ordered list)
  - `updated_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_role_assignment_add` (planned)
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `user_id`
  - `role_id`
  - `assigned_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_role_assignment_remove` (planned)
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `user_id`
  - `role_id`
  - `removed_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_channel_override_update` (legacy migration event)
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `role`
  - `updated_fields`
  - `updated_at_unix`
- Optional:
  - `actor_user_id`
- Migration notes:
  - This legacy event name is retained only for mixed-version rollout safety.
  - Payload shape is role-based only (`role` + `updated_fields`).
  - New producers should emit the explicit events below.

#### `workspace_channel_role_override_update`
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `role`
  - `updated_fields`
  - `updated_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_channel_permission_override_update`
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `channel_id`
  - `target_kind` (`role` | `member`)
  - `target_id`
  - `updated_fields`
  - `updated_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_ip_ban_sync` (planned, redacted)
- Scope: guild moderation views
- Visibility: authorized owner/moderator viewers only
- Minimum payload:
  - `guild_id`
  - `summary` (redacted counts and action metadata only)
  - `updated_at_unix`
- Optional:
  - `actor_user_id`

### User-Scoped Events

#### `profile_update`
- Scope: user (plus permitted observers)
- Visibility:
  - user-scoped payload to acting user
  - observer payload to friendship participants connected on gateway
- Minimum payload:
  - `user_id`
  - `updated_fields`
  - `updated_at_unix`

#### `profile_avatar_update`
- Scope: user (plus permitted observers)
- Visibility:
  - user-scoped payload to acting user
  - observer payload to friendship participants connected on gateway
- Minimum payload:
  - `user_id`
  - `avatar_version`
  - `updated_at_unix`

#### `friend_request_create`
- Scope: user
- Visibility: sender + recipient only
- Minimum payload:
  - `request_id`
  - `sender_user_id`
  - `recipient_user_id`
  - `created_at_unix`

#### `friend_request_update`
- Scope: user
- Visibility: sender + recipient only
- Minimum payload:
  - `request_id`
  - `state`
  - `updated_at_unix`
- Optional:
  - `actor_user_id`

#### `friend_request_delete`
- Scope: user
- Visibility: sender + recipient only
- Minimum payload:
  - `request_id`
  - `deleted_at_unix`
- Optional:
  - `actor_user_id`

#### `friend_remove`
- Scope: user
- Visibility: both friendship participants only
- Minimum payload:
  - `user_id`
  - `friend_user_id`
  - `removed_at_unix`
- Optional:
  - `actor_user_id`

## Rollout Checklist
- Deploy server event additions before client features that require them.
- Keep existing envelope version at `v=1`; add only optional payload fields during minor rollouts.
- Verify `/metrics` includes:
  - `filament_gateway_events_emitted_total`
  - `filament_gateway_events_dropped_total`
  - `filament_gateway_events_unknown_received_total`
  - `filament_gateway_events_parse_rejected_total`
  - `filament_voice_sync_repairs_total`
- Watch for spikes in:
  - dropped events (`reason="full_queue"` or `reason="closed"`)
  - parse-rejected ingress events (`scope="ingress"`)
  - unknown ingress event types from stale/misbehaving clients
- Roll out web/desktop clients gradually and confirm critical realtime paths:
  - message create/update/delete
  - workspace rename/visibility update
  - role and override changes
  - profile and friendship updates
  - voice participant sync/join/leave and stream publish/unpublish

## Mixed-Version Fallback Behavior
- New clients must ignore unknown event types and continue processing subsequent events.
- Clients must fail closed on malformed payloads: reject the event, keep connection behavior unchanged, and avoid local state mutation.
- Server payload changes must remain additive and backward compatible until all supported clients are updated.
- If a client does not recognize an event needed for local state freshness, it must rely on existing REST refresh flows instead of speculative local mutation.
