import type { CollaborationPanelPropGroupsOptions } from "./collaboration-panel-prop-groups";
import type { PermissionName, RoleName } from "../../../domain/chat";

interface ModerationChannelOverrideEntityOption {
  role: CollaborationPanelPropGroupsOptions["moderation"]["overrideRoleInput"];
  label: string;
  hasExplicitOverride: boolean;
  allow: PermissionName[];
  deny: PermissionName[];
  updatedAtUnix: number | null;
}

type ModerationChannelOverrideEffectivePermissions = Record<RoleName, PermissionName[]>;

export interface CollaborationPanelPropGroupsStateOptions {
  friendRecipientUserIdInput: () => string;
  friendRequests: CollaborationPanelPropGroupsOptions["friendships"]["friendRequests"];
  friends: CollaborationPanelPropGroupsOptions["friendships"]["friends"];
  isRunningFriendAction: () => boolean;
  friendStatus: () => string;
  friendError: () => string;
  onSubmitFriendRequest:
    CollaborationPanelPropGroupsOptions["friendships"]["onSubmitFriendRequest"];
  setFriendRecipientUserIdInput:
    CollaborationPanelPropGroupsOptions["friendships"]["setFriendRecipientUserIdInput"];
  onAcceptIncomingFriendRequest:
    CollaborationPanelPropGroupsOptions["friendships"]["onAcceptIncomingFriendRequest"];
  onDismissFriendRequest:
    CollaborationPanelPropGroupsOptions["friendships"]["onDismissFriendRequest"];
  onRemoveFriendship:
    CollaborationPanelPropGroupsOptions["friendships"]["onRemoveFriendship"];
  searchQuery: () => string;
  isSearching: () => boolean;
  hasActiveWorkspace: () => boolean;
  canManageSearchMaintenance: () => boolean;
  isRunningSearchOps: () => boolean;
  searchOpsStatus: () => string;
  searchError: () => string;
  searchResults: CollaborationPanelPropGroupsOptions["search"]["searchResults"];
  onSubmitSearch: CollaborationPanelPropGroupsOptions["search"]["onSubmitSearch"];
  setSearchQuery: CollaborationPanelPropGroupsOptions["search"]["setSearchQuery"];
  onRebuildSearch: CollaborationPanelPropGroupsOptions["search"]["onRebuildSearch"];
  onReconcileSearch:
    CollaborationPanelPropGroupsOptions["search"]["onReconcileSearch"];
  displayUserLabel:
    CollaborationPanelPropGroupsOptions["search"]["displayUserLabel"];
  attachmentFilename: () => string;
  activeAttachments:
    CollaborationPanelPropGroupsOptions["attachments"]["activeAttachments"];
  isUploadingAttachment: () => boolean;
  hasActiveChannel: () => boolean;
  attachmentStatus: () => string;
  attachmentError: () => string;
  downloadingAttachmentId:
    CollaborationPanelPropGroupsOptions["attachments"]["downloadingAttachmentId"];
  deletingAttachmentId:
    CollaborationPanelPropGroupsOptions["attachments"]["deletingAttachmentId"];
  onSubmitUploadAttachment:
    CollaborationPanelPropGroupsOptions["attachments"]["onSubmitUploadAttachment"];
  setSelectedAttachment:
    CollaborationPanelPropGroupsOptions["attachments"]["setSelectedAttachment"];
  setAttachmentFilename:
    CollaborationPanelPropGroupsOptions["attachments"]["setAttachmentFilename"];
  onDownloadAttachment:
    CollaborationPanelPropGroupsOptions["attachments"]["onDownloadAttachment"];
  onRemoveAttachment:
    CollaborationPanelPropGroupsOptions["attachments"]["onRemoveAttachment"];
  moderationUserIdInput: () => string;
  moderationRoleInput:
    () => CollaborationPanelPropGroupsOptions["moderation"]["moderationRoleInput"];
  overrideRoleInput:
    () => CollaborationPanelPropGroupsOptions["moderation"]["overrideRoleInput"];
  overrideAllowCsv: () => string;
  overrideDenyCsv: () => string;
  channelOverrideEntities: ModerationChannelOverrideEntityOption[];
  channelOverrideEffectivePermissions: ModerationChannelOverrideEffectivePermissions;
  isModerating: () => boolean;
  hasActiveModerationWorkspace: () => boolean;
  hasActiveModerationChannel: () => boolean;
  canManageRoles: () => boolean;
  canBanMembers: () => boolean;
  canManageChannelOverrides: () => boolean;
  moderationStatus: () => string;
  moderationError: () => string;
  setModerationUserIdInput:
    CollaborationPanelPropGroupsOptions["moderation"]["setModerationUserIdInput"];
  setModerationRoleInput:
    CollaborationPanelPropGroupsOptions["moderation"]["setModerationRoleInput"];
  onRunMemberAction:
    CollaborationPanelPropGroupsOptions["moderation"]["onRunMemberAction"];
  setOverrideRoleInput:
    CollaborationPanelPropGroupsOptions["moderation"]["setOverrideRoleInput"];
  setOverrideAllowCsv:
    CollaborationPanelPropGroupsOptions["moderation"]["setOverrideAllowCsv"];
  setOverrideDenyCsv:
    CollaborationPanelPropGroupsOptions["moderation"]["setOverrideDenyCsv"];
  onApplyOverride:
    CollaborationPanelPropGroupsOptions["moderation"]["onApplyOverride"];
  onOpenRoleManagementPanel:
    CollaborationPanelPropGroupsOptions["moderation"]["onOpenRoleManagementPanel"];
}

