import type {
  PermissionName,
  RoleName,
} from "../../../domain/chat";
import type { createAttachmentController } from "../controllers/attachment-controller";
import type { createFriendshipController } from "../controllers/friendship-controller";
import type { createModerationController } from "../controllers/moderation-controller";
import type { createSearchController } from "../controllers/search-controller";
import {
  permissionListFromBits,
  resolveEffectiveLegacyRolePermissions,
} from "../permissions/effective-permissions";
import type { CreateAppShellSelectorsResult } from "../selectors/create-app-shell-selectors";
import type { createDiagnosticsState } from "../state/diagnostics-state";
import type { createMessageState } from "../state/message-state";
import type { createWorkspaceState } from "../state/workspace-state";
import type { createAppShellRuntimeLabels } from "./runtime-labels";
import type { CollaborationPanelPropGroupsStateOptions } from "./collaboration-panel-prop-groups-options";

export interface CollaborationPanelHostStateOptions {
  workspaceChannelState: ReturnType<typeof createWorkspaceState>["workspaceChannel"];
  friendshipsState: ReturnType<typeof createWorkspaceState>["friendships"];
  discoveryState: ReturnType<typeof createWorkspaceState>["discovery"];
  messageState: ReturnType<typeof createMessageState>;
  diagnosticsState: ReturnType<typeof createDiagnosticsState>;
  selectors: CreateAppShellSelectorsResult;
  friendshipActions: ReturnType<typeof createFriendshipController>;
  searchActions: ReturnType<typeof createSearchController>;
  attachmentActions: ReturnType<typeof createAttachmentController>;
  moderationActions: ReturnType<typeof createModerationController>;
  labels: ReturnType<typeof createAppShellRuntimeLabels>;
  openOverlayPanel: (panel: "role-management") => void;
}

const LEGACY_OVERRIDE_ROLE_ORDER: readonly RoleName[] = ["member", "moderator", "owner"];

const LEGACY_OVERRIDE_ROLE_LABEL: Record<RoleName, string> = {
  member: "@everyone",
  moderator: "moderator",
  owner: "owner",
};

function normalizePermissionList(
  permissions: ReadonlyArray<PermissionName>,
): PermissionName[] {
  return [...new Set(permissions)];
}

function normalizeOverridePermissions(input: {
  allow: ReadonlyArray<PermissionName>;
  deny: ReadonlyArray<PermissionName>;
}): { allow: PermissionName[]; deny: PermissionName[] } {
  const allow = normalizePermissionList(input.allow);
  const deny = normalizePermissionList(input.deny).filter(
    (permission) => !allow.includes(permission),
  );
  return { allow, deny };
}

