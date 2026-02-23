import { describe, expect, it, vi } from "vitest";
import {
  attachmentFromResponse,
  channelIdFromInput,
  channelKindFromInput,
  friendListFromResponse,
  friendRequestListFromResponse,
  guildIdFromInput,
  guildVisibilityFromInput,
  permissionFromInput,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
} from "../src/domain/chat";
import { createPanelHostPropGroups } from "../src/features/app-shell/runtime/panel-host-prop-groups";
import {
  defaultVoiceDevicePreferences,
  mediaDeviceIdFromInput,
} from "../src/lib/voice-device-settings";

describe("app shell panel host prop groups", () => {
  it("composes workspace/support/collaboration groups into panel-host props", async () => {
    const onCreateWorkspaceSubmit = vi.fn();
    const onSaveWorkspaceSettings = vi.fn();
    const onSubmitFriendRequest = vi.fn();

    const roleId = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAH");
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

    const panelProps = createPanelHostPropGroups({
      workspaceChannelCreate: {
        workspaceCreate: {
          createGuildName: "Ops",
          createGuildVisibility: guildVisibilityFromInput("private"),
          createChannelName: "alerts",
          createChannelKind: channelKindFromInput("text"),
          isCreatingWorkspace: false,
          canDismissWorkspaceCreateForm: true,
          workspaceError: "",
          onCreateWorkspaceSubmit,
          setCreateGuildName: () => undefined,
          setCreateGuildVisibility: () => undefined,
          setCreateChannelName: () => undefined,
          setCreateChannelKind: () => undefined,
          onCancelWorkspaceCreate: () => undefined,
        },
        channelCreate: {
          newChannelName: "ops-voice",
          newChannelKind: channelKindFromInput("voice"),
          isCreatingChannel: false,
          channelCreateError: "",
          onCreateChannelSubmit: () => undefined,
          setNewChannelName: () => undefined,
          setNewChannelKind: () => undefined,
          onCancelChannelCreate: () => undefined,
        },
      },
      support: {
        publicDirectory: {
          publicGuildSearchQuery: "ops",
          isSearchingPublicGuilds: false,
          publicGuildSearchError: "",
          publicGuildDirectory: [],
          publicGuildJoinStatusByGuildId: {},
          publicGuildJoinErrorByGuildId: {},
          onSubmitPublicGuildSearch: () => undefined,
          onJoinGuildFromDirectory: () => undefined,
          setPublicGuildSearchQuery: () => undefined,
        },
        settings: {
          activeSettingsCategory: "profile",
          activeVoiceSettingsSubmenu: "audio-devices",
          voiceDevicePreferences: defaultVoiceDevicePreferences(),
          audioInputDevices: [
            {
              deviceId: mediaDeviceIdFromInput("input-1"),
              label: "Mic",
              kind: "audioinput",
            },
          ],
          audioOutputDevices: [
            {
              deviceId: mediaDeviceIdFromInput("output-1"),
              label: "Speaker",
              kind: "audiooutput",
            },
          ],
          isRefreshingAudioDevices: false,
          audioDevicesStatus: "ready",
          audioDevicesError: "",
          profile: null,
          profileDraftUsername: "filament",
          profileDraftAbout: "ops",
          selectedAvatarFilename: "avatar.png",
          isSavingProfile: false,
          isUploadingProfileAvatar: false,
          profileSettingsStatus: "ready",
          profileSettingsError: "",
          onOpenSettingsCategory: () => undefined,
          onOpenVoiceSettingsSubmenu: () => undefined,
          onSetVoiceDevicePreference: () => undefined,
          onRefreshAudioDeviceInventory: () => undefined,
          setProfileDraftUsername: () => undefined,
          setProfileDraftAbout: () => undefined,
          setSelectedProfileAvatarFile: () => undefined,
          onSaveProfileSettings: () => undefined,
          onUploadProfileAvatar: () => undefined,
          avatarUrlForUser: (userId) => `/avatar/${userId}`,
        },
        workspaceSettings: {
          hasActiveWorkspace: true,
          canManageWorkspaceSettings: true,
          canManageMemberRoles: true,
          workspaceSettingsSection: "profile",
          workspaceName: "Ops",
          workspaceVisibility: guildVisibilityFromInput("private"),
          isSavingWorkspaceSettings: false,
          workspaceSettingsStatus: "ready",
          workspaceSettingsError: "",
          memberRoleStatus: "ready",
          memberRoleError: "",
          isMutatingMemberRoles: false,
          viewAsRoleSimulatorEnabled: false,
          viewAsRoleSimulatorRole: "member",
          members: [],
          roles: [],
          assignableRoleIds: [roleId],
          setWorkspaceSettingsName: () => undefined,
          setWorkspaceSettingsVisibility: () => undefined,
          setViewAsRoleSimulatorEnabled: () => undefined,
          setViewAsRoleSimulatorRole: () => undefined,
          setWorkspaceSettingsStatus: () => undefined,
          setWorkspaceSettingsError: () => undefined,
          onSaveWorkspaceSettings,
          onAssignMemberRole: () => undefined,
          onUnassignMemberRole: () => undefined,
        },
        roleManagement: {
          hasActiveWorkspace: true,
          canManageWorkspaceRoles: true,
          canManageMemberRoles: true,
          roles: [
            {
              roleId,
              name: workspaceRoleNameFromInput("Moderator"),
              position: 10,
              isSystem: false,
              permissions: [permissionFromInput("manage_workspace_roles")],
            },
          ],
          isLoadingRoles: false,
          isMutatingRoles: false,
          roleManagementStatus: "ready",
          roleManagementError: "",
          targetUserIdInput: "01ARZ3NDEKTSV4RRFFQ69G5FAK",
          setTargetUserIdInput: () => undefined,
          onRefreshRoles: () => undefined,
          onCreateRole: () => undefined,
          onUpdateRole: () => undefined,
          onDeleteRole: () => undefined,
          onReorderRoles: () => undefined,
          onAssignRole: () => undefined,
          onUnassignRole: () => undefined,
          onOpenModerationPanel: () => undefined,
        },
        utility: {
          echoInput: "ping",
          healthStatus: "ok",
          diagError: "",
          diagnosticsEventCounts: {
            session_refresh_succeeded: 0,
            session_refresh_failed: 0,
            health_check_succeeded: 0,
            health_check_failed: 0,
            echo_succeeded: 0,
            echo_failed: 0,
            logout_requested: 0,
            gateway_connected: 0,
            gateway_disconnected: 0,
          },
          showDiagnosticsCounters: false,
          isCheckingHealth: false,
          isEchoing: false,
          setEchoInput: () => undefined,
          onRunHealthCheck: () => undefined,
          onRunEcho: () => undefined,
        },
      },
      collaboration: {
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
          setFriendRecipientUserIdInput: () => undefined,
          onAcceptIncomingFriendRequest: () => undefined,
          onDismissFriendRequest: () => undefined,
          onRemoveFriendship: () => undefined,
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
          onSubmitSearch: () => undefined,
          setSearchQuery: () => undefined,
          onRebuildSearch: () => undefined,
          onReconcileSearch: () => undefined,
          displayUserLabel: (userId) => `@${userId}`,
        },
        attachments: {
          attachmentFilename: "ops.log",
          activeAttachments: [attachment],
          isUploadingAttachment: false,
          hasActiveChannel: true,
          attachmentStatus: "ready",
          attachmentError: "",
          downloadingAttachmentId: null,
          deletingAttachmentId: null,
          onSubmitUploadAttachment: () => undefined,
          setSelectedAttachment: () => undefined,
          setAttachmentFilename: () => undefined,
          onDownloadAttachment: () => undefined,
          onRemoveAttachment: () => undefined,
        },
        moderation: {
          moderationUserIdInput: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
          moderationRoleInput: "member",
          overrideRoleInput: "moderator",
          overrideAllowCsv: "create_message",
          overrideDenyCsv: "delete_message",
          channelOverrideEffectivePermissions: {
            member: ["create_message"],
            moderator: ["create_message", "delete_message"],
            owner: ["manage_roles"],
          },
          isModerating: false,
          hasActiveWorkspace: true,
          hasActiveChannel: true,
          canManageRoles: true,
          canBanMembers: true,
          canManageChannelOverrides: true,
          moderationStatus: "ready",
          moderationError: "",
          setModerationUserIdInput: () => undefined,
          setModerationRoleInput: () => undefined,
          onRunMemberAction: () => undefined,
          setOverrideRoleInput: () => undefined,
          setOverrideAllowCsv: () => undefined,
          setOverrideDenyCsv: () => undefined,
          onApplyOverride: () => undefined,
          onOpenRoleManagementPanel: () => undefined,
        },
      },
    });

    expect(panelProps.workspaceCreatePanelProps.createGuildName).toBe("Ops");
    expect(panelProps.workspaceSettingsPanelProps.workspaceName).toBe("Ops");
    expect(panelProps.friendshipsPanelProps.friends).toHaveLength(1);
    expect(panelProps.attachmentsPanelProps.activeAttachments).toHaveLength(1);

    const submitEvent = { preventDefault: vi.fn() } as unknown as SubmitEvent;

    await panelProps.workspaceCreatePanelProps.onSubmit(submitEvent);
    await panelProps.workspaceSettingsPanelProps.onSaveWorkspaceSettings();
    await panelProps.friendshipsPanelProps.onSubmitFriendRequest(submitEvent);

    expect(onCreateWorkspaceSubmit).toHaveBeenCalledWith(submitEvent);
    expect(onSaveWorkspaceSettings).toHaveBeenCalledOnce();
    expect(onSubmitFriendRequest).toHaveBeenCalledWith(submitEvent);
  });
});
