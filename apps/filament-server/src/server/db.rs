use filament_core::{ChannelKind, Permission, PermissionSet, Role};
use sqlx::{Postgres, Row, Transaction};
use ulid::Ulid;

use super::{
    core::{AppState, GuildVisibility},
    errors::AuthFailure,
    permissions::{
        all_permissions, default_everyone_permissions, default_member_permissions,
        default_moderator_permissions, DEFAULT_ROLE_MEMBER, DEFAULT_ROLE_MODERATOR,
        SYSTEM_ROLE_EVERYONE, SYSTEM_ROLE_WORKSPACE_OWNER,
    },
};

const CREATE_USER_IP_OBSERVATIONS_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS user_ip_observations (
                    observation_id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    ip_cidr TEXT NOT NULL,
                    first_seen_at_unix BIGINT NOT NULL,
                    last_seen_at_unix BIGINT NOT NULL
                )";
const CREATE_USER_IP_OBSERVATIONS_UNIQUE_INDEX_SQL: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_user_ip_observations_user_ip_unique
                    ON user_ip_observations(user_id, ip_cidr)";
const CREATE_GUILD_IP_BANS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS guild_ip_bans (
                    ban_id TEXT PRIMARY KEY,
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    ip_cidr TEXT NOT NULL,
                    source_user_id TEXT NULL REFERENCES users(user_id) ON DELETE SET NULL,
                    reason TEXT NOT NULL,
                    created_by_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    created_at_unix BIGINT NOT NULL,
                    expires_at_unix BIGINT NULL
                )";
const CREATE_USER_IP_OBSERVATIONS_USER_LAST_SEEN_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_user_ip_observations_user_last_seen
                    ON user_ip_observations(user_id, last_seen_at_unix DESC)";
const CREATE_GUILD_IP_BANS_GUILD_CREATED_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_guild_ip_bans_guild_created
                    ON guild_ip_bans(guild_id, created_at_unix DESC)";
const CREATE_AUDIT_LOGS_GUILD_CREATED_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_audit_logs_guild_created
                    ON audit_logs(guild_id, created_at_unix DESC)";
const CREATE_AUDIT_LOGS_GUILD_ACTION_CREATED_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_audit_logs_guild_action_created
                    ON audit_logs(guild_id, action, created_at_unix DESC)";
const CREATE_GUILD_ROLES_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS guild_roles (
                    role_id TEXT PRIMARY KEY,
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    position INTEGER NOT NULL,
                    permissions_allow_mask BIGINT NOT NULL,
                    is_system BOOLEAN NOT NULL DEFAULT FALSE,
                    system_key TEXT NULL,
                    created_at_unix BIGINT NOT NULL
                )";
const CREATE_GUILD_ROLES_GUILD_POSITION_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_guild_roles_guild_position
                    ON guild_roles(guild_id, position DESC, created_at_unix ASC)";
const CREATE_GUILD_ROLES_GUILD_SYSTEM_KEY_UNIQUE_INDEX_SQL: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS idx_guild_roles_guild_system_key_unique
                    ON guild_roles(guild_id, system_key)
                    WHERE system_key IS NOT NULL";
const CREATE_GUILD_ROLE_MEMBERS_TABLE_SQL: &str = "CREATE TABLE IF NOT EXISTS guild_role_members (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    role_id TEXT NOT NULL REFERENCES guild_roles(role_id) ON DELETE CASCADE,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    assigned_at_unix BIGINT NOT NULL,
                    PRIMARY KEY(guild_id, role_id, user_id)
                )";
const CREATE_GUILD_ROLE_MEMBERS_GUILD_USER_INDEX_SQL: &str =
    "CREATE INDEX IF NOT EXISTS idx_guild_role_members_guild_user
                    ON guild_role_members(guild_id, user_id)";
