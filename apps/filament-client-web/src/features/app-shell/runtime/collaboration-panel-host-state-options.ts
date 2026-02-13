import type { createAttachmentController } from "../controllers/attachment-controller";
import type { createFriendshipController } from "../controllers/friendship-controller";
import type { createModerationController } from "../controllers/moderation-controller";
import type { createSearchController } from "../controllers/search-controller";
import type { CreateAppShellSelectorsResult } from "../selectors/create-app-shell-selectors";
import type { createDiagnosticsState } from "../state/diagnostics-state";
import type { createMessageState } from "../state/message-state";
import type { createWorkspaceState } from "../state/workspace-state";
import type { createAppShellRuntimeLabels } from "./runtime-labels";
import type { CollaborationPanelPropGroupsStateOptions } from "./collaboration-panel-prop-groups-options";

export interface CollaborationPanelHostStateOptions {
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

export function createCollaborationPanelHostStateOptions(
  options: CollaborationPanelHostStateOptions,
): CollaborationPanelPropGroupsStateOptions {
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
