import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  attachmentIdFromInput,
  attachmentFilenameFromInput,
  channelIdFromInput,
  channelKindFromInput,
  guildIdFromInput,
  guildNameFromInput,
  roleFromInput,
  userIdFromInput,
} from "../src/domain/chat";
import {
  buildPanelHostPropGroups,
  type BuildPanelHostPropGroupsOptions,
} from "../src/features/app-shell/adapters/panel-host-props";
import { PanelHost } from "../src/features/app-shell/components/panels/PanelHost";

function baseOptions(
  overrides: Partial<BuildPanelHostPropGroupsOptions> = {},
): BuildPanelHostPropGroupsOptions {
  const defaultOptions: BuildPanelHostPropGroupsOptions = {
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

    newChannelName: "alerts",
    newChannelKind: channelKindFromInput("text"),
    isCreatingChannel: false,
    channelCreateError: "",
    onCreateChannelSubmit: vi.fn(),
    setNewChannelName: vi.fn(),
    setNewChannelKind: vi.fn(),
    onCancelChannelCreate: vi.fn(),

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
    onSubmitPublicGuildSearch: vi.fn(),
    setPublicGuildSearchQuery: vi.fn(),

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

    moderationUserIdInput: "",
    moderationRoleInput: roleFromInput("member"),
    overrideRoleInput: roleFromInput("member"),
    overrideAllowCsv: "",
    overrideDenyCsv: "",
    isModerating: false,
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

    echoInput: "",
    healthStatus: "",
    diagError: "",
    isCheckingHealth: false,
    isEchoing: false,
    setEchoInput: vi.fn(),
    onRunHealthCheck: vi.fn(),
    onRunEcho: vi.fn(),
  };

  return {
    ...defaultOptions,
    ...overrides,
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
        setCreateGuildName,
        setCreateGuildVisibility,
        setCreateChannelName,
        setCreateChannelKind,
        onCreateWorkspaceSubmit,
        onCancelWorkspaceCreate,
      }),
    );

    render(() => (
      <PanelHost
        panel="workspace-create"
        canCloseActivePanel={true}
        canManageWorkspaceChannels={true}
        canAccessActiveChannel={true}
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
    const setSelectedAttachment = vi.fn();
    const setAttachmentFilename = vi.fn();

    const propGroups = buildPanelHostPropGroups(
      baseOptions({
        setModerationRoleInput,
        setOverrideRoleInput,
        setSelectedAttachment,
        setAttachmentFilename,
      }),
    );

    propGroups.moderationPanelProps.onModerationRoleChange("moderator");
    propGroups.moderationPanelProps.onOverrideRoleChange("owner");

    const proofFile = new File(["proof"], "proof.png", { type: "image/png" });
    propGroups.attachmentsPanelProps.onAttachmentFileInput(proofFile);
    propGroups.attachmentsPanelProps.onAttachmentFileInput(null);

    expect(setModerationRoleInput).toHaveBeenCalledWith("moderator");
    expect(setOverrideRoleInput).toHaveBeenCalledWith("owner");
    expect(setSelectedAttachment).toHaveBeenNthCalledWith(1, proofFile);
    expect(setSelectedAttachment).toHaveBeenNthCalledWith(2, null);
    expect(setAttachmentFilename).toHaveBeenNthCalledWith(1, "proof.png");
    expect(setAttachmentFilename).toHaveBeenNthCalledWith(2, "");
  });
});