const CREATE_CHANNEL_PERMISSION_OVERRIDES_TABLE_SQL: &str =
    "CREATE TABLE IF NOT EXISTS channel_permission_overrides (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    channel_id TEXT NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
                    target_kind SMALLINT NOT NULL,
                    target_id TEXT NOT NULL,
                    allow_mask BIGINT NOT NULL,
                    deny_mask BIGINT NOT NULL,
                    PRIMARY KEY(guild_id, channel_id, target_kind, target_id)
                )";

const TARGET_KIND_ROLE: i16 = 0;

fn now_unix() -> i64 {
    let now = std::time::SystemTime::now();
    let secs = now
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_else(|_| std::time::Duration::from_secs(0))
        .as_secs();
    i64::try_from(secs).unwrap_or(i64::MAX)
}

#[derive(Debug)]
struct SeededGuildRoleIds {
    workspace_owner_role_id: String,
    member_role_id: String,
    moderator_role_id: String,
}

async fn ensure_seed_roles_for_guild(
    tx: &mut Transaction<'_, Postgres>,
    guild_id: &str,
) -> Result<SeededGuildRoleIds, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT role_id, name, system_key
         FROM guild_roles
         WHERE guild_id = $1",
    )
    .bind(guild_id)
    .fetch_all(&mut **tx)
    .await?;

    let mut everyone_role_id = None;
    let mut workspace_owner_role_id = None;
    let mut member_role_id = None;
    let mut moderator_role_id = None;

    for row in rows {
        let role_id: String = row.try_get("role_id")?;
        let name: String = row.try_get("name")?;
        let system_key = row.try_get::<Option<String>, _>("system_key")?;
        if system_key.as_deref() == Some(SYSTEM_ROLE_EVERYONE) {
            everyone_role_id = Some(role_id.clone());
        }
        if system_key.as_deref() == Some(SYSTEM_ROLE_WORKSPACE_OWNER) {
            workspace_owner_role_id = Some(role_id.clone());
        }
        if system_key.as_deref() == Some(DEFAULT_ROLE_MEMBER)
            || name.eq_ignore_ascii_case(DEFAULT_ROLE_MEMBER)
        {
            member_role_id = Some(role_id.clone());
        }
        if system_key.as_deref() == Some(DEFAULT_ROLE_MODERATOR)
            || name.eq_ignore_ascii_case(DEFAULT_ROLE_MODERATOR)
        {
            moderator_role_id = Some(role_id);
        }
    }

    if everyone_role_id.is_none() {
        let role_id = Ulid::new().to_string();
        sqlx::query(
            "INSERT INTO guild_roles
                (role_id, guild_id, name, position, permissions_allow_mask, is_system, system_key, created_at_unix)
             VALUES
                ($1, $2, $3, $4, $5, TRUE, $6, $7)",
        )
        .bind(&role_id)
        .bind(guild_id)
        .bind("@everyone")
        .bind(0_i32)
        .bind(i64::try_from(default_everyone_permissions().bits()).unwrap_or(i64::MAX))
        .bind(SYSTEM_ROLE_EVERYONE)
        .bind(now_unix())
        .execute(&mut **tx)
        .await?;
        everyone_role_id = Some(role_id);
    }

    if workspace_owner_role_id.is_none() {
        let role_id = Ulid::new().to_string();
        sqlx::query(
            "INSERT INTO guild_roles
                (role_id, guild_id, name, position, permissions_allow_mask, is_system, system_key, created_at_unix)
             VALUES
                ($1, $2, $3, $4, $5, TRUE, $6, $7)",
        )
        .bind(&role_id)
        .bind(guild_id)
        .bind("workspace_owner")
        .bind(10_000_i32)
        .bind(i64::try_from(all_permissions().bits()).unwrap_or(i64::MAX))
        .bind(SYSTEM_ROLE_WORKSPACE_OWNER)
        .bind(now_unix())
        .execute(&mut **tx)
        .await?;
        workspace_owner_role_id = Some(role_id);
    }

    if moderator_role_id.is_none() {
        let role_id = Ulid::new().to_string();
        sqlx::query(
            "INSERT INTO guild_roles
                (role_id, guild_id, name, position, permissions_allow_mask, is_system, system_key, created_at_unix)
             VALUES
                ($1, $2, $3, $4, $5, FALSE, $6, $7)",
        )
        .bind(&role_id)
        .bind(guild_id)
        .bind(DEFAULT_ROLE_MODERATOR)
        .bind(100_i32)
        .bind(i64::try_from(default_moderator_permissions().bits()).unwrap_or(i64::MAX))
        .bind(DEFAULT_ROLE_MODERATOR)
        .bind(now_unix())
        .execute(&mut **tx)
        .await?;
        moderator_role_id = Some(role_id);
    }

    if member_role_id.is_none() {
        let role_id = Ulid::new().to_string();
        sqlx::query(
            "INSERT INTO guild_roles
                (role_id, guild_id, name, position, permissions_allow_mask, is_system, system_key, created_at_unix)
             VALUES
                ($1, $2, $3, $4, $5, FALSE, $6, $7)",
        )
        .bind(&role_id)
        .bind(guild_id)
        .bind(DEFAULT_ROLE_MEMBER)
        .bind(1_i32)
        .bind(i64::try_from(default_member_permissions().bits()).unwrap_or(i64::MAX))
        .bind(DEFAULT_ROLE_MEMBER)
        .bind(now_unix())
        .execute(&mut **tx)
        .await?;
        member_role_id = Some(role_id);
    }

    let _everyone_role_id = everyone_role_id.expect("everyone role should be set");
    Ok(SeededGuildRoleIds {
        workspace_owner_role_id: workspace_owner_role_id
            .expect("workspace owner role should be set"),
        member_role_id: member_role_id.expect("member role should be set"),
        moderator_role_id: moderator_role_id.expect("moderator role should be set"),
    })
}

