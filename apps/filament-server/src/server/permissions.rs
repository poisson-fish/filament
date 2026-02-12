use std::collections::HashSet;

use filament_core::{Permission, PermissionSet, Role};

pub(crate) const SYSTEM_ROLE_EVERYONE: &str = "everyone";
pub(crate) const SYSTEM_ROLE_WORKSPACE_OWNER: &str = "workspace_owner";
pub(crate) const DEFAULT_ROLE_MODERATOR: &str = "moderator";
pub(crate) const DEFAULT_ROLE_MEMBER: &str = "member";

pub(crate) const MAX_GUILD_ROLES: usize = 64;
pub(crate) const MAX_MEMBER_ROLE_ASSIGNMENTS: usize = 16;
pub(crate) const MAX_ROLE_NAME_CHARS: usize = 32;

const KNOWN_PERMISSIONS: [Permission; 12] = [
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

pub(crate) fn known_permission_mask() -> u64 {
    KNOWN_PERMISSIONS
        .into_iter()
        .fold(0_u64, |bits, permission| bits | permission_mask(permission))
}

pub(crate) fn all_permissions() -> PermissionSet {
    PermissionSet::from_bits(known_permission_mask())
}

pub(crate) fn mask_permissions(raw_bits: u64) -> (PermissionSet, u64) {
    let mask = known_permission_mask();
    let masked = raw_bits & mask;
    let unknown = raw_bits & !mask;
    (PermissionSet::from_bits(masked), unknown)
}

pub(crate) fn default_everyone_permissions() -> PermissionSet {
    let mut permissions = PermissionSet::empty();
    permissions.insert(Permission::CreateMessage);
    permissions.insert(Permission::SubscribeStreams);
    permissions
}

pub(crate) fn default_moderator_permissions() -> PermissionSet {
    let mut permissions = PermissionSet::empty();
    permissions.insert(Permission::ManageMemberRoles);
    permissions.insert(Permission::ManageChannelOverrides);
    permissions.insert(Permission::DeleteMessage);
    permissions.insert(Permission::BanMember);
    permissions.insert(Permission::ViewAuditLog);
    permissions.insert(Permission::ManageIpBans);
    permissions.insert(Permission::CreateMessage);
    permissions.insert(Permission::PublishVideo);
    permissions.insert(Permission::PublishScreenShare);
    permissions.insert(Permission::SubscribeStreams);
    permissions
}

pub(crate) fn default_member_permissions() -> PermissionSet {
    let mut permissions = PermissionSet::empty();
    permissions.insert(Permission::CreateMessage);
    permissions.insert(Permission::SubscribeStreams);
    permissions
}

pub(crate) fn membership_to_legacy_role(
    role_ids: &HashSet<String>,
    workspace_owner_role_id: &str,
    moderator_role_id: &str,
) -> Role {
    if role_ids.contains(workspace_owner_role_id) {
        return Role::Owner;
    }
    if role_ids.contains(moderator_role_id) {
        return Role::Moderator;
    }
    Role::Member
}

fn permission_mask(permission: Permission) -> u64 {
    match permission {
        Permission::ManageRoles => 1 << 0,
        Permission::ManageMemberRoles => 1 << 1,
        Permission::ManageWorkspaceRoles => 1 << 2,
        Permission::ManageChannelOverrides => 1 << 3,
        Permission::DeleteMessage => 1 << 4,
        Permission::BanMember => 1 << 5,
        Permission::ViewAuditLog => 1 << 6,
        Permission::ManageIpBans => 1 << 7,
        Permission::CreateMessage => 1 << 8,
        Permission::PublishVideo => 1 << 9,
        Permission::PublishScreenShare => 1 << 10,
        Permission::SubscribeStreams => 1 << 11,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        all_permissions, default_everyone_permissions, default_member_permissions,
        default_moderator_permissions, mask_permissions, DEFAULT_ROLE_MEMBER,
        DEFAULT_ROLE_MODERATOR, SYSTEM_ROLE_EVERYONE, SYSTEM_ROLE_WORKSPACE_OWNER,
    };
    use filament_core::Permission;

    #[test]
    fn known_permission_masking_drops_unknown_bits() {
        let (masked, unknown) = mask_permissions((1 << 30) | (1 << 4));
        assert!(masked.contains(Permission::DeleteMessage));
        assert_eq!(unknown, 1 << 30);
    }

    #[test]
    fn default_permission_sets_match_phase_seven_matrix() {
        let everyone = default_everyone_permissions();
        assert!(everyone.contains(Permission::CreateMessage));
        assert!(everyone.contains(Permission::SubscribeStreams));
        assert!(!everyone.contains(Permission::DeleteMessage));

        let moderator = default_moderator_permissions();
        assert!(moderator.contains(Permission::ManageMemberRoles));
        assert!(moderator.contains(Permission::ManageIpBans));
        assert!(!moderator.contains(Permission::ManageWorkspaceRoles));

        let owner = all_permissions();
        let full = all_permissions();
        assert_eq!(owner.bits(), full.bits());

        let member = default_member_permissions();
        assert!(member.contains(Permission::CreateMessage));
        assert!(member.contains(Permission::SubscribeStreams));
        assert!(!member.contains(Permission::DeleteMessage));
    }

    #[test]
    fn phase_seven_role_keys_are_stable() {
        assert_eq!(SYSTEM_ROLE_EVERYONE, "everyone");
        assert_eq!(SYSTEM_ROLE_WORKSPACE_OWNER, "workspace_owner");
        assert_eq!(DEFAULT_ROLE_MODERATOR, "moderator");
        assert_eq!(DEFAULT_ROLE_MEMBER, "member");
    }
}