function buildChannelOverrideEntities(options: {
  overrides: ReturnType<
    ReturnType<typeof createWorkspaceState>["workspaceChannel"]["workspaceChannelOverridesByGuildId"]
  >[string][string] | undefined;
  selectedRole: RoleName;
}): CollaborationPanelPropGroupsStateOptions["channelOverrideEntities"] {
  const byRole = new Map<RoleName, {
    allow: PermissionName[];
    deny: PermissionName[];
    updatedAtUnix: number | null;
  }>();

  for (const entry of options.overrides ?? []) {
    if (entry.targetKind !== "legacy_role") {
      continue;
    }
    const normalized = normalizeOverridePermissions({
      allow: entry.allow,
      deny: entry.deny,
    });
    byRole.set(entry.role, {
      allow: normalized.allow,
      deny: normalized.deny,
      updatedAtUnix: entry.updatedAtUnix,
    });
  }

  const entities = LEGACY_OVERRIDE_ROLE_ORDER.map((role) => {
    const existing = byRole.get(role);
    const isSelectedWithoutOverride = role === options.selectedRole && !existing;
    if (existing) {
      return {
        role,
        label: LEGACY_OVERRIDE_ROLE_LABEL[role],
        hasExplicitOverride: true,
        allow: existing.allow,
        deny: existing.deny,
        updatedAtUnix: existing.updatedAtUnix,
      };
    }
    if (isSelectedWithoutOverride) {
      return {
        role,
        label: LEGACY_OVERRIDE_ROLE_LABEL[role],
        hasExplicitOverride: false,
        allow: [],
        deny: [],
        updatedAtUnix: null,
      };
    }
    return {
      role,
      label: LEGACY_OVERRIDE_ROLE_LABEL[role],
      hasExplicitOverride: false,
      allow: [],
      deny: [],
      updatedAtUnix: null,
    };
  });

  const member = entities.find((entry) => entry.role === "member");
  const others = entities
    .filter((entry) => entry.role !== "member")
    .sort((left, right) => {
      if (left.hasExplicitOverride !== right.hasExplicitOverride) {
        return left.hasExplicitOverride ? -1 : 1;
      }
      const leftUpdatedAt = left.updatedAtUnix ?? -1;
      const rightUpdatedAt = right.updatedAtUnix ?? -1;
      if (leftUpdatedAt !== rightUpdatedAt) {
        return rightUpdatedAt - leftUpdatedAt;
      }
      return LEGACY_OVERRIDE_ROLE_ORDER.indexOf(left.role)
        - LEGACY_OVERRIDE_ROLE_ORDER.indexOf(right.role);
    });

  return member ? [member, ...others] : others;
}

function buildChannelOverrideEffectivePermissions(options: {
  roles: ReturnType<
    ReturnType<typeof createWorkspaceState>["workspaceChannel"]["workspaceRolesByGuildId"]
  >[string] | undefined;
  overrides: ReturnType<
    ReturnType<typeof createWorkspaceState>["workspaceChannel"]["workspaceChannelOverridesByGuildId"]
  >[string][string] | undefined;
}): Record<RoleName, PermissionName[]> {
  const roles = options.roles ?? [];
  const overrides = options.overrides ?? [];
  return {
    member: permissionListFromBits(
      resolveEffectiveLegacyRolePermissions({
        role: "member",
        guildRoles: roles,
        channelOverrides: overrides,
      }),
    ),
    moderator: permissionListFromBits(
      resolveEffectiveLegacyRolePermissions({
        role: "moderator",
        guildRoles: roles,
        channelOverrides: overrides,
      }),
    ),
    owner: permissionListFromBits(
      resolveEffectiveLegacyRolePermissions({
        role: "owner",
        guildRoles: roles,
        channelOverrides: overrides,
      }),
    ),
  };
}