async fn backfill_role_assignments_for_guild(
    tx: &mut Transaction<'_, Postgres>,
    guild_id: &str,
    created_by_user_id: Option<&str>,
    role_ids: &SeededGuildRoleIds,
) -> Result<(), sqlx::Error> {
    let members = sqlx::query("SELECT user_id, role FROM guild_members WHERE guild_id = $1")
        .bind(guild_id)
        .fetch_all(&mut **tx)
        .await?;

    for row in members {
        let user_id: String = row.try_get("user_id")?;
        let role_value: i16 = row.try_get("role")?;
        let legacy_role = role_from_i16(role_value).unwrap_or(Role::Member);
        let role_id = match legacy_role {
            Role::Owner => Some(&role_ids.workspace_owner_role_id),
            Role::Moderator => Some(&role_ids.moderator_role_id),
            Role::Member => Some(&role_ids.member_role_id),
        };
        if let Some(role_id) = role_id {
            sqlx::query(
                "INSERT INTO guild_role_members (guild_id, role_id, user_id, assigned_at_unix)
                 VALUES ($1, $2, $3, $4)
                 ON CONFLICT (guild_id, role_id, user_id) DO NOTHING",
            )
            .bind(guild_id)
            .bind(role_id)
            .bind(user_id)
            .bind(now_unix())
            .execute(&mut **tx)
            .await?;
        }
    }

    if let Some(owner_user_id) = created_by_user_id {
        sqlx::query(
            "INSERT INTO guild_role_members (guild_id, role_id, user_id, assigned_at_unix)
             SELECT $1, $2, $3, $4
             WHERE EXISTS (
                 SELECT 1 FROM guild_members WHERE guild_id = $1 AND user_id = $3
             )
             ON CONFLICT (guild_id, role_id, user_id) DO NOTHING",
        )
        .bind(guild_id)
        .bind(&role_ids.workspace_owner_role_id)
        .bind(owner_user_id)
        .bind(now_unix())
        .execute(&mut **tx)
        .await?;
    }

    // Ensure each guild keeps at least one workspace owner assignment.
    let owner_count = sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*)
         FROM guild_role_members grm
         JOIN guild_roles gr ON gr.role_id = grm.role_id
         WHERE grm.guild_id = $1
           AND gr.system_key = $2",
    )
    .bind(guild_id)
    .bind(SYSTEM_ROLE_WORKSPACE_OWNER)
    .fetch_one(&mut **tx)
    .await?;
    if owner_count == 0 {
        sqlx::query(
            "INSERT INTO guild_role_members (guild_id, role_id, user_id, assigned_at_unix)
             SELECT $1, $2, gm.user_id, $3
             FROM guild_members gm
             WHERE gm.guild_id = $1
             ORDER BY gm.role DESC, gm.user_id ASC
             LIMIT 1
             ON CONFLICT (guild_id, role_id, user_id) DO NOTHING",
        )
        .bind(guild_id)
        .bind(&role_ids.workspace_owner_role_id)
        .bind(now_unix())
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

async fn backfill_channel_role_overrides_for_guild(
    tx: &mut Transaction<'_, Postgres>,
    guild_id: &str,
    role_ids: &SeededGuildRoleIds,
) -> Result<(), sqlx::Error> {
    let rows = sqlx::query(
        "SELECT channel_id, role, allow_mask, deny_mask
         FROM channel_role_overrides
         WHERE guild_id = $1",
    )
    .bind(guild_id)
    .fetch_all(&mut **tx)
    .await?;

    for row in rows {
        let channel_id: String = row.try_get("channel_id")?;
        let role_value: i16 = row.try_get("role")?;
        let role = role_from_i16(role_value).unwrap_or(Role::Member);
        let target_role_id = match role {
            Role::Owner => &role_ids.workspace_owner_role_id,
            Role::Moderator => &role_ids.moderator_role_id,
            Role::Member => &role_ids.member_role_id,
        };
        let allow_mask: i64 = row.try_get("allow_mask")?;
        let deny_mask: i64 = row.try_get("deny_mask")?;

        sqlx::query(
            "INSERT INTO channel_permission_overrides
                (guild_id, channel_id, target_kind, target_id, allow_mask, deny_mask)
             VALUES
                ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (guild_id, channel_id, target_kind, target_id)
             DO UPDATE SET allow_mask = EXCLUDED.allow_mask, deny_mask = EXCLUDED.deny_mask",
        )
        .bind(guild_id)
        .bind(channel_id)
        .bind(TARGET_KIND_ROLE)
        .bind(target_role_id)
        .bind(allow_mask)
        .bind(deny_mask)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

async fn backfill_hierarchical_permission_schema(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), sqlx::Error> {
    let guild_rows = sqlx::query("SELECT guild_id, created_by_user_id FROM guilds")
        .fetch_all(&mut **tx)
        .await?;

    for row in guild_rows {
        let guild_id: String = row.try_get("guild_id")?;
        let created_by_user_id = row.try_get::<Option<String>, _>("created_by_user_id")?;
        let role_ids = ensure_seed_roles_for_guild(tx, &guild_id).await?;
        backfill_role_assignments_for_guild(
            tx,
            &guild_id,
            created_by_user_id.as_deref(),
            &role_ids,
        )
        .await?;
        backfill_channel_role_overrides_for_guild(tx, &guild_id, &role_ids).await?;
    }

    Ok(())
}

#[allow(clippy::too_many_lines)]
pub(crate) async fn ensure_db_schema(state: &AppState) -> Result<(), AuthFailure> {
    const SCHEMA_INIT_LOCK_ID: i64 = 0x4649_4c41_4d45_4e54;
    let Some(pool) = &state.db_pool else {
        return Ok(());
    };

    state
        .db_init
        .get_or_try_init(|| async move {
            let mut tx = pool.begin().await?;
            sqlx::query("SELECT pg_advisory_xact_lock($1)")
                .bind(SCHEMA_INIT_LOCK_ID)
                .execute(&mut *tx)
                .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS users (
                    user_id TEXT PRIMARY KEY,
                    username TEXT UNIQUE NOT NULL,
                    about_markdown TEXT NOT NULL DEFAULT '',
                    avatar_object_key TEXT NULL,
                    avatar_mime_type TEXT NULL,
                    avatar_size_bytes BIGINT NULL,
                    avatar_sha256_hex TEXT NULL,
                    avatar_version BIGINT NOT NULL DEFAULT 0,
                    password_hash TEXT NOT NULL,
                    failed_logins SMALLINT NOT NULL DEFAULT 0,
                    locked_until_unix BIGINT NULL
                )",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS about_markdown TEXT")
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "UPDATE users
                 SET about_markdown = ''
                 WHERE about_markdown IS NULL",
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "ALTER TABLE users ALTER COLUMN about_markdown SET DEFAULT ''",
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query("ALTER TABLE users ALTER COLUMN about_markdown SET NOT NULL")
                .execute(&mut *tx)
                .await?;

            sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_object_key TEXT")
                .execute(&mut *tx)
                .await?;
            sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_mime_type TEXT")
                .execute(&mut *tx)
                .await?;
            sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_size_bytes BIGINT")
                .execute(&mut *tx)
                .await?;
            sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_sha256_hex TEXT")
                .execute(&mut *tx)
                .await?;

            sqlx::query("ALTER TABLE users ADD COLUMN IF NOT EXISTS avatar_version BIGINT")
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "UPDATE users
                 SET avatar_version = 0
                 WHERE avatar_version IS NULL",
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query("ALTER TABLE users ALTER COLUMN avatar_version SET DEFAULT 0")
                .execute(&mut *tx)
                .await?;
            sqlx::query("ALTER TABLE users ALTER COLUMN avatar_version SET NOT NULL")
                .execute(&mut *tx)
                .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS sessions (
                    session_id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    refresh_token_hash BYTEA NOT NULL,
                    expires_at_unix BIGINT NOT NULL,
                    revoked BOOLEAN NOT NULL DEFAULT FALSE
                )",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS used_refresh_tokens (
                    token_hash BYTEA PRIMARY KEY,
                    session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE
                )",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS guilds (
                    guild_id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    visibility SMALLINT NOT NULL DEFAULT 0,
                    created_by_user_id TEXT REFERENCES users(user_id),
                    created_at_unix BIGINT NOT NULL
                )",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "ALTER TABLE guilds
                 ADD COLUMN IF NOT EXISTS visibility SMALLINT NOT NULL DEFAULT 0",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "ALTER TABLE guilds
                 ADD COLUMN IF NOT EXISTS created_by_user_id TEXT REFERENCES users(user_id)",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS guild_members (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    role SMALLINT NOT NULL,
                    PRIMARY KEY(guild_id, user_id)
                )",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "UPDATE guilds AS g
                 SET created_by_user_id = gm.user_id
                 FROM guild_members AS gm
                 WHERE g.created_by_user_id IS NULL
                   AND gm.guild_id = g.guild_id
                   AND gm.role = $1",
            )
            .bind(role_to_i16(Role::Owner))
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS channels (
                    channel_id TEXT PRIMARY KEY,
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    name TEXT NOT NULL,
                    kind SMALLINT NOT NULL DEFAULT 0,
                    created_at_unix BIGINT NOT NULL
                )",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query("ALTER TABLE channels ADD COLUMN IF NOT EXISTS kind SMALLINT")
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE channels SET kind = $1 WHERE kind IS NULL")
                .bind(channel_kind_to_i16(ChannelKind::Text))
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "ALTER TABLE channels ALTER COLUMN kind SET DEFAULT 0",
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query("ALTER TABLE channels ALTER COLUMN kind SET NOT NULL")
                .execute(&mut *tx)
                .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS messages (
                    message_id TEXT PRIMARY KEY,
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    channel_id TEXT NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
                    author_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    content TEXT NOT NULL,
                    created_at_unix BIGINT NOT NULL
                )",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS channel_role_overrides (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    channel_id TEXT NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
                    role SMALLINT NOT NULL,
                    allow_mask BIGINT NOT NULL,
                    deny_mask BIGINT NOT NULL,
                    PRIMARY KEY(guild_id, channel_id, role)
                )",
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query(CREATE_GUILD_ROLES_TABLE_SQL)
                .execute(&mut *tx)
                .await?;
            sqlx::query(CREATE_GUILD_ROLE_MEMBERS_TABLE_SQL)
                .execute(&mut *tx)
                .await?;
            sqlx::query(CREATE_CHANNEL_PERMISSION_OVERRIDES_TABLE_SQL)
                .execute(&mut *tx)
                .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS message_reactions (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    channel_id TEXT NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
                    message_id TEXT NOT NULL REFERENCES messages(message_id) ON DELETE CASCADE,
                    emoji TEXT NOT NULL,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    created_at_unix BIGINT NOT NULL,
                    PRIMARY KEY(guild_id, channel_id, message_id, emoji, user_id)
                )",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS attachments (
                    attachment_id TEXT PRIMARY KEY,
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    channel_id TEXT NOT NULL REFERENCES channels(channel_id) ON DELETE CASCADE,
                    owner_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    filename TEXT NOT NULL,
                    mime_type TEXT NOT NULL,
                    size_bytes BIGINT NOT NULL,
                    sha256_hex TEXT NOT NULL,
                    object_key TEXT NOT NULL UNIQUE,
                    created_at_unix BIGINT NOT NULL
                )",
            )
            .execute(&mut *tx)
            .await?;

            // Backfill legacy attachment schemas so uploads do not fail after upgrades.
            sqlx::query("ALTER TABLE attachments ADD COLUMN IF NOT EXISTS object_key TEXT")
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "UPDATE attachments
                 SET object_key = CONCAT('attachments/', attachment_id)
                 WHERE object_key IS NULL OR object_key = ''",
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query("ALTER TABLE attachments ALTER COLUMN object_key SET NOT NULL")
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "CREATE UNIQUE INDEX IF NOT EXISTS idx_attachments_object_key_unique
                    ON attachments(object_key)",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query("ALTER TABLE attachments ADD COLUMN IF NOT EXISTS created_at_unix BIGINT")
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "UPDATE attachments
                 SET created_at_unix = 0
                 WHERE created_at_unix IS NULL",
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query("ALTER TABLE attachments ALTER COLUMN created_at_unix SET NOT NULL")
                .execute(&mut *tx)
                .await?;

            sqlx::query(
                "CREATE INDEX IF NOT EXISTS idx_attachments_owner
                    ON attachments(owner_id)",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "ALTER TABLE attachments
                 ADD COLUMN IF NOT EXISTS message_id TEXT NULL REFERENCES messages(message_id) ON DELETE SET NULL",
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "CREATE INDEX IF NOT EXISTS idx_attachments_message
                    ON attachments(message_id)",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS friendships (
                    user_a_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    user_b_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    created_at_unix BIGINT NOT NULL,
                    CHECK (user_a_id < user_b_id),
                    PRIMARY KEY(user_a_id, user_b_id)
                )",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS friendship_requests (
                    request_id TEXT PRIMARY KEY,
                    sender_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    recipient_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    created_at_unix BIGINT NOT NULL,
                    CHECK (sender_user_id <> recipient_user_id)
                )",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "CREATE INDEX IF NOT EXISTS idx_friendship_requests_sender
                    ON friendship_requests(sender_user_id)",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "CREATE INDEX IF NOT EXISTS idx_friendship_requests_recipient
                    ON friendship_requests(recipient_user_id)",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS guild_bans (
                    guild_id TEXT NOT NULL REFERENCES guilds(guild_id) ON DELETE CASCADE,
                    user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    banned_by_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    created_at_unix BIGINT NOT NULL,
                    PRIMARY KEY(guild_id, user_id)
                )",
            )
            .execute(&mut *tx)
            .await?;

            sqlx::query(CREATE_USER_IP_OBSERVATIONS_TABLE_SQL)
            .execute(&mut *tx)
            .await?;
            sqlx::query(CREATE_USER_IP_OBSERVATIONS_UNIQUE_INDEX_SQL)
            .execute(&mut *tx)
            .await?;

            sqlx::query(CREATE_GUILD_IP_BANS_TABLE_SQL)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                "CREATE TABLE IF NOT EXISTS audit_logs (
                    audit_id TEXT PRIMARY KEY,
                    guild_id TEXT NULL,
                    actor_user_id TEXT NOT NULL REFERENCES users(user_id) ON DELETE CASCADE,
                    target_user_id TEXT NULL,
                    action TEXT NOT NULL,
                    details_json TEXT NOT NULL,
                    created_at_unix BIGINT NOT NULL
                )",
            )
            .execute(&mut *tx)
            .await?;
            sqlx::query(CREATE_USER_IP_OBSERVATIONS_USER_LAST_SEEN_INDEX_SQL)
            .execute(&mut *tx)
            .await?;
            sqlx::query(CREATE_GUILD_IP_BANS_GUILD_CREATED_INDEX_SQL)
            .execute(&mut *tx)
            .await?;
            sqlx::query(CREATE_AUDIT_LOGS_GUILD_CREATED_INDEX_SQL)
            .execute(&mut *tx)
            .await?;
            sqlx::query(CREATE_AUDIT_LOGS_GUILD_ACTION_CREATED_INDEX_SQL)
            .execute(&mut *tx)
            .await?;
            sqlx::query(CREATE_GUILD_ROLES_GUILD_POSITION_INDEX_SQL)
                .execute(&mut *tx)
                .await?;
            sqlx::query(CREATE_GUILD_ROLES_GUILD_SYSTEM_KEY_UNIQUE_INDEX_SQL)
                .execute(&mut *tx)
                .await?;
            sqlx::query(CREATE_GUILD_ROLE_MEMBERS_GUILD_USER_INDEX_SQL)
                .execute(&mut *tx)
                .await?;

            sqlx::query(
                "CREATE INDEX IF NOT EXISTS idx_messages_channel_message_id
                    ON messages(channel_id, message_id DESC)",
            )
            .execute(&mut *tx)
            .await?;

            backfill_hierarchical_permission_schema(&mut tx).await?;

            tx.commit().await?;

            Ok::<(), sqlx::Error>(())
        })
        .await
        .map_err(|e| {
            tracing::error!(event = "db.init", error = %e);
            AuthFailure::Internal
        })?;

    Ok(())
}

