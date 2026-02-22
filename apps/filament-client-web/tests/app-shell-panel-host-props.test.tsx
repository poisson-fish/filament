import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  attachmentIdFromInput,
  attachmentFilenameFromInput,
  channelIdFromInput,
  channelKindFromInput,
  guildIdFromInput,
  guildNameFromInput,
  permissionFromInput,
  roleFromInput,
  userIdFromInput,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
} from "../src/domain/chat";
import {
  buildAttachmentsPanelProps,
  buildModerationPanelProps,
  buildPanelHostPropGroups,
  buildRoleManagementPanelProps,
  type AttachmentsPanelBuilderOptions,
  type BuildPanelHostPropGroupsOptions,
  type ChannelCreatePanelBuilderOptions,
  type FriendshipsPanelBuilderOptions,
  type ModerationPanelBuilderOptions,
  type PublicDirectoryPanelBuilderOptions,
  type RoleManagementPanelBuilderOptions,
  type SearchPanelBuilderOptions,
  type SettingsPanelBuilderOptions,
  type UtilityPanelBuilderOptions,
  type WorkspaceSettingsPanelBuilderOptions,
  type WorkspaceCreatePanelBuilderOptions,
} from "../src/features/app-shell/adapters/panel-host-props";
import { PanelHost } from "../src/features/app-shell/components/panels/PanelHost";

interface PanelHostOptionsOverrides {
  workspaceCreate?: Partial<WorkspaceCreatePanelBuilderOptions>;
  channelCreate?: Partial<ChannelCreatePanelBuilderOptions>;
  publicDirectory?: Partial<PublicDirectoryPanelBuilderOptions>;
  settings?: Partial<SettingsPanelBuilderOptions>;
  workspaceSettings?: Partial<WorkspaceSettingsPanelBuilderOptions>;
  friendships?: Partial<FriendshipsPanelBuilderOptions>;
  search?: Partial<SearchPanelBuilderOptions>;
  attachments?: Partial<AttachmentsPanelBuilderOptions>;
  moderation?: Partial<ModerationPanelBuilderOptions>;
  roleManagement?: Partial<RoleManagementPanelBuilderOptions>;
  utility?: Partial<UtilityPanelBuilderOptions>;
}

