mod migrations;

use filament_core::{ChannelKind, EncryptedChannelPolicy, Permission, PermissionSet, Role};

use self::migrations::v10_role_color_schema::apply_role_color_schema;
use self::migrations::v11_profile_banner_schema::apply_profile_banner_schema;
use self::migrations::v12_e2ee_identity::apply_e2ee_identity_schema;
use self::migrations::v12_e2ee_root_rotation::apply_e2ee_root_rotation_schema;
use self::migrations::v13_e2ee_messages::apply_e2ee_message_schema;
use self::migrations::v14_e2ee_mailbox::apply_e2ee_mailbox_schema;
use self::migrations::v15_e2ee_conversation_provisioning::apply_e2ee_conversation_provisioning_schema;
use self::migrations::v16_e2ee_commit_mailbox::apply_e2ee_commit_mailbox_schema;
use self::migrations::v17_e2ee_proposal_mailbox::apply_e2ee_proposal_mailbox_schema;
use self::migrations::v18_e2ee_group_membership::apply_e2ee_group_membership_schema;
use self::migrations::v19_e2ee_attachment_mailbox::apply_e2ee_attachment_mailbox_schema;
use self::migrations::v1_hierarchical_permissions::backfill_hierarchical_permission_schema;
pub(crate) use self::migrations::v1_hierarchical_permissions::seed_hierarchical_permissions_for_new_guild;
use self::migrations::v20_e2ee_commit_idempotency::apply_e2ee_commit_idempotency_schema;
use self::migrations::v21_e2ee_message_idempotency::apply_e2ee_message_idempotency_schema;
use self::migrations::v22_e2ee_channel_mode::apply_e2ee_channel_mode_schema;
use self::migrations::v23_e2ee_channel_policy::apply_e2ee_channel_policy_schema;
use self::migrations::v24_e2ee_channel_groups::apply_e2ee_channel_group_schema;
use self::migrations::v25_e2ee_channel_membership::apply_e2ee_channel_membership_schema;
use self::migrations::v26_e2ee_channel_authorization::apply_e2ee_channel_authorization_schema;
use self::migrations::v27_e2ee_channel_adds::apply_e2ee_channel_add_schema;
use self::migrations::v28_e2ee_channel_workspace_joins::apply_e2ee_channel_workspace_join_schema;
use self::migrations::v29_e2ee_channel_moderators::apply_e2ee_channel_moderator_schema;
use self::migrations::v2_attachment_schema::apply_attachment_schema;
use self::migrations::v30_e2ee_channel_audiences::apply_e2ee_channel_audience_schema;
use self::migrations::v31_e2ee_channel_policy_transitions::apply_e2ee_channel_policy_transition_schema;
use self::migrations::v3_social_graph_schema::apply_social_graph_schema;
use self::migrations::v4_moderation_audit_schema::apply_moderation_audit_schema;
use self::migrations::v5_identity_schema::apply_identity_schema;
use self::migrations::v6_workspace_schema::apply_workspace_schema;
use self::migrations::v7_message_schema::apply_message_schema;
use self::migrations::v8_permission_legacy_schema::apply_permission_legacy_schema;
use self::migrations::v9_default_join_role_schema::apply_default_join_role_schema;

use super::{
    core::{AppState, GuildVisibility},
    errors::AuthFailure,
};

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

            apply_identity_schema(&mut tx).await?;
            apply_workspace_schema(&mut tx).await?;
            apply_message_schema(&mut tx).await?;
            apply_permission_legacy_schema(&mut tx).await?;

            apply_attachment_schema(&mut tx).await?;

            apply_social_graph_schema(&mut tx).await?;
            apply_moderation_audit_schema(&mut tx).await?;

            backfill_hierarchical_permission_schema(&mut tx).await?;
            apply_default_join_role_schema(&mut tx).await?;
            apply_role_color_schema(&mut tx).await?;
            apply_profile_banner_schema(&mut tx).await?;
            apply_e2ee_identity_schema(&mut tx).await?;
            apply_e2ee_root_rotation_schema(&mut tx).await?;
            apply_e2ee_message_schema(&mut tx).await?;
            apply_e2ee_mailbox_schema(&mut tx).await?;
            apply_e2ee_conversation_provisioning_schema(&mut tx).await?;
            apply_e2ee_commit_mailbox_schema(&mut tx).await?;
            apply_e2ee_proposal_mailbox_schema(&mut tx).await?;
            apply_e2ee_group_membership_schema(&mut tx).await?;
            apply_e2ee_attachment_mailbox_schema(&mut tx).await?;
            apply_e2ee_commit_idempotency_schema(&mut tx).await?;
            apply_e2ee_message_idempotency_schema(&mut tx).await?;
            apply_e2ee_channel_mode_schema(&mut tx).await?;
            apply_e2ee_channel_policy_schema(&mut tx).await?;
            apply_e2ee_channel_group_schema(&mut tx).await?;
            apply_e2ee_channel_membership_schema(&mut tx).await?;
            apply_e2ee_channel_authorization_schema(&mut tx).await?;
            apply_e2ee_channel_add_schema(&mut tx).await?;
            apply_e2ee_channel_workspace_join_schema(&mut tx).await?;
            apply_e2ee_channel_moderator_schema(&mut tx).await?;
            apply_e2ee_channel_audience_schema(&mut tx).await?;
            apply_e2ee_channel_policy_transition_schema(&mut tx).await?;

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

pub(crate) const fn encrypted_channel_policy_to_i16(policy: EncryptedChannelPolicy) -> i16 {
    match policy {
        EncryptedChannelPolicy::Disabled => 0,
        EncryptedChannelPolicy::RequireModeratorMembership => 1,
        EncryptedChannelPolicy::Unrestricted => 2,
    }
}

pub(crate) const fn encrypted_channel_policy_from_i16(
    value: i16,
) -> Option<EncryptedChannelPolicy> {
    match value {
        0 => Some(EncryptedChannelPolicy::Disabled),
        1 => Some(EncryptedChannelPolicy::RequireModeratorMembership),
        2 => Some(EncryptedChannelPolicy::Unrestricted),
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

pub(crate) fn channel_type_to_i16(channel_type: filament_core::ChannelType) -> i16 {
    match channel_type {
        filament_core::ChannelType::Plaintext => 0,
        filament_core::ChannelType::Encrypted => 1,
    }
}

pub(crate) fn channel_type_from_i16(value: i16) -> Option<filament_core::ChannelType> {
    match value {
        0 => Some(filament_core::ChannelType::Plaintext),
        1 => Some(filament_core::ChannelType::Encrypted),
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
    use filament_core::EncryptedChannelPolicy;

    use super::{
        encrypted_channel_policy_from_i16, encrypted_channel_policy_to_i16, ensure_db_schema,
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
    fn encrypted_channel_policy_database_mapping_is_exact() {
        for policy in [
            EncryptedChannelPolicy::Disabled,
            EncryptedChannelPolicy::RequireModeratorMembership,
            EncryptedChannelPolicy::Unrestricted,
        ] {
            assert_eq!(
                encrypted_channel_policy_from_i16(encrypted_channel_policy_to_i16(policy)),
                Some(policy)
            );
        }
        assert_eq!(encrypted_channel_policy_from_i16(-1), None);
        assert_eq!(encrypted_channel_policy_from_i16(3), None);
    }
}