pub(crate) fn role_to_i16(role: Role) -> i16 {
    match role {
        Role::Owner => 2,
        Role::Moderator => 1,
        Role::Member => 0,
    }
}

pub(crate) fn role_from_i16(value: i16) -> Option<Role> {
    match value {
        2 => Some(Role::Owner),
        1 => Some(Role::Moderator),
        0 => Some(Role::Member),
        _ => None,
    }
}

pub(crate) fn visibility_to_i16(visibility: GuildVisibility) -> i16 {
    match visibility {
        GuildVisibility::Private => 0,
        GuildVisibility::Public => 1,
    }
}

pub(crate) fn visibility_from_i16(value: i16) -> Option<GuildVisibility> {
    match value {
        0 => Some(GuildVisibility::Private),
        1 => Some(GuildVisibility::Public),
        _ => None,
    }
}

pub(crate) fn channel_kind_to_i16(kind: ChannelKind) -> i16 {
    match kind {
        ChannelKind::Text => 0,
        ChannelKind::Voice => 1,
    }
}

pub(crate) fn channel_kind_from_i16(value: i16) -> Option<ChannelKind> {
    match value {
        0 => Some(ChannelKind::Text),
        1 => Some(ChannelKind::Voice),
        _ => None,
    }
}

pub(crate) fn permission_set_to_i64(value: PermissionSet) -> Result<i64, AuthFailure> {
    i64::try_from(value.bits()).map_err(|_| AuthFailure::Internal)
}

