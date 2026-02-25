import { createMemo, type Accessor } from "solid-js";
import type {
  ChannelId,
  ChannelPermissionSnapshot,
  GuildId,
  PermissionName,
  RoleName,
  UserId,
} from "../../../domain/chat";
import {
  permissionListFromBits,
  resolveAssignedRoleIdsForUser,
  resolveEffectiveChannelPermissions,
  resolveEffectiveLegacyRolePermissions,
} from "./effective-permissions";
import type {
  WorkspaceChannelOverridesByGuildId,
  WorkspaceRolesByGuildId,
  WorkspaceUserRolesByGuildId,
} from "../state/workspace-state";

export interface ClientPermissionLayerOptions {
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  currentUserId: Accessor<UserId | null>;
  channelPermissions: Accessor<ChannelPermissionSnapshot | null>;
  workspaceRolesByGuildId: Accessor<WorkspaceRolesByGuildId>;
  workspaceUserRolesByGuildId: Accessor<WorkspaceUserRolesByGuildId>;
  workspaceChannelOverridesByGuildId: Accessor<WorkspaceChannelOverridesByGuildId>;
  viewAsRoleSimulatorEnabled: Accessor<boolean>;
  viewAsRoleSimulatorRole: Accessor<RoleName>;
}

export interface ClientPermissionLayer {
  effectivePermissions: Accessor<PermissionName[]>;
  workspaceEffectivePermissions: Accessor<PermissionName[]>;
  canAccessActiveChannel: Accessor<boolean>;
  canPublishVoiceCamera: Accessor<boolean>;
  canPublishVoiceScreenShare: Accessor<boolean>;
  canSubscribeVoiceStreams: Accessor<boolean>;
  canManageWorkspaceChannels: Accessor<boolean>;
  canManageSearchMaintenance: Accessor<boolean>;
  canManageWorkspaceRoles: Accessor<boolean>;
  canManageMemberRoles: Accessor<boolean>;
  hasRoleManagementAccess: Accessor<boolean>;
  canManageRoles: Accessor<boolean>;
  canManageChannelOverrides: Accessor<boolean>;
  canBanMembers: Accessor<boolean>;
  canDeleteMessages: Accessor<boolean>;
  hasModerationAccess: Accessor<boolean>;
}

function hasPermission(
  effectivePermissions: PermissionName[],
  permission: PermissionName,
): boolean {
  return effectivePermissions.includes(permission);
}

export function createClientPermissionLayer(
  options: ClientPermissionLayerOptions,
): ClientPermissionLayer {
  const effectivePermissions = createMemo<PermissionName[]>(() => {
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    if (!guildId || !channelId) {
      return permissionListFromBits(0);
    }

    const roles = options.workspaceRolesByGuildId()[guildId] ?? [];
    const userRoleAssignments = options.workspaceUserRolesByGuildId()[guildId];
    const assignedRoleIds = resolveAssignedRoleIdsForUser(
      options.currentUserId(),
      userRoleAssignments,
    );
    const channelOverrides =
      options.workspaceChannelOverridesByGuildId()[guildId]?.[channelId] ?? [];
    if (options.viewAsRoleSimulatorEnabled()) {
      return permissionListFromBits(
        resolveEffectiveLegacyRolePermissions({
          role: options.viewAsRoleSimulatorRole(),
          guildRoles: roles,
          channelOverrides,
        }),
      );
    }
    const bits = resolveEffectiveChannelPermissions({
      channelPermissionsSnapshot: options.channelPermissions(),
      guildRoles: roles,
      assignedRoleIds,
      channelOverrides,
    });
    return permissionListFromBits(bits);
  });

  const workspaceEffectivePermissions = createMemo<PermissionName[]>(() => {
    const guildId = options.activeGuildId();
    if (!guildId) {
      return permissionListFromBits(0);
    }

    const roles = options.workspaceRolesByGuildId()[guildId] ?? [];
    const userRoleAssignments = options.workspaceUserRolesByGuildId()[guildId];
    const assignedRoleIds = resolveAssignedRoleIdsForUser(
      options.currentUserId(),
      userRoleAssignments,
    );
    if (options.viewAsRoleSimulatorEnabled()) {
      return permissionListFromBits(
        resolveEffectiveLegacyRolePermissions({
          role: options.viewAsRoleSimulatorRole(),
          guildRoles: roles,
          channelOverrides: [],
        }),
      );
    }
    const assignedRoleBits = resolveEffectiveChannelPermissions({
      channelPermissionsSnapshot: null,
      guildRoles: roles,
      assignedRoleIds,
      channelOverrides: [],
    });
    const snapshotRole = options.channelPermissions()?.role ?? null;
    const legacyRoleBits = snapshotRole
      ? resolveEffectiveLegacyRolePermissions({
        role: snapshotRole,
        guildRoles: roles,
        channelOverrides: [],
      })
      : 0;
    return permissionListFromBits(assignedRoleBits | legacyRoleBits);
  });

  const canAccessActiveChannel = createMemo(() =>
    hasPermission(effectivePermissions(), "create_message"),
  );
  const canPublishVoiceCamera = createMemo(() =>
    hasPermission(effectivePermissions(), "publish_video"),
  );
  const canPublishVoiceScreenShare = createMemo(() =>
    hasPermission(effectivePermissions(), "publish_screen_share"),
  );
  const canSubscribeVoiceStreams = createMemo(() =>
    hasPermission(effectivePermissions(), "subscribe_streams"),
  );
  const canManageWorkspaceChannels = createMemo(() =>
    hasPermission(effectivePermissions(), "manage_channel_overrides"),
  );
  const canManageSearchMaintenance = createMemo(() => canManageWorkspaceChannels());
  const canManageWorkspaceRoles = createMemo(
    () =>
      hasPermission(workspaceEffectivePermissions(), "manage_workspace_roles") ||
      hasPermission(workspaceEffectivePermissions(), "manage_roles"),
  );
  const canManageMemberRoles = createMemo(
    () =>
      hasPermission(workspaceEffectivePermissions(), "manage_member_roles") ||
      hasPermission(workspaceEffectivePermissions(), "manage_roles"),
  );
  const hasRoleManagementAccess = createMemo(
    () => canManageWorkspaceRoles() || canManageMemberRoles(),
  );
  const canManageRoles = createMemo(() => hasRoleManagementAccess());
  const canManageChannelOverrides = createMemo(() =>
    hasPermission(effectivePermissions(), "manage_channel_overrides"),
  );
  const canBanMembers = createMemo(() =>
    hasPermission(effectivePermissions(), "ban_member"),
  );
  const canDeleteMessages = createMemo(() =>
    hasPermission(effectivePermissions(), "delete_message"),
  );
  const hasModerationAccess = createMemo(
    () => canManageRoles() || canBanMembers() || canManageChannelOverrides(),
  );

  return {
    effectivePermissions,
    workspaceEffectivePermissions,
    canAccessActiveChannel,
    canPublishVoiceCamera,
    canPublishVoiceScreenShare,
    canSubscribeVoiceStreams,
    canManageWorkspaceChannels,
    canManageSearchMaintenance,
    canManageWorkspaceRoles,
    canManageMemberRoles,
    hasRoleManagementAccess,
    canManageRoles,
    canManageChannelOverrides,
    canBanMembers,
    canDeleteMessages,
    hasModerationAccess,
  };
}
