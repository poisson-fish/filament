import { describe, expect, it, vi } from "vitest";
import { createSupportPanelHostStateOptions } from "../src/features/app-shell/runtime/support-panel-host-state-options";

describe("app shell support panel-host state options", () => {
  it("maps support panel-host accessors and handlers", () => {
    const publicGuildSearchQuery = () => "ops";
    const isSearchingPublicGuilds = () => false;
    const publicGuildSearchError = () => "";
    const publicGuildDirectory = vi.fn(() => [{ guildId: "guild-1" }]);
    const publicGuildJoinStatusByGuildId = vi.fn(() => ({ "guild-1": "idle" }));
    const publicGuildJoinErrorByGuildId = vi.fn(() => ({}));
    const setPublicGuildSearchQuery = vi.fn();

    const activeSettingsCategory = () => "profile";
    const activeVoiceSettingsSubmenu = () => "audio-devices";
    const activeWorkspaceSettingsSection = () => "profile" as const;
    const setActiveVoiceSettingsSubmenu = vi.fn();

    const voiceDevicePreferences = () => ({
      audioinput: "in-1",
      audiooutput: "out-1",
    });
    const audioInputDevices = () => [{ deviceId: "in-1", label: "Mic" }];
    const audioOutputDevices = () => [{ deviceId: "out-1", label: "Speaker" }];
    const isRefreshingAudioDevices = () => false;
    const audioDevicesStatus = () => "ready";
    const audioDevicesError = () => "";

    const profile = vi.fn(() => ({ userId: "user-1" }));
    const profileDraftUsername = () => "filament";
    const profileDraftAbout = () => "ops";
    const selectedProfileAvatarFile = () => ({ name: "avatar.png" } as File);
    const isSavingProfile = () => false;
    const isUploadingProfileAvatar = () => false;
    const profileSettingsStatus = () => "ready";
    const profileSettingsError = () => "";
    const setProfileDraftUsername = vi.fn();
    const setProfileDraftAbout = vi.fn();
    const setSelectedProfileAvatarFile = vi.fn();
    const saveProfileSettings = vi.fn();
    const uploadProfileAvatar = vi.fn();
    const avatarUrlForUser = vi.fn(() => "/avatar/user-1");

    const activeWorkspace = vi.fn(() => ({ id: "guild-1" }));
    const activeGuildId = () => "guild-1";
    const onlineMembers = () => ["user-2"];
    const workspaceUserRolesByGuildId = () => ({
      "guild-1": {
        "user-1": ["role-1"],
        "user-2": [],
      },
    });
    const canManageRoles = () => true;
    const workspaceSettingsName = () => "Ops";
    const workspaceSettingsVisibility = () => "private";
    const isSavingWorkspaceSettings = () => false;
    const workspaceSettingsStatus = () => "ready";
    const workspaceSettingsError = () => "";
    const setWorkspaceSettingsName = vi.fn();
    const setWorkspaceSettingsVisibility = vi.fn();
    const setWorkspaceSettingsStatus = vi.fn();
    const setWorkspaceSettingsError = vi.fn();
    const viewAsRoleSimulatorEnabled = () => true;
    const viewAsRoleSimulatorRole = () => "moderator";
    const setViewAsRoleSimulatorEnabled = vi.fn();
    const setViewAsRoleSimulatorRole = vi.fn();
    const workspaceMembersByGuildId = () => ({
      "guild-1": ["user-1", "user-2"],
    });
    const isLoadingWorkspaceMembers = () => false;
    const workspaceMembersError = () => "";

    const canManageWorkspaceRoles = () => true;
    const canManageMemberRoles = () => true;
    const roles = () => [{ roleId: "role-1", position: 10, isSystem: false }];
    const isLoadingRoles = () => false;
    const isMutatingRoles = () => false;
    const roleManagementStatus = () => "ready";
    const roleManagementError = () => "";
    const moderationUserIdInput = () => "user-2";
    const setModerationUserIdInput = vi.fn();
    const refreshRoles = vi.fn();
    const createRole = vi.fn();
    const updateRole = vi.fn();
    const deleteRole = vi.fn();
    const reorderRoles = vi.fn();
    const assignRoleToMember = vi.fn();
    const unassignRoleFromMember = vi.fn();

    const echoInput = () => "ping";
    const healthStatus = () => "ok";
    const diagError = () => "";
    const diagnosticsEventCounts = () => ({
      session_refresh_succeeded: 1,
      session_refresh_failed: 2,
      health_check_succeeded: 3,
      health_check_failed: 4,
      echo_succeeded: 5,
      echo_failed: 6,
      logout_requested: 7,
      gateway_connected: 8,
      gateway_disconnected: 9,
    });
    const isCheckingHealth = () => false;
    const isEchoing = () => false;
    const setEchoInput = vi.fn();

    const runPublicGuildSearch = vi.fn();
    const joinGuildFromDirectory = vi.fn();
    const openSettingsCategory = vi.fn();
    const setVoiceDevicePreference = vi.fn();
    const refreshAudioDeviceInventory = vi.fn(async () => undefined);
    const saveWorkspaceSettings = vi.fn(async () => undefined);
    const openOverlayPanel = vi.fn();
    const displayUserLabel = (userId: string) => `@${userId}`;
    const runHealthCheck = vi.fn();
    const runEcho = vi.fn();

    const stateOptions = createSupportPanelHostStateOptions({
      discoveryState: {
        publicGuildSearchQuery,
        isSearchingPublicGuilds,
        publicGuildSearchError,
        publicGuildDirectory,
        publicGuildJoinStatusByGuildId,
        publicGuildJoinErrorByGuildId,
        setPublicGuildSearchQuery,
      } as unknown as Parameters<typeof createSupportPanelHostStateOptions>[0]["discoveryState"],
      overlayState: {
        activeSettingsCategory,
        activeVoiceSettingsSubmenu,
        activeWorkspaceSettingsSection,
        setActiveVoiceSettingsSubmenu,
      } as unknown as Parameters<typeof createSupportPanelHostStateOptions>[0]["overlayState"],
      voiceState: {
        voiceDevicePreferences,
        audioInputDevices,
        audioOutputDevices,
        isRefreshingAudioDevices,
        audioDevicesStatus,
        audioDevicesError,
      } as unknown as Parameters<typeof createSupportPanelHostStateOptions>[0]["voiceState"],
      profileState: {
        onlineMembers,
        profileDraftUsername,
        profileDraftAbout,
        selectedProfileAvatarFile,
        isSavingProfile,
        isUploadingProfileAvatar,
        profileSettingsStatus,
        profileSettingsError,
        setProfileDraftUsername,
        setProfileDraftAbout,
        setSelectedProfileAvatarFile,
      } as unknown as Parameters<typeof createSupportPanelHostStateOptions>[0]["profileState"],
      workspaceChannelState: {
        activeGuildId,
        workspaceUserRolesByGuildId,
        workspaceSettingsName,
        workspaceSettingsVisibility,
        isSavingWorkspaceSettings,
        workspaceSettingsStatus,
        workspaceSettingsError,
        viewAsRoleSimulatorEnabled,
        viewAsRoleSimulatorRole,
        setWorkspaceSettingsName,
        setWorkspaceSettingsVisibility,
        setViewAsRoleSimulatorEnabled,
        setViewAsRoleSimulatorRole,
        setWorkspaceSettingsStatus,
        setWorkspaceSettingsError,
      } as unknown as Parameters<typeof createSupportPanelHostStateOptions>[0]["workspaceChannelState"],
      diagnosticsState: {
        moderationUserIdInput,
        setModerationUserIdInput,
        echoInput,
        healthStatus,
        diagError,
        diagnosticsEventCounts,
        isCheckingHealth,
        isEchoing,
        setEchoInput,
      } as unknown as Parameters<typeof createSupportPanelHostStateOptions>[0]["diagnosticsState"],
      selectors: {
        activeWorkspace,
        canManageRoles,
        canManageWorkspaceRoles,
        canManageMemberRoles,
      } as unknown as Parameters<typeof createSupportPanelHostStateOptions>[0]["selectors"],
      publicDirectoryActions: {
        runPublicGuildSearch,
        joinGuildFromDirectory,
      } as unknown as Parameters<typeof createSupportPanelHostStateOptions>[0]["publicDirectoryActions"],
      profileController: {
        profile,
        saveProfileSettings,
        uploadProfileAvatar,
        avatarUrlForUser,
      } as unknown as Parameters<typeof createSupportPanelHostStateOptions>[0]["profileController"],
      roleManagementActions: {
        roles,
        isLoadingRoles,
        isMutatingRoles,
        roleManagementStatus,
        roleManagementError,
        refreshRoles,
        createRole,
        updateRole,
        deleteRole,
        reorderRoles,
        assignRoleToMember,
        unassignRoleFromMember,
      } as unknown as Parameters<typeof createSupportPanelHostStateOptions>[0]["roleManagementActions"],
      sessionDiagnostics: {
        runHealthCheck,
        runEcho,
      } as unknown as Parameters<typeof createSupportPanelHostStateOptions>[0]["sessionDiagnostics"],
      openSettingsCategory,
      setVoiceDevicePreference,
      refreshAudioDeviceInventory,
      saveWorkspaceSettings,
      openOverlayPanel,
      displayUserLabel,
      workspaceMembersByGuildId,
      isLoadingWorkspaceMembers,
      workspaceMembersError,
      isDevelopmentMode: true,
    });

    expect(stateOptions.publicGuildSearchQuery).toBe(publicGuildSearchQuery);
    expect(stateOptions.publicGuildDirectory).toEqual(publicGuildDirectory());
    expect(stateOptions.activeSettingsCategory).toBe(activeSettingsCategory);
    expect(stateOptions.profile()).toEqual(profile());
    expect(stateOptions.selectedAvatarFilename()).toBe("avatar.png");
    expect(stateOptions.workspaceName).toBe(workspaceSettingsName);
    expect(stateOptions.workspaceSettingsSection()).toBe("profile");
    expect(stateOptions.viewAsRoleSimulatorEnabled()).toBe(true);
    expect(stateOptions.viewAsRoleSimulatorRole()).toBe("moderator");
    expect(stateOptions.members()).toEqual([
      { userId: "user-1", label: "@user-1", roleIds: ["role-1"] },
      { userId: "user-2", label: "@user-2", roleIds: [] },
    ]);
    expect(stateOptions.assignableRoleIds()).toEqual([]);
    expect(stateOptions.roles).toBe(roles);
    expect(stateOptions.echoInput).toBe(echoInput);
    expect(stateOptions.diagnosticsEventCounts).toBe(diagnosticsEventCounts);
    expect(stateOptions.showDiagnosticsCounters).toBe(true);

    stateOptions.onOpenModerationPanel();
    expect(openOverlayPanel).toHaveBeenCalledWith("moderation");
  });
});