function baseOptions(
  overrides: PanelHostOptionsOverrides = {},
): BuildPanelHostPropGroupsOptions {
  const defaults: BuildPanelHostPropGroupsOptions = {
    workspaceCreate: {
      createGuildName: "Security Ops",
      createGuildVisibility: "private",
      createChannelName: "incident-room",
      createChannelKind: channelKindFromInput("text"),
      isCreatingWorkspace: false,
      canDismissWorkspaceCreateForm: true,
      workspaceError: "",
      onCreateWorkspaceSubmit: vi.fn(),
      setCreateGuildName: vi.fn(),
      setCreateGuildVisibility: vi.fn(),
      setCreateChannelName: vi.fn(),
      setCreateChannelKind: vi.fn(),
      onCancelWorkspaceCreate: vi.fn(),
    },
    channelCreate: {
      newChannelName: "alerts",
      newChannelKind: channelKindFromInput("text"),
      isCreatingChannel: false,
      channelCreateError: "",
      onCreateChannelSubmit: vi.fn(),
      setNewChannelName: vi.fn(),
      setNewChannelKind: vi.fn(),
      onCancelChannelCreate: vi.fn(),
    },
    publicDirectory: {
      publicGuildSearchQuery: "",
      isSearchingPublicGuilds: false,
      publicGuildSearchError: "",
      publicGuildDirectory: [
        {
          guildId: guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW"),
          name: guildNameFromInput("Security Ops"),
          visibility: "public",
        },
      ],
      publicGuildJoinStatusByGuildId: {},
      publicGuildJoinErrorByGuildId: {},
      onSubmitPublicGuildSearch: vi.fn(),
      onJoinGuildFromDirectory: vi.fn(),
      setPublicGuildSearchQuery: vi.fn(),
    },
    settings: {
      activeSettingsCategory: "voice",
      activeVoiceSettingsSubmenu: "audio-devices",
      voiceDevicePreferences: {
        audioInputDeviceId: null,
        audioOutputDeviceId: null,
      },
      audioInputDevices: [],
      audioOutputDevices: [],
      isRefreshingAudioDevices: false,
      audioDevicesStatus: "",
      audioDevicesError: "",
      profile: null,
      profileDraftUsername: "",
      profileDraftAbout: "",
      profileAvatarUrl: null,
      selectedAvatarFilename: "",
      isSavingProfile: false,
      isUploadingProfileAvatar: false,
      profileSettingsStatus: "",
      profileSettingsError: "",
      onOpenSettingsCategory: vi.fn(),
      onOpenVoiceSettingsSubmenu: vi.fn(),
      onSetVoiceDevicePreference: vi.fn(),
      onRefreshAudioDeviceInventory: vi.fn(),
      setProfileDraftUsername: vi.fn(),
      setProfileDraftAbout: vi.fn(),
      setSelectedProfileAvatarFile: vi.fn(),
      onSaveProfileSettings: vi.fn(),
      onUploadProfileAvatar: vi.fn(),
    },
    workspaceSettings: {
      hasActiveWorkspace: true,
      canManageWorkspaceSettings: true,
      canManageMemberRoles: true,
      workspaceName: "Security Ops",
      workspaceVisibility: "private",
      isSavingWorkspaceSettings: false,
      workspaceSettingsStatus: "",
      workspaceSettingsError: "",
      memberRoleStatus: "",
      memberRoleError: "",
      isMutatingMemberRoles: false,
      members: [],
      roles: [],
      assignableRoleIds: [],
      setWorkspaceSettingsName: vi.fn(),
      setWorkspaceSettingsVisibility: vi.fn(),
      onSaveWorkspaceSettings: vi.fn(),
      onAssignMemberRole: vi.fn(),
      onUnassignMemberRole: vi.fn(),
    },
    friendships: {
      friendRecipientUserIdInput: "",
      friendRequests: { incoming: [], outgoing: [] },
      friends: [],
      isRunningFriendAction: false,
      friendStatus: "",
      friendError: "",
      onSubmitFriendRequest: vi.fn(),
      setFriendRecipientUserIdInput: vi.fn(),
      onAcceptIncomingFriendRequest: vi.fn(),
      onDismissFriendRequest: vi.fn(),
      onRemoveFriendship: vi.fn(),
    },
    search: {
      searchQuery: "",
      isSearching: false,
      hasActiveWorkspace: true,
      canManageSearchMaintenance: false,
      isRunningSearchOps: false,
      searchOpsStatus: "",
      searchError: "",
      searchResults: null,
      onSubmitSearch: vi.fn(),
      setSearchQuery: vi.fn(),
      onRebuildSearch: vi.fn(),
      onReconcileSearch: vi.fn(),
      displayUserLabel: (userId) => userId,
    },
    attachments: {
      attachmentFilename: "",
      activeAttachments: [
        {
          attachmentId: attachmentIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAZ"),
          guildId: guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW"),
          channelId: channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX"),
          ownerId: userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
          filename: attachmentFilenameFromInput("upload.png"),
          mimeType: "image/png",
          sizeBytes: 7,
          sha256Hex: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        },
      ],
      isUploadingAttachment: false,
      hasActiveChannel: true,
      attachmentStatus: "",
      attachmentError: "",
      downloadingAttachmentId: null,
      deletingAttachmentId: null,
      onSubmitUploadAttachment: vi.fn(),
      setSelectedAttachment: vi.fn(),
      setAttachmentFilename: vi.fn(),
      onDownloadAttachment: vi.fn(),
      onRemoveAttachment: vi.fn(),
    },
    moderation: {
      moderationUserIdInput: "",
      moderationRoleInput: roleFromInput("member"),
      overrideRoleInput: roleFromInput("member"),
      overrideAllowCsv: "",
      overrideDenyCsv: "",
      channelOverrideEntities: [],
      channelOverrideEffectivePermissions: {
        member: [],
        moderator: [],
        owner: [],
      },
      isModerating: false,
      hasActiveWorkspace: true,
      hasActiveChannel: true,
      canManageRoles: true,
      canBanMembers: true,
      canManageChannelOverrides: true,
      moderationStatus: "",
      moderationError: "",
      setModerationUserIdInput: vi.fn(),
      setModerationRoleInput: vi.fn(),
      onRunMemberAction: vi.fn(),
      setOverrideRoleInput: vi.fn(),
      setOverrideAllowCsv: vi.fn(),
      setOverrideDenyCsv: vi.fn(),
      onApplyOverride: vi.fn(),
      onOpenRoleManagementPanel: vi.fn(),
    },
    roleManagement: {
      hasActiveWorkspace: true,
      canManageWorkspaceRoles: true,
      canManageMemberRoles: true,
      roles: [
        {
          roleId: workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB1"),
          name: workspaceRoleNameFromInput("Responder"),
          position: 3,
          isSystem: false,
          permissions: [
            permissionFromInput("create_message"),
            permissionFromInput("subscribe_streams"),
          ],
        },
      ],
      isLoadingRoles: false,
      isMutatingRoles: false,
      roleManagementStatus: "",
      roleManagementError: "",
      targetUserIdInput: "",
      setTargetUserIdInput: vi.fn(),
      onRefreshRoles: vi.fn(),
      onCreateRole: vi.fn(),
      onUpdateRole: vi.fn(),
      onDeleteRole: vi.fn(),
      onReorderRoles: vi.fn(),
      onAssignRole: vi.fn(),
      onUnassignRole: vi.fn(),
      onOpenModerationPanel: vi.fn(),
    },
    utility: {
      echoInput: "",
      healthStatus: "",
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
      setEchoInput: vi.fn(),
      onRunHealthCheck: vi.fn(),
      onRunEcho: vi.fn(),
    },
  };

  return {
    workspaceCreate: {
      ...defaults.workspaceCreate,
      ...overrides.workspaceCreate,
    },
    channelCreate: {
      ...defaults.channelCreate,
      ...overrides.channelCreate,
    },
    publicDirectory: {
      ...defaults.publicDirectory,
      ...overrides.publicDirectory,
    },
    settings: {
      ...defaults.settings,
      ...overrides.settings,
    },
    workspaceSettings: {
      ...defaults.workspaceSettings,
      ...overrides.workspaceSettings,
    },
    friendships: {
      ...defaults.friendships,
      ...overrides.friendships,
    },
    search: {
      ...defaults.search,
      ...overrides.search,
    },
    attachments: {
      ...defaults.attachments,
      ...overrides.attachments,
    },
    moderation: {
      ...defaults.moderation,
      ...overrides.moderation,
    },
    roleManagement: {
      ...defaults.roleManagement,
      ...overrides.roleManagement,
    },
    utility: {
      ...defaults.utility,
      ...overrides.utility,
    },
  };
}

