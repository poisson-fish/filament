import { describe, expect, it, vi } from "vitest";
import {
  guildIdFromInput,
  guildVisibilityFromInput,
  permissionFromInput,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
} from "../src/domain/chat";
import type { SettingsPanelBuilderOptions } from "../src/features/app-shell/adapters/panel-host-props";
import { createSupportPanelPropGroups } from "../src/features/app-shell/runtime/support-panel-prop-groups";
import {
  defaultVoiceDevicePreferences,
  mediaDeviceIdFromInput,
} from "../src/lib/voice-device-settings";

describe("app shell support panel prop groups", () => {
  it("maps support panel groups and preserves key handlers", async () => {
    const onSubmitPublicGuildSearch = vi.fn();
    const onJoinGuildFromDirectory = vi.fn();
    const onOpenSettingsCategory = vi.fn();
    const onSaveWorkspaceSettings = vi.fn();
    const onUnassignRole = vi.fn();
    const onRunHealthCheck = vi.fn();

    const roleId = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAH");

    const panelGroups = createSupportPanelPropGroups({
      publicDirectory: {
        publicGuildSearchQuery: "ops",
        isSearchingPublicGuilds: false,
        publicGuildSearchError: "",
        publicGuildDirectory: [],
        publicGuildJoinStatusByGuildId: {},
        publicGuildJoinErrorByGuildId: {},
        onSubmitPublicGuildSearch,
        onJoinGuildFromDirectory,
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
        profile: {
          userId: "01ARZ3NDEKTSV4RRFFQ69G5FAJ",
        } as unknown as NonNullable<SettingsPanelBuilderOptions["profile"]>,
        profileDraftUsername: "filament",
        profileDraftAbout: "ops",
        selectedAvatarFilename: "avatar.png",
        isSavingProfile: false,
        isUploadingProfileAvatar: false,
        profileSettingsStatus: "ready",
        profileSettingsError: "",
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
        onUnassignRole,
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
        onRunHealthCheck,
        onRunEcho: () => undefined,
      },
    });

    expect(panelGroups.publicDirectory.publicGuildSearchQuery).toBe("ops");
    expect(panelGroups.settings.activeSettingsCategory).toBe("profile");
    expect(panelGroups.workspaceSettings.workspaceName).toBe("Ops");
    expect(panelGroups.workspaceSettings.workspaceSettingsSection).toBe("profile");
    expect(panelGroups.roleManagement.roles).toHaveLength(1);
    expect(panelGroups.utility.echoInput).toBe("ping");

    panelGroups.settings.onOpenSettingsCategory("voice");
    expect(onOpenSettingsCategory).toHaveBeenCalledWith("voice");

    await panelGroups.workspaceSettings.onSaveWorkspaceSettings();
    expect(onSaveWorkspaceSettings).toHaveBeenCalledOnce();

    await panelGroups.roleManagement.onUnassignRole(
      "01ARZ3NDEKTSV4RRFFQ69G5FAL",
      roleId,
    );
    expect(onUnassignRole).toHaveBeenCalledWith(
      "01ARZ3NDEKTSV4RRFFQ69G5FAL",
      roleId,
    );

    await panelGroups.utility.onRunHealthCheck();
    expect(onRunHealthCheck).toHaveBeenCalledOnce();

    const submitEvent = { preventDefault: vi.fn() } as unknown as SubmitEvent;
    await panelGroups.publicDirectory.onSubmitPublicGuildSearch(submitEvent);
    expect(onSubmitPublicGuildSearch).toHaveBeenCalledWith(submitEvent);

    await panelGroups.publicDirectory.onJoinGuildFromDirectory(
      guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAM"),
    );
    expect(onJoinGuildFromDirectory).toHaveBeenCalledWith(
      guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAM"),
    );
  });
});
