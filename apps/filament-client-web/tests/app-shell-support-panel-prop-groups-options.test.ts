import { describe, expect, it, vi } from "vitest";
import {
  guildIdFromInput,
  guildVisibilityFromInput,
  permissionFromInput,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
} from "../src/domain/chat";
import type { SettingsPanelBuilderOptions } from "../src/features/app-shell/adapters/panel-host-props";
import { createSupportPanelPropGroupsOptions } from "../src/features/app-shell/runtime/support-panel-prop-groups-options";
import {
  defaultVoiceDevicePreferences,
  mediaDeviceIdFromInput,
} from "../src/lib/voice-device-settings";

describe("app shell support panel prop group state options", () => {
  it("maps runtime accessors and handlers into support panel group options", () => {
    const onSubmitPublicGuildSearch = vi.fn();
    const onJoinGuildFromDirectory = vi.fn();
    const onOpenSettingsCategory = vi.fn();
    const onRunHealthCheck = vi.fn();

    const roleId = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAH");

    const options = createSupportPanelPropGroupsOptions({
      publicGuildSearchQuery: () => "ops",
      isSearchingPublicGuilds: () => false,
      publicGuildSearchError: () => "",
      publicGuildDirectory: [],
      publicGuildJoinStatusByGuildId: {},
      publicGuildJoinErrorByGuildId: {},
      onSubmitPublicGuildSearch,
      onJoinGuildFromDirectory,
      setPublicGuildSearchQuery: () => undefined,
      activeSettingsCategory: () => "profile",
      activeVoiceSettingsSubmenu: () => "audio-devices",
      voiceDevicePreferences: () => defaultVoiceDevicePreferences(),
      audioInputDevices: () => [
        {
          deviceId: mediaDeviceIdFromInput("input-1"),
          label: "Mic",
          kind: "audioinput",
        },
      ],
      audioOutputDevices: () => [
        {
          deviceId: mediaDeviceIdFromInput("output-1"),
          label: "Speaker",
          kind: "audiooutput",
        },
      ],
      isRefreshingAudioDevices: () => false,
      audioDevicesStatus: () => "ready",
      audioDevicesError: () => "",
      profile: () =>
        ({
          userId: "01ARZ3NDEKTSV4RRFFQ69G5FAJ",
        }) as unknown as NonNullable<SettingsPanelBuilderOptions["profile"]>,
      profileDraftUsername: () => "filament",
      profileDraftAbout: () => "ops",
      selectedAvatarFilename: () => "avatar.png",
      isSavingProfile: () => false,
      isUploadingProfileAvatar: () => false,
      profileSettingsStatus: () => "ready",
      profileSettingsError: () => "",
      onOpenSettingsCategory,
      onOpenVoiceSettingsSubmenu: () => undefined,
      onSetVoiceDevicePreference: () => undefined,
      onRefreshAudioDeviceInventory: () => undefined,
      setProfileDraftUsername: () => undefined,
      setProfileDraftAbout: () => undefined,
      setSelectedProfileAvatarFile: () => undefined,
      onSaveProfileSettings: () => undefined,
      onUploadProfileAvatar: () => undefined,
      avatarUrlForUser: (userId) => `/avatar/${userId}`,
      hasActiveWorkspace: () => true,
      canManageWorkspaceSettings: () => true,
      workspaceName: () => "Ops",
      workspaceVisibility: () => guildVisibilityFromInput("private"),
      isSavingWorkspaceSettings: () => false,
      workspaceSettingsStatus: () => "ready",
      workspaceSettingsError: () => "",
      setWorkspaceSettingsName: () => undefined,
      setWorkspaceSettingsVisibility: () => undefined,
      setWorkspaceSettingsStatus: () => undefined,
      setWorkspaceSettingsError: () => undefined,
      onSaveWorkspaceSettings: () => undefined,
      canManageWorkspaceRoles: () => true,
      canManageMemberRoles: () => true,
      roles: () => [
        {
          roleId,
          name: workspaceRoleNameFromInput("Moderator"),
          position: 10,
          isSystem: false,
          permissions: [permissionFromInput("manage_workspace_roles")],
        },
      ],
      isLoadingRoles: () => false,
      isMutatingRoles: () => false,
      roleManagementStatus: () => "ready",
      roleManagementError: () => "",
      targetUserIdInput: () => "01ARZ3NDEKTSV4RRFFQ69G5FAK",
      setTargetUserIdInput: () => undefined,
      onRefreshRoles: () => undefined,
      onCreateRole: () => undefined,
      onUpdateRole: () => undefined,
      onDeleteRole: () => undefined,
      onReorderRoles: () => undefined,
      onAssignRole: () => undefined,
      onUnassignRole: () => undefined,
      onOpenModerationPanel: () => undefined,
      echoInput: () => "ping",
      healthStatus: () => "ok",
      diagError: () => "",
      isCheckingHealth: () => false,
      isEchoing: () => false,
      setEchoInput: () => undefined,
      onRunHealthCheck,
      onRunEcho: () => undefined,
    });

    expect(options.publicDirectory.publicGuildSearchQuery).toBe("ops");
    expect(options.settings.activeSettingsCategory).toBe("profile");
    expect(options.workspaceSettings.workspaceName).toBe("Ops");
    expect(options.roleManagement.roles).toHaveLength(1);
    expect(options.utility.echoInput).toBe("ping");

    const submitEvent = { preventDefault: vi.fn() } as unknown as SubmitEvent;
    void options.publicDirectory.onSubmitPublicGuildSearch(submitEvent);
    expect(onSubmitPublicGuildSearch).toHaveBeenCalledWith(submitEvent);

    void options.publicDirectory.onJoinGuildFromDirectory(
      guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAM"),
    );
    expect(onJoinGuildFromDirectory).toHaveBeenCalledWith(
      guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAM"),
    );

    options.settings.onOpenSettingsCategory("voice");
    expect(onOpenSettingsCategory).toHaveBeenCalledWith("voice");

    void options.utility.onRunHealthCheck();
    expect(onRunHealthCheck).toHaveBeenCalledOnce();
  });
});
