mod migrations;

use filament_core::{ChannelKind, Permission, PermissionSet, Role};

use self::migrations::v1_hierarchical_permissions::backfill_hierarchical_permission_schema;
use self::migrations::v2_attachment_schema::apply_attachment_schema;
use self::migrations::v3_social_graph_schema::apply_social_graph_schema;
use self::migrations::v4_moderation_audit_schema::apply_moderation_audit_schema;
pub(crate) use self::migrations::v1_hierarchical_permissions::seed_hierarchical_permissions_for_new_guild;

use super::{
    core::{AppState, GuildVisibility},
    errors::AuthFailure,
};

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

            apply_attachment_schema(&mut tx).await?;

            apply_social_graph_schema(&mut tx).await?;
            apply_moderation_audit_schema(&mut tx).await?;
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
        ensure_db_schema, CREATE_CHANNEL_PERMISSION_OVERRIDES_TABLE_SQL,
        CREATE_GUILD_ROLES_TABLE_SQL, CREATE_GUILD_ROLE_MEMBERS_TABLE_SQL,
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
        assert!(CREATE_GUILD_ROLES_TABLE_SQL.contains("guild_roles"));
        assert!(CREATE_GUILD_ROLE_MEMBERS_TABLE_SQL.contains("guild_role_members"));
        assert!(
            CREATE_CHANNEL_PERMISSION_OVERRIDES_TABLE_SQL.contains("channel_permission_overrides")
        );
    }
}
