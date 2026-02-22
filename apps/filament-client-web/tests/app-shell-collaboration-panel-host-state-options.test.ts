import { describe, expect, it, vi } from "vitest";
import { channelIdFromInput, guildIdFromInput } from "../src/domain/chat";
import { createCollaborationPanelHostStateOptions } from "../src/features/app-shell/runtime/collaboration-panel-host-state-options";

describe("app shell collaboration panel-host state options", () => {
  it("maps collaboration panel-host accessors and handlers", () => {
    const friendRecipientUserIdInput = () => "user-2";
    const friendRequests = () => [{ senderUserId: "user-3" }];
    const friends = () => [{ userId: "user-4" }];
    const isRunningFriendAction = () => false;
    const friendStatus = () => "ready";
    const friendError = () => "";
    const setFriendRecipientUserIdInput = vi.fn();

    const searchQuery = () => "incident";
    const isSearching = () => false;
    const isRunningSearchOps = () => true;
    const searchOpsStatus = () => "rebuilding";
    const searchError = () => "";
    const searchResults = () => [{ messageId: "m1" }];
    const setSearchQuery = vi.fn();

    const attachmentFilename = () => "proof.png";
    const isUploadingAttachment = () => false;
    const attachmentStatus = () => "idle";
    const attachmentError = () => "";
    const downloadingAttachmentId = () => "att-1";
    const deletingAttachmentId = () => "";
    const setSelectedAttachment = vi.fn();
    const setAttachmentFilename = vi.fn();

    const moderationUserIdInput = () => "user-9";
    const moderationRoleInput = () => "mod";
    const overrideRoleInput = () => "helper";
    const overrideAllowCsv = () => "send_messages";
    const overrideDenyCsv = () => "manage_roles";
    const isModerating = () => false;
    const moderationStatus = () => "idle";
    const moderationError = () => "";
    const setModerationUserIdInput = vi.fn();
    const setModerationRoleInput = vi.fn();
    const setOverrideRoleInput = vi.fn();
    const setOverrideAllowCsv = vi.fn();
    const setOverrideDenyCsv = vi.fn();

    const activeWorkspace = vi.fn(() => ({ id: "guild-1" }));
    const activeChannel = vi.fn(() => ({ id: "channel-1" }));
    const canManageSearchMaintenance = () => true;
    const activeAttachments = vi.fn(() => [{ id: "att-9" }]);
    const canManageRoles = () => true;
    const canBanMembers = () => true;
    const canManageChannelOverrides = () => false;

    const submitFriendRequest = vi.fn();
    const acceptIncomingFriendRequest = vi.fn();
    const dismissFriendRequest = vi.fn();
    const removeFriendship = vi.fn();

    const runSearch = vi.fn();
    const rebuildSearch = vi.fn();
    const reconcileSearch = vi.fn();
    const displayUserLabel = vi.fn(() => "user");

    const uploadAttachment = vi.fn();
    const downloadAttachment = vi.fn();
    const removeAttachment = vi.fn();

    const runMemberAction = vi.fn();
    const applyOverride = vi.fn();

    const openOverlayPanel = vi.fn();
    const activeGuildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAA");
    const activeChannelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAB");

    const stateOptions = createCollaborationPanelHostStateOptions({
      workspaceChannelState: {
        activeGuildId: () => activeGuildId,
        activeChannelId: () => activeChannelId,
        workspaceChannelOverridesByGuildId: () => ({
          [activeGuildId]: {
            [activeChannelId]: [
              {
                targetKind: "legacy_role",
                role: "moderator",
                allow: ["create_message"],
                deny: [],
                updatedAtUnix: 40,
              },
            ],
          },
        }),
      } as unknown as Parameters<
        typeof createCollaborationPanelHostStateOptions
      >[0]["workspaceChannelState"],
      friendshipsState: {
        friendRecipientUserIdInput,
        friendRequests,
        friends,
        isRunningFriendAction,
        friendStatus,
        friendError,
        setFriendRecipientUserIdInput,
      } as unknown as Parameters<
        typeof createCollaborationPanelHostStateOptions
      >[0]["friendshipsState"],
      discoveryState: {
        searchQuery,
        isSearching,
        isRunningSearchOps,
        searchOpsStatus,
        searchError,
        searchResults,
        setSearchQuery,
      } as unknown as Parameters<
        typeof createCollaborationPanelHostStateOptions
      >[0]["discoveryState"],
      messageState: {
        attachmentFilename,
        isUploadingAttachment,
        attachmentStatus,
        attachmentError,
        downloadingAttachmentId,
        deletingAttachmentId,
        setSelectedAttachment,
        setAttachmentFilename,
      } as unknown as Parameters<
        typeof createCollaborationPanelHostStateOptions
      >[0]["messageState"],
      diagnosticsState: {
        moderationUserIdInput,
        moderationRoleInput,
        overrideRoleInput,
        overrideAllowCsv,
        overrideDenyCsv,
        isModerating,
        moderationStatus,
        moderationError,
        setModerationUserIdInput,
        setModerationRoleInput,
        setOverrideRoleInput,
        setOverrideAllowCsv,
        setOverrideDenyCsv,
      } as unknown as Parameters<
        typeof createCollaborationPanelHostStateOptions
      >[0]["diagnosticsState"],
      selectors: {
        activeWorkspace,
        activeChannel,
        canManageSearchMaintenance,
        activeAttachments,
        canManageRoles,
        canBanMembers,
        canManageChannelOverrides,
      } as unknown as Parameters<
        typeof createCollaborationPanelHostStateOptions
      >[0]["selectors"],
      friendshipActions: {
        submitFriendRequest,
        acceptIncomingFriendRequest,
        dismissFriendRequest,
        removeFriendship,
      } as unknown as Parameters<
        typeof createCollaborationPanelHostStateOptions
      >[0]["friendshipActions"],
      searchActions: {
        runSearch,
        rebuildSearch,
        reconcileSearch,
      } as unknown as Parameters<
        typeof createCollaborationPanelHostStateOptions
      >[0]["searchActions"],
      attachmentActions: {
        uploadAttachment,
        downloadAttachment,
        removeAttachment,
      } as unknown as Parameters<
        typeof createCollaborationPanelHostStateOptions
      >[0]["attachmentActions"],
      moderationActions: {
        runMemberAction,
        applyOverride,
      } as unknown as Parameters<
        typeof createCollaborationPanelHostStateOptions
      >[0]["moderationActions"],
      labels: {
        displayUserLabel,
      } as unknown as Parameters<
        typeof createCollaborationPanelHostStateOptions
      >[0]["labels"],
      openOverlayPanel,
    });

    expect(stateOptions.friendRecipientUserIdInput).toBe(friendRecipientUserIdInput);
    expect(stateOptions.friendRequests).toEqual(friendRequests());
    expect(stateOptions.friends).toEqual(friends());
    expect(stateOptions.onSubmitFriendRequest).toBe(submitFriendRequest);
    expect(stateOptions.searchResults).toEqual(searchResults());
    expect(stateOptions.onRebuildSearch).toBe(rebuildSearch);
    expect(stateOptions.activeAttachments).toEqual(activeAttachments());
    expect(stateOptions.onSubmitUploadAttachment).toBe(uploadAttachment);
    expect(stateOptions.moderationRoleInput).toBe(moderationRoleInput);
    expect(stateOptions.channelOverrideEntities[0]?.role).toBe("member");
    expect(stateOptions.channelOverrideEntities[1]?.role).toBe("moderator");
    expect(stateOptions.onApplyOverride).toBe(applyOverride);

    stateOptions.onOpenRoleManagementPanel();
    expect(openOverlayPanel).toHaveBeenCalledWith("role-management");
  });
});
