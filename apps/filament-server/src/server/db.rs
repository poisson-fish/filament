use filament_core::{ChannelKind, Permission, PermissionSet, Role};

use super::{
    core::{AppState, GuildVisibility},
    errors::AuthFailure,
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

            sqlx::query(
                "CREATE INDEX IF NOT EXISTS idx_messages_channel_message_id
                    ON messages(channel_id, message_id DESC)",
            )
            .execute(&mut *tx)
            .await?;

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

pub(crate) fn permission_set_from_i64(value: i64) -> Result<PermissionSet, AuthFailure> {
    let bits = u64::try_from(value).map_err(|_| AuthFailure::Internal)?;
    Ok(PermissionSet::from_bits(bits))
}

pub(crate) fn permission_set_from_list(values: &[Permission]) -> PermissionSet {
    let mut set = PermissionSet::empty();
    for permission in values {
        set.insert(*permission);
    }
    set
}

pub(crate) fn permission_list_from_set(value: PermissionSet) -> Vec<Permission> {
    const ORDERED_PERMISSIONS: [Permission; 8] = [
        Permission::ManageRoles,
        Permission::ManageChannelOverrides,
        Permission::DeleteMessage,
        Permission::BanMember,
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
        CREATE_AUDIT_LOGS_GUILD_CREATED_INDEX_SQL, CREATE_GUILD_IP_BANS_GUILD_CREATED_INDEX_SQL,
        CREATE_GUILD_IP_BANS_TABLE_SQL, CREATE_USER_IP_OBSERVATIONS_TABLE_SQL,
        CREATE_USER_IP_OBSERVATIONS_UNIQUE_INDEX_SQL,
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
    }
}
