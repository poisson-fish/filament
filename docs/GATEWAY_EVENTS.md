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

#### `workspace_update` (planned)
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `updated_fields` (`name`, `visibility`, and future safe workspace settings)
  - `updated_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_member_add` (planned)
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `user_id`
  - `role`
  - `joined_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_member_update` (planned)
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `user_id`
  - `updated_fields` (for example `role`)
  - `updated_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_member_remove` (planned)
- Scope: guild
- Visibility: authorized guild members
- Minimum payload:
  - `guild_id`
  - `user_id`
  - `reason` (`kick` or `ban` or `leave`)
  - `removed_at_unix`
- Optional:
  - `actor_user_id`

#### `workspace_member_ban` (planned)
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

#### `workspace_channel_override_update` (planned)
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

#### `profile_update` (planned)
- Scope: user (plus permitted observers)
- Visibility:
  - user-scoped payload to acting user
  - observer payload to viewers permitted to see profile
- Minimum payload:
  - `user_id`
  - `updated_fields`
  - `updated_at_unix`

#### `profile_avatar_update` (planned)
- Scope: user (plus permitted observers)
- Visibility:
  - user-scoped payload to acting user
  - observer payload to viewers permitted to see avatar
- Minimum payload:
  - `user_id`
  - `avatar_version` or `avatar_url`
  - `updated_at_unix`

#### `friend_request_create` (planned)
- Scope: user
- Visibility: sender + recipient only
- Minimum payload:
  - `request_id`
  - `sender_user_id`
  - `recipient_user_id`
  - `created_at_unix`

#### `friend_request_update` (planned)
- Scope: user
- Visibility: sender + recipient only
- Minimum payload:
  - `request_id`
  - `state`
  - `updated_at_unix`
- Optional:
  - `actor_user_id`

#### `friend_request_delete` (planned)
- Scope: user
- Visibility: sender + recipient only
- Minimum payload:
  - `request_id`
  - `deleted_at_unix`
- Optional:
  - `actor_user_id`

#### `friend_remove` (planned)
- Scope: user
- Visibility: both friendship participants only
- Minimum payload:
  - `user_id`
  - `friend_user_id`
  - `removed_at_unix`
- Optional:
  - `actor_user_id`