pub(crate) fn permission_set_from_list(values: &[Permission]) -> PermissionSet {
    let mut set = PermissionSet::empty();
    for permission in values {
        set.insert(*permission);
    }
    set
}

pub(crate) fn permission_list_from_set(value: PermissionSet) -> Vec<Permission> {
    const ORDERED_PERMISSIONS: [Permission; 12] = [
        Permission::ManageRoles,
        Permission::ManageMemberRoles,
        Permission::ManageWorkspaceRoles,
        Permission::ManageChannelOverrides,
        Permission::DeleteMessage,
        Permission::BanMember,
        Permission::ViewAuditLog,
        Permission::ManageIpBans,
        Permission::CreateMessage,
        Permission::PublishVideo,
        Permission::PublishScreenShare,
        Permission::SubscribeStreams,
    ];

    ORDERED_PERMISSIONS
        .into_iter()
        .filter(|permission| value.contains(*permission))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ensure_db_schema, CREATE_AUDIT_LOGS_GUILD_ACTION_CREATED_INDEX_SQL,
        CREATE_AUDIT_LOGS_GUILD_CREATED_INDEX_SQL, CREATE_CHANNEL_PERMISSION_OVERRIDES_TABLE_SQL,
        CREATE_GUILD_IP_BANS_GUILD_CREATED_INDEX_SQL, CREATE_GUILD_IP_BANS_TABLE_SQL,
        CREATE_GUILD_ROLES_TABLE_SQL, CREATE_GUILD_ROLE_MEMBERS_TABLE_SQL,
        CREATE_USER_IP_OBSERVATIONS_TABLE_SQL, CREATE_USER_IP_OBSERVATIONS_UNIQUE_INDEX_SQL,
        CREATE_USER_IP_OBSERVATIONS_USER_LAST_SEEN_INDEX_SQL,
    };
    use crate::server::core::{AppConfig, AppState};

    #[tokio::test]
    async fn schema_init_is_noop_and_idempotent_without_database_pool() {
        let state = AppState::new(&AppConfig::default()).expect("app state should initialize");
        ensure_db_schema(&state)
            .await
            .expect("schema init without database should succeed");
        ensure_db_schema(&state)
            .await
            .expect("schema init should be idempotent");
    }

    #[test]
    fn phase_one_schema_statements_define_required_tables_and_indexes() {
        assert!(CREATE_USER_IP_OBSERVATIONS_TABLE_SQL.contains("user_ip_observations"));
        assert!(CREATE_GUILD_IP_BANS_TABLE_SQL.contains("guild_ip_bans"));
        assert!(CREATE_USER_IP_OBSERVATIONS_UNIQUE_INDEX_SQL
            .contains("idx_user_ip_observations_user_ip_unique"));
        assert!(CREATE_USER_IP_OBSERVATIONS_USER_LAST_SEEN_INDEX_SQL
            .contains("idx_user_ip_observations_user_last_seen"));
        assert!(CREATE_GUILD_IP_BANS_GUILD_CREATED_INDEX_SQL
            .contains("idx_guild_ip_bans_guild_created"));
        assert!(CREATE_AUDIT_LOGS_GUILD_CREATED_INDEX_SQL.contains("idx_audit_logs_guild_created"));
        assert!(CREATE_AUDIT_LOGS_GUILD_ACTION_CREATED_INDEX_SQL
            .contains("idx_audit_logs_guild_action_created"));
        assert!(CREATE_GUILD_ROLES_TABLE_SQL.contains("guild_roles"));
        assert!(CREATE_GUILD_ROLE_MEMBERS_TABLE_SQL.contains("guild_role_members"));
        assert!(
            CREATE_CHANNEL_PERMISSION_OVERRIDES_TABLE_SQL.contains("channel_permission_overrides")
        );
    }
}
