import { describe, expect, it, vi } from "vitest";
import {
  attachmentFromResponse,
  attachmentIdFromInput,
  channelIdFromInput,
  friendListFromResponse,
  friendRequestListFromResponse,
  guildIdFromInput,
  userIdFromInput,
} from "../src/domain/chat";
import { createCollaborationPanelPropGroups } from "../src/features/app-shell/runtime/collaboration-panel-prop-groups";

describe("app shell collaboration panel prop groups", () => {
  it("maps friendship/search/attachment/moderation props and handlers", async () => {
    const onSubmitFriendRequest = vi.fn();
    const setFriendRecipientUserIdInput = vi.fn();
    const onAcceptIncomingFriendRequest = vi.fn();
    const onDismissFriendRequest = vi.fn();
    const onRemoveFriendship = vi.fn();

    const onSubmitSearch = vi.fn();
    const setSearchQuery = vi.fn();
    const onRebuildSearch = vi.fn();
    const onReconcileSearch = vi.fn();

    const onSubmitUploadAttachment = vi.fn();
    const setSelectedAttachment = vi.fn();
    const setAttachmentFilename = vi.fn();
    const onDownloadAttachment = vi.fn();
    const onRemoveAttachment = vi.fn();

    const setModerationUserIdInput = vi.fn();
    const setModerationRoleInput = vi.fn();
    const onRunMemberAction = vi.fn();
    const setOverrideRoleInput = vi.fn();
    const setOverrideAllowCsv = vi.fn();
    const setOverrideDenyCsv = vi.fn();
    const onApplyOverride = vi.fn();
    const onOpenRoleManagementPanel = vi.fn();

    const attachment = attachmentFromResponse({
      attachment_id: "01ARZ3NDEKTSV4RRFFQ69G5FA1",
      guild_id: guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FA2"),
      channel_id: channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FA3"),
      owner_id: "01ARZ3NDEKTSV4RRFFQ69G5FA4",
      filename: "ops.log",
      mime_type: "text/plain",
      size_bytes: 32,
      sha256_hex: "a".repeat(64),
    });

    const propGroups = createCollaborationPanelPropGroups({
      friendships: {
        friendRecipientUserIdInput: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
        friendRequests: friendRequestListFromResponse({ incoming: [], outgoing: [] }),
        friends: friendListFromResponse({
          friends: [
            {
              user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
              username: "filament",
              created_at_unix: 10,
            },
          ],
        }),
        isRunningFriendAction: false,
        friendStatus: "ready",
        friendError: "",
        onSubmitFriendRequest,
        setFriendRecipientUserIdInput,
        onAcceptIncomingFriendRequest,
        onDismissFriendRequest,
        onRemoveFriendship,
      },
      search: {
        searchQuery: "incident",
        isSearching: false,
        hasActiveWorkspace: true,
        canManageSearchMaintenance: true,
        isRunningSearchOps: false,
        searchOpsStatus: "idle",
        searchError: "",
        searchResults: null,
        onSubmitSearch,
        setSearchQuery,
        onRebuildSearch,
        onReconcileSearch,
        displayUserLabel: (userId) => `@${userId}`,
      },
      attachments: {
        attachmentFilename: "ops.log",
        activeAttachments: [attachment],
        isUploadingAttachment: false,
        hasActiveChannel: true,
        attachmentStatus: "ready",
        attachmentError: "",
        downloadingAttachmentId: attachmentIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FA5"),
        deletingAttachmentId: null,
        onSubmitUploadAttachment,
        setSelectedAttachment,
        setAttachmentFilename,
        onDownloadAttachment,
        onRemoveAttachment,
      },
      moderation: {
        moderationUserIdInput: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
        moderationRoleInput: "member",
        overrideRoleInput: "moderator",
        overrideAllowCsv: "create_message",
        overrideDenyCsv: "delete_message",
        channelOverrideEntities: [
          {
            role: "member",
            label: "@everyone",
            hasExplicitOverride: true,
            allow: ["create_message"],
            deny: [],
            updatedAtUnix: 1,
          },
        ],
        isModerating: false,
        hasActiveWorkspace: true,
        hasActiveChannel: true,
        canManageRoles: true,
        canBanMembers: true,
        canManageChannelOverrides: true,
        moderationStatus: "ready",
        moderationError: "",
        setModerationUserIdInput,
        setModerationRoleInput,
        onRunMemberAction,
        setOverrideRoleInput,
        setOverrideAllowCsv,
        setOverrideDenyCsv,
        onApplyOverride,
        onOpenRoleManagementPanel,
      },
    });

    expect(propGroups.friendships.friends).toHaveLength(1);
    expect(propGroups.search.searchQuery).toBe("incident");
    expect(propGroups.attachments.activeAttachments).toHaveLength(1);
    expect(propGroups.moderation.canManageRoles).toBe(true);
    expect(propGroups.moderation.channelOverrideEntities).toHaveLength(1);

    const submitEvent = { preventDefault: vi.fn() } as unknown as SubmitEvent;

    await propGroups.friendships.onSubmitFriendRequest(submitEvent);
    await propGroups.search.onSubmitSearch(submitEvent);
    await propGroups.attachments.onSubmitUploadAttachment(submitEvent);
    await propGroups.moderation.onApplyOverride(submitEvent);

    expect(onSubmitFriendRequest).toHaveBeenCalledWith(submitEvent);
    expect(onSubmitSearch).toHaveBeenCalledWith(submitEvent);
    expect(onSubmitUploadAttachment).toHaveBeenCalledWith(submitEvent);
    expect(onApplyOverride).toHaveBeenCalledWith(submitEvent);

    await propGroups.friendships.onAcceptIncomingFriendRequest("req-1");
    await propGroups.friendships.onDismissFriendRequest("req-2");
    await propGroups.friendships.onRemoveFriendship(
      userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAC"),
    );
    propGroups.search.setSearchQuery("alerts");
    await propGroups.search.onRebuildSearch();
    await propGroups.search.onReconcileSearch();
    propGroups.attachments.setAttachmentFilename("ops-2.log");
    await propGroups.attachments.onDownloadAttachment(attachment);
    await propGroups.attachments.onRemoveAttachment(attachment);
    propGroups.moderation.setModerationUserIdInput("01ARZ3NDEKTSV4RRFFQ69G5FAD");
    await propGroups.moderation.onRunMemberAction("ban");
    propGroups.moderation.onOpenRoleManagementPanel();

    expect(onAcceptIncomingFriendRequest).toHaveBeenCalledWith("req-1");
    expect(onDismissFriendRequest).toHaveBeenCalledWith("req-2");
    expect(onRemoveFriendship).toHaveBeenCalledTimes(1);
    expect(setSearchQuery).toHaveBeenCalledWith("alerts");
    expect(onRebuildSearch).toHaveBeenCalledOnce();
    expect(onReconcileSearch).toHaveBeenCalledOnce();
    expect(setAttachmentFilename).toHaveBeenCalledWith("ops-2.log");
    expect(onDownloadAttachment).toHaveBeenCalledWith(attachment);
    expect(onRemoveAttachment).toHaveBeenCalledWith(attachment);
    expect(setModerationUserIdInput).toHaveBeenCalledWith(
      "01ARZ3NDEKTSV4RRFFQ69G5FAD",
    );
    expect(onRunMemberAction).toHaveBeenCalledWith("ban");
    expect(onOpenRoleManagementPanel).toHaveBeenCalledOnce();
    expect(setFriendRecipientUserIdInput).not.toHaveBeenCalled();
    expect(setSelectedAttachment).not.toHaveBeenCalled();
    expect(setModerationRoleInput).not.toHaveBeenCalled();
    expect(setOverrideRoleInput).not.toHaveBeenCalled();
    expect(setOverrideAllowCsv).not.toHaveBeenCalled();
    expect(setOverrideDenyCsv).not.toHaveBeenCalled();
  });
});