describe("app shell panel host props adapter", () => {
  it("keeps workspace panel interactions wired to typed handlers", () => {
    const setCreateGuildName = vi.fn();
    const setCreateGuildVisibility = vi.fn();
    const setCreateChannelName = vi.fn();
    const setCreateChannelKind = vi.fn();
    const onCreateWorkspaceSubmit = vi.fn();
    const onCancelWorkspaceCreate = vi.fn();

    const propGroups = buildPanelHostPropGroups(
      baseOptions({
        workspaceCreate: {
          setCreateGuildName,
          setCreateGuildVisibility,
          setCreateChannelName,
          setCreateChannelKind,
          onCreateWorkspaceSubmit,
          onCancelWorkspaceCreate,
        },
      }),
    );

    render(() => (
      <PanelHost
        panel="workspace-create"
        canCloseActivePanel={true}
        canManageWorkspaceChannels={true}
        canAccessActiveChannel={true}
        hasRoleManagementAccess={true}
        hasModerationAccess={true}
        panelTitle={() => "Workspace"}
        panelClassName={() => "panel-window"}
        onClose={vi.fn()}
        {...propGroups}
      />
    ));

    fireEvent.input(screen.getByLabelText("Workspace name"), {
      target: { value: "Blue Team" },
    });
    fireEvent.change(screen.getByLabelText("Visibility"), {
      target: { value: "public" },
    });
    fireEvent.input(screen.getByLabelText("First channel"), {
      target: { value: "ops" },
    });
    fireEvent.change(screen.getByLabelText("Channel type"), {
      target: { value: "voice" },
    });
    const workspaceForm = screen.getByLabelText("Workspace name").closest("form");
    expect(workspaceForm).not.toBeNull();
    fireEvent.submit(workspaceForm!);
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));

    expect(setCreateGuildName).toHaveBeenCalledWith("Blue Team");
    expect(setCreateGuildVisibility).toHaveBeenCalledWith("public");
    expect(setCreateChannelName).toHaveBeenCalledWith("ops");
    expect(setCreateChannelKind).toHaveBeenCalledWith("voice");
    expect(onCreateWorkspaceSubmit).toHaveBeenCalledTimes(1);
    expect(onCancelWorkspaceCreate).toHaveBeenCalledTimes(1);
  });

  it("keeps moderation role and attachment file callbacks mapped", () => {
    const setModerationRoleInput = vi.fn();
    const setOverrideRoleInput = vi.fn();
    const onOpenRoleManagementPanel = vi.fn();
    const setTargetUserIdInput = vi.fn();
    const setSelectedAttachment = vi.fn();
    const setAttachmentFilename = vi.fn();

    const options = baseOptions({
      moderation: {
        setModerationRoleInput,
        setOverrideRoleInput,
        onOpenRoleManagementPanel,
      },
      roleManagement: {
        setTargetUserIdInput,
      },
      attachments: {
        setSelectedAttachment,
        setAttachmentFilename,
      },
    });

    const moderationPanelProps = buildModerationPanelProps(options.moderation);
    const roleManagementPanelProps = buildRoleManagementPanelProps(options.roleManagement);
    const attachmentsPanelProps = buildAttachmentsPanelProps(options.attachments);

    moderationPanelProps.onModerationRoleChange("moderator");
    moderationPanelProps.onOverrideRoleChange("owner");
    moderationPanelProps.onOpenRoleManagementPanel();
    roleManagementPanelProps.onTargetUserIdInput("01ARZ3NDEKTSV4RRFFQ69G5FAA");

    const proofFile = new File(["proof"], "proof.png", { type: "image/png" });
    attachmentsPanelProps.onAttachmentFileInput(proofFile);
    attachmentsPanelProps.onAttachmentFileInput(null);

    expect(setModerationRoleInput).toHaveBeenCalledWith("moderator");
    expect(setOverrideRoleInput).toHaveBeenCalledWith("owner");
    expect(moderationPanelProps.channelOverrideEffectivePermissions).toEqual({
      member: [],
      moderator: [],
      owner: [],
    });
    expect(onOpenRoleManagementPanel).toHaveBeenCalledTimes(1);
    expect(setTargetUserIdInput).toHaveBeenCalledWith("01ARZ3NDEKTSV4RRFFQ69G5FAA");
    expect(setSelectedAttachment).toHaveBeenNthCalledWith(1, proofFile);
    expect(setSelectedAttachment).toHaveBeenNthCalledWith(2, null);
    expect(setAttachmentFilename).toHaveBeenNthCalledWith(1, "proof.png");
    expect(setAttachmentFilename).toHaveBeenNthCalledWith(2, "");
  });

  it("renders panel host shell with utility layout classes and stable panel hooks", () => {
    const propGroups = buildPanelHostPropGroups(baseOptions());

    const { container } = render(() => (
      <PanelHost
        panel="workspace-create"
        canCloseActivePanel={true}
        canManageWorkspaceChannels={true}
        canAccessActiveChannel={true}
        hasRoleManagementAccess={true}
        hasModerationAccess={true}
        panelTitle={() => "Workspace"}
        panelClassName={() => "panel-window panel-window-compact"}
        onClose={vi.fn()}
        {...propGroups}
      />
    ));

    const backdrop = container.querySelector(".panel-backdrop");
    expect(backdrop).not.toBeNull();
    expect(backdrop?.className).toContain("fixed");
    expect(backdrop?.className).toContain("inset-0");
    expect(backdrop?.className).toContain("bg-bg-0/44");
    expect(backdrop?.className).toContain("backdrop-blur-[5px]");

    const panelWindow = screen.getByRole("dialog", { name: "Workspace panel" });
    expect(panelWindow.className).toContain("panel-window");
    expect(panelWindow.className).toContain("panel-window-compact");
    expect(panelWindow.className).toContain("w-full");
    expect(panelWindow.className).toContain("md:w-[min(30rem,100%)]");
    expect(panelWindow.className).toContain("bg-bg-1");
    expect(panelWindow.className).toContain("border-line-soft");
    expect(panelWindow.className).not.toContain("bg-bg-1/90");
    expect(panelWindow.className).not.toContain("backdrop-blur-xl");

    const panelHeader = container.querySelector(".panel-window-header");
    expect(panelHeader).not.toBeNull();
    expect(panelHeader?.className).toContain("border-b");

    const panelBody = container.querySelector(".panel-window-body");
    expect(panelBody).not.toBeNull();
    expect(panelBody?.className).toContain("overflow-auto");
  });

  it("uses wide window sizing for client settings overlays", () => {
    const propGroups = buildPanelHostPropGroups(baseOptions());

    render(() => (
      <PanelHost
        panel="client-settings"
        canCloseActivePanel={true}
        canManageWorkspaceChannels={true}
        canAccessActiveChannel={true}
        hasRoleManagementAccess={true}
        hasModerationAccess={true}
        panelTitle={() => "Client settings"}
        panelClassName={() => "panel-window panel-window-wide"}
        onClose={vi.fn()}
        {...propGroups}
      />
    ));

    const panelWindow = screen.getByRole("dialog", { name: "Client settings panel" });
    expect(panelWindow.className).toContain("panel-window-wide");
    expect(panelWindow.className).toContain("md:w-[min(58rem,100%)]");
  });

  it("composes panel groups from panel-scoped builders without mapping drift", () => {
    const options = baseOptions();

    const propGroups = buildPanelHostPropGroups(options);

    expect(propGroups.workspaceCreatePanelProps.createGuildName).toBe(
      options.workspaceCreate.createGuildName,
    );
    expect(propGroups.channelCreatePanelProps.newChannelKind).toBe(
      options.channelCreate.newChannelKind,
    );
    expect(propGroups.publicDirectoryPanelProps.guilds).toBe(
      options.publicDirectory.publicGuildDirectory,
    );
    expect(propGroups.publicDirectoryPanelProps.joinStatusByGuildId).toBe(
      options.publicDirectory.publicGuildJoinStatusByGuildId,
    );
    expect(propGroups.settingsPanelProps.activeSettingsCategory).toBe(
      options.settings.activeSettingsCategory,
    );
    expect(propGroups.friendshipsPanelProps.friendRequests).toBe(
      options.friendships.friendRequests,
    );
    expect(propGroups.workspaceSettingsPanelProps.workspaceName).toBe(
      options.workspaceSettings.workspaceName,
    );
    expect(propGroups.searchPanelProps.searchResults).toBe(options.search.searchResults);
    expect(propGroups.attachmentsPanelProps.activeAttachments).toBe(
      options.attachments.activeAttachments,
    );
    expect(propGroups.moderationPanelProps.canManageRoles).toBe(
      options.moderation.canManageRoles,
    );
    expect(propGroups.roleManagementPanelProps.roles).toBe(options.roleManagement.roles);
    expect(propGroups.utilityPanelProps.echoInput).toBe(options.utility.echoInput);
  });
});