export function createCollaborationPanelHostStateOptions(
  options: CollaborationPanelHostStateOptions,
): CollaborationPanelPropGroupsStateOptions {
  const activeGuildId = options.workspaceChannelState.activeGuildId();
  const activeChannelId = options.workspaceChannelState.activeChannelId();
  const channelOverrides =
    activeGuildId && activeChannelId
      ? options.workspaceChannelState.workspaceChannelOverridesByGuildId()[activeGuildId]?.[
        activeChannelId
      ]
      : undefined;
  const workspaceRoles = activeGuildId
    ? options.workspaceChannelState.workspaceRolesByGuildId()[activeGuildId]
    : undefined;

  return {
    friendRecipientUserIdInput: options.friendshipsState.friendRecipientUserIdInput,
    friendRequests: options.friendshipsState.friendRequests(),
    friends: options.friendshipsState.friends(),
    isRunningFriendAction: options.friendshipsState.isRunningFriendAction,
    friendStatus: options.friendshipsState.friendStatus,
    friendError: options.friendshipsState.friendError,
    onSubmitFriendRequest: options.friendshipActions.submitFriendRequest,
    setFriendRecipientUserIdInput:
      options.friendshipsState.setFriendRecipientUserIdInput,
    onAcceptIncomingFriendRequest:
      options.friendshipActions.acceptIncomingFriendRequest,
    onDismissFriendRequest: options.friendshipActions.dismissFriendRequest,
    onRemoveFriendship: options.friendshipActions.removeFriendship,
    searchQuery: options.discoveryState.searchQuery,
    isSearching: options.discoveryState.isSearching,
    hasActiveWorkspace: () => Boolean(options.selectors.activeWorkspace()),
    canManageSearchMaintenance: options.selectors.canManageSearchMaintenance,
    isRunningSearchOps: options.discoveryState.isRunningSearchOps,
    searchOpsStatus: options.discoveryState.searchOpsStatus,
    searchError: options.discoveryState.searchError,
    searchResults: options.discoveryState.searchResults(),
    onSubmitSearch: options.searchActions.runSearch,
    setSearchQuery: options.discoveryState.setSearchQuery,
    onRebuildSearch: options.searchActions.rebuildSearch,
    onReconcileSearch: options.searchActions.reconcileSearch,
    displayUserLabel: options.labels.displayUserLabel,
    attachmentFilename: options.messageState.attachmentFilename,
    activeAttachments: options.selectors.activeAttachments(),
    isUploadingAttachment: options.messageState.isUploadingAttachment,
    hasActiveChannel: () => Boolean(options.selectors.activeChannel()),
    attachmentStatus: options.messageState.attachmentStatus,
    attachmentError: options.messageState.attachmentError,
    downloadingAttachmentId: options.messageState.downloadingAttachmentId(),
    deletingAttachmentId: options.messageState.deletingAttachmentId(),
    onSubmitUploadAttachment: options.attachmentActions.uploadAttachment,
    setSelectedAttachment: options.messageState.setSelectedAttachment,
    setAttachmentFilename: options.messageState.setAttachmentFilename,
    onDownloadAttachment: options.attachmentActions.downloadAttachment,
    onRemoveAttachment: options.attachmentActions.removeAttachment,
    moderationUserIdInput: options.diagnosticsState.moderationUserIdInput,
    moderationRoleInput: options.diagnosticsState.moderationRoleInput,
    overrideRoleInput: options.diagnosticsState.overrideRoleInput,
    overrideAllowCsv: options.diagnosticsState.overrideAllowCsv,
    overrideDenyCsv: options.diagnosticsState.overrideDenyCsv,
    channelOverrideEntities: buildChannelOverrideEntities({
      overrides: channelOverrides,
      selectedRole: options.diagnosticsState.overrideRoleInput(),
    }),
    channelOverrideEffectivePermissions: buildChannelOverrideEffectivePermissions({
      roles: workspaceRoles,
      overrides: channelOverrides,
    }),
    isModerating: options.diagnosticsState.isModerating,
    hasActiveModerationWorkspace: () =>
      Boolean(options.selectors.activeWorkspace()),
    hasActiveModerationChannel: () => Boolean(options.selectors.activeChannel()),
    canManageRoles: options.selectors.canManageRoles,
    canBanMembers: options.selectors.canBanMembers,
    canManageChannelOverrides: options.selectors.canManageChannelOverrides,
    moderationStatus: options.diagnosticsState.moderationStatus,
    moderationError: options.diagnosticsState.moderationError,
    setModerationUserIdInput: options.diagnosticsState.setModerationUserIdInput,
    setModerationRoleInput: options.diagnosticsState.setModerationRoleInput,
    onRunMemberAction: options.moderationActions.runMemberAction,
    setOverrideRoleInput: options.diagnosticsState.setOverrideRoleInput,
    setOverrideAllowCsv: options.diagnosticsState.setOverrideAllowCsv,
    setOverrideDenyCsv: options.diagnosticsState.setOverrideDenyCsv,
    onApplyOverride: options.moderationActions.applyOverride,
    onOpenRoleManagementPanel: () => options.openOverlayPanel("role-management"),
  };
}