export function createCollaborationPanelPropGroupsOptions(
  options: CollaborationPanelPropGroupsStateOptions,
): CollaborationPanelPropGroupsOptions {
  return {
    friendships: {
      friendRecipientUserIdInput: options.friendRecipientUserIdInput(),
      friendRequests: options.friendRequests,
      friends: options.friends,
      isRunningFriendAction: options.isRunningFriendAction(),
      friendStatus: options.friendStatus(),
      friendError: options.friendError(),
      onSubmitFriendRequest: options.onSubmitFriendRequest,
      setFriendRecipientUserIdInput: options.setFriendRecipientUserIdInput,
      onAcceptIncomingFriendRequest: options.onAcceptIncomingFriendRequest,
      onDismissFriendRequest: options.onDismissFriendRequest,
      onRemoveFriendship: options.onRemoveFriendship,
    },
    search: {
      searchQuery: options.searchQuery(),
      isSearching: options.isSearching(),
      hasActiveWorkspace: options.hasActiveWorkspace(),
      canManageSearchMaintenance: options.canManageSearchMaintenance(),
      isRunningSearchOps: options.isRunningSearchOps(),
      searchOpsStatus: options.searchOpsStatus(),
      searchError: options.searchError(),
      searchResults: options.searchResults,
      onSubmitSearch: options.onSubmitSearch,
      setSearchQuery: options.setSearchQuery,
      onRebuildSearch: options.onRebuildSearch,
      onReconcileSearch: options.onReconcileSearch,
      displayUserLabel: options.displayUserLabel,
    },
    attachments: {
      attachmentFilename: options.attachmentFilename(),
      activeAttachments: options.activeAttachments,
      isUploadingAttachment: options.isUploadingAttachment(),
      hasActiveChannel: options.hasActiveChannel(),
      attachmentStatus: options.attachmentStatus(),
      attachmentError: options.attachmentError(),
      downloadingAttachmentId: options.downloadingAttachmentId,
      deletingAttachmentId: options.deletingAttachmentId,
      onSubmitUploadAttachment: options.onSubmitUploadAttachment,
      setSelectedAttachment: options.setSelectedAttachment,
      setAttachmentFilename: options.setAttachmentFilename,
      onDownloadAttachment: options.onDownloadAttachment,
      onRemoveAttachment: options.onRemoveAttachment,
    },
    moderation: {
      moderationUserIdInput: options.moderationUserIdInput(),
      moderationRoleInput: options.moderationRoleInput(),
      overrideRoleInput: options.overrideRoleInput(),
      overrideAllowCsv: options.overrideAllowCsv(),
      overrideDenyCsv: options.overrideDenyCsv(),
      channelOverrideEntities: options.channelOverrideEntities,
      channelOverrideEffectivePermissions: options.channelOverrideEffectivePermissions,
      isModerating: options.isModerating(),
      hasActiveWorkspace: options.hasActiveModerationWorkspace(),
      hasActiveChannel: options.hasActiveModerationChannel(),
      canManageRoles: options.canManageRoles(),
      canBanMembers: options.canBanMembers(),
      canManageChannelOverrides: options.canManageChannelOverrides(),
      moderationStatus: options.moderationStatus(),
      moderationError: options.moderationError(),
      setModerationUserIdInput: options.setModerationUserIdInput,
      setModerationRoleInput: options.setModerationRoleInput,
      onRunMemberAction: options.onRunMemberAction,
      setOverrideRoleInput: options.setOverrideRoleInput,
      setOverrideAllowCsv: options.setOverrideAllowCsv,
      setOverrideDenyCsv: options.setOverrideDenyCsv,
      onApplyOverride: options.onApplyOverride,
      onOpenRoleManagementPanel: options.onOpenRoleManagementPanel,
    },
  };
}
