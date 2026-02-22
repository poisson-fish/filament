import { beforeEach, describe, expect, it } from "vitest";
import {
  DEFAULT_SETTINGS_CATEGORY,
  DEFAULT_VOICE_SETTINGS_SUBMENU,
} from "../src/features/app-shell/config/settings-menu";
import { RTC_DISCONNECTED_SNAPSHOT } from "../src/features/app-shell/config/ui-constants";
import { createDiagnosticsState } from "../src/features/app-shell/state/diagnostics-state";
import { createMessageState } from "../src/features/app-shell/state/message-state";
import { createOverlayState } from "../src/features/app-shell/state/overlay-state";
import { createProfileState } from "../src/features/app-shell/state/profile-state";
import {
  createVoiceState,
  DEFAULT_VOICE_SESSION_CAPABILITIES,
} from "../src/features/app-shell/state/voice-state";
import { isMessageHistoryLoading } from "../src/features/app-shell/state/message-state";
import { createWorkspaceState } from "../src/features/app-shell/state/workspace-state";
import {
  channelIdFromInput,
  guildIdFromInput,
  userIdFromInput,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
} from "../src/domain/chat";
import { VOICE_DEVICE_SETTINGS_STORAGE_KEY } from "../src/lib/voice-device-settings";

beforeEach(() => {
  if (typeof window.localStorage?.setItem !== "function") {
    return;
  }
  window.localStorage.setItem(VOICE_DEVICE_SETTINGS_STORAGE_KEY, "invalid-json");
});

describe("app shell state factories", () => {
  it("provides workspace and message defaults", () => {
    const workspaceState = createWorkspaceState();
    const messageState = createMessageState();

    expect(workspaceState.workspaceChannel.workspaces()).toEqual([]);
    expect(workspaceState.workspaceChannel.activeGuildId()).toBeNull();
    expect(workspaceState.workspaceChannel.activeChannelId()).toBeNull();
    expect(workspaceState.workspaceChannel.workspaceBootstrapDone()).toBe(false);
    expect(workspaceState.workspaceChannel.createGuildName()).toBe("Security Ops");
    expect(workspaceState.workspaceChannel.createGuildVisibility()).toBe("private");
    expect(workspaceState.friendships.friendRequests()).toEqual({ incoming: [], outgoing: [] });
    expect(workspaceState.workspaceChannel.channelPermissions()).toBeNull();
    expect(workspaceState.workspaceChannel.workspaceRolesByGuildId()).toEqual({});
    expect(workspaceState.workspaceChannel.workspaceUserRolesByGuildId()).toEqual({});
    expect(workspaceState.workspaceChannel.workspaceChannelOverridesByGuildId()).toEqual({});
    expect(workspaceState.workspaceChannel.createChannelName()).toBe("incident-room");
    expect(workspaceState.workspaceChannel.workspaceSettingsName()).toBe("");
    expect(workspaceState.workspaceChannel.workspaceSettingsVisibility()).toBe("private");
    expect(workspaceState.workspaceChannel.isSavingWorkspaceSettings()).toBe(false);
    expect(workspaceState.workspaceChannel.workspaceSettingsStatus()).toBe("");
    expect(workspaceState.workspaceChannel.workspaceSettingsError()).toBe("");
    expect(workspaceState.workspaceChannel.viewAsRoleSimulatorEnabled()).toBe(false);
    expect(workspaceState.workspaceChannel.viewAsRoleSimulatorRole()).toBe("member");
    expect(workspaceState.friendships.friendRecipientUserIdInput()).toBe("");
    expect(workspaceState.discovery.publicGuildDirectory()).toEqual([]);
    expect(workspaceState.discovery.publicGuildJoinStatusByGuildId()).toEqual({});
    expect(workspaceState.discovery.publicGuildJoinErrorByGuildId()).toEqual({});
    expect(workspaceState.discovery.searchResults()).toBeNull();

    expect(messageState.composer()).toBe("");
    expect(messageState.messages()).toEqual([]);
    expect(messageState.nextBefore()).toBeNull();
    expect(messageState.showLoadOlderButton()).toBe(false);
    expect(messageState.sendMessageState()).toEqual({
      phase: "idle",
      statusMessage: "",
      errorMessage: "",
    });
    expect(messageState.isSendingMessage()).toBe(false);
    messageState.setSendMessageState({
      phase: "running",
      statusMessage: "",
      errorMessage: "",
    });
    expect(messageState.isSendingMessage()).toBe(true);
    expect(messageState.refreshMessagesState()).toEqual({
      phase: "idle",
      statusMessage: "",
      errorMessage: "",
    });
    expect(messageState.messageHistoryLoadTarget()).toBeNull();
    expect(messageState.isLoadingMessages()).toBe(false);
    expect(messageState.isLoadingOlder()).toBe(false);
    messageState.setRefreshMessagesState({
      phase: "running",
      statusMessage: "",
      errorMessage: "",
    });
    messageState.setMessageHistoryLoadTarget("refresh");
    expect(messageState.isLoadingMessages()).toBe(true);
    expect(messageState.isLoadingOlder()).toBe(false);
    messageState.setMessageHistoryLoadTarget("load-older");
    expect(messageState.isLoadingMessages()).toBe(false);
    expect(messageState.isLoadingOlder()).toBe(true);
    expect(messageState.reactionState()).toEqual({});
    expect(messageState.pendingReactionByKey()).toEqual({});
    expect(messageState.openReactionPickerMessageId()).toBeNull();
    expect(messageState.composerAttachments()).toEqual([]);
    expect(messageState.attachmentByChannel()).toEqual({});
  });

  it("provides profile and diagnostics defaults", () => {
    const profileState = createProfileState();
    const diagnosticsState = createDiagnosticsState();

    expect(profileState.gatewayOnline()).toBe(false);
    expect(profileState.onlineMembers()).toEqual([]);
    expect(profileState.resolvedUsernames()).toEqual({});
    expect(profileState.avatarVersionByUserId()).toEqual({});
    expect(profileState.profileDraftUsername()).toBe("");
    expect(profileState.profileDraftAbout()).toBe("");
    expect(profileState.selectedProfileAvatarFile()).toBeNull();
    expect(profileState.selectedProfileUserId()).toBeNull();

    expect(diagnosticsState.moderationRoleInput()).toBe("member");
    expect(diagnosticsState.overrideRoleInput()).toBe("member");
    expect(diagnosticsState.overrideAllowCsv()).toBe("create_message");
    expect(diagnosticsState.healthStatus()).toBe("");
    expect(diagnosticsState.echoInput()).toBe("hello filament");
    expect(diagnosticsState.diagError()).toBe("");
    expect(diagnosticsState.diagnosticsEventCounts()).toEqual({
      session_refresh_succeeded: 0,
      session_refresh_failed: 0,
      health_check_succeeded: 0,
      health_check_failed: 0,
      echo_succeeded: 0,
      echo_failed: 0,
      logout_requested: 0,
      gateway_connected: 0,
      gateway_disconnected: 0,
    });
  });

  it("provides voice and overlay defaults", () => {
    const voiceState = createVoiceState();
    const overlayState = createOverlayState();

    expect(voiceState.rtcSnapshot()).toEqual(RTC_DISCONNECTED_SNAPSHOT);
    expect(voiceState.voiceStatus()).toBe("");
    expect(voiceState.voiceError()).toBe("");
    expect(voiceState.voiceJoinState()).toEqual({
      phase: "idle",
      statusMessage: "",
      errorMessage: "",
    });
    expect(voiceState.isJoiningVoice()).toBe(false);
    voiceState.setVoiceJoinState({
      phase: "running",
      statusMessage: "",
      errorMessage: "",
    });
    expect(voiceState.isJoiningVoice()).toBe(true);
    expect(voiceState.voiceSessionChannelKey()).toBeNull();
    expect(voiceState.voiceSessionStartedAtUnixMs()).toBeNull();
    expect(voiceState.voiceSessionCapabilities()).toEqual(DEFAULT_VOICE_SESSION_CAPABILITIES);
    expect(voiceState.voiceDevicePreferences()).toEqual({
      audioInputDeviceId: null,
      audioOutputDeviceId: null,
    });
    expect(voiceState.audioInputDevices()).toEqual([]);
    expect(voiceState.audioOutputDevices()).toEqual([]);

    expect(overlayState.activeOverlayPanel()).toBeNull();
    expect(overlayState.activeSettingsCategory()).toBe(DEFAULT_SETTINGS_CATEGORY);
    expect(overlayState.activeVoiceSettingsSubmenu()).toBe(DEFAULT_VOICE_SETTINGS_SUBMENU);
    expect(overlayState.isChannelRailCollapsed()).toBe(false);
    expect(overlayState.isMemberRailCollapsed()).toBe(true);
  });

  it("exposes setter-accessor pairs by slice", () => {
    const workspaceState = createWorkspaceState();
    const overlayState = createOverlayState();

    workspaceState.workspaceChannel.setWorkspaceError("error");
    workspaceState.friendships.setFriendStatus("updated");
    workspaceState.discovery.setSearchQuery("incident");
    overlayState.setChannelRailCollapsed(true);

    expect(workspaceState.workspaceChannel.workspaceError()).toBe("error");
    expect(workspaceState.friendships.friendStatus()).toBe("updated");
    expect(workspaceState.discovery.searchQuery()).toBe("incident");
    expect(overlayState.isChannelRailCollapsed()).toBe(true);
  });

  it("tracks ordered roles, role assignments, and channel overrides per workspace", () => {
    const workspaceState = createWorkspaceState();
    const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    const userId = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAY");
    const incidentRoleId = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB0");
    const responderRoleId = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");
    const everyoneRoleId = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAZ");
    const channelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");

    workspaceState.workspaceChannel.setWorkspaceRolesForGuild(guildId, [
      {
        roleId: everyoneRoleId,
        name: workspaceRoleNameFromInput("Everyone"),
        position: 1,
        isSystem: true,
        permissions: ["create_message"],
      },
      {
        roleId: responderRoleId,
        name: workspaceRoleNameFromInput("Responder"),
        position: 50,
        isSystem: false,
        permissions: ["create_message", "delete_message"],
      },
    ]);
    workspaceState.workspaceChannel.upsertWorkspaceRoleForGuild(guildId, {
      roleId: incidentRoleId,
      name: workspaceRoleNameFromInput("Incident Commander"),
      position: 75,
      isSystem: false,
      permissions: ["manage_member_roles"],
    });
    workspaceState.workspaceChannel.updateWorkspaceRoleForGuild(
      guildId,
      responderRoleId,
      {
        name: workspaceRoleNameFromInput("Responder Prime"),
        permissions: ["delete_message", "create_message", "delete_message"],
      },
    );
    workspaceState.workspaceChannel.reorderWorkspaceRolesForGuild(guildId, [
      everyoneRoleId,
      responderRoleId,
      incidentRoleId,
    ]);

    workspaceState.workspaceChannel.assignWorkspaceRoleToUser(
      guildId,
      userId,
      responderRoleId,
    );
    workspaceState.workspaceChannel.assignWorkspaceRoleToUser(
      guildId,
      userId,
      responderRoleId,
    );
    workspaceState.workspaceChannel.unassignWorkspaceRoleFromUser(
      guildId,
      userId,
      responderRoleId,
    );
    workspaceState.workspaceChannel.assignWorkspaceRoleToUser(
      guildId,
      userId,
      responderRoleId,
    );
    workspaceState.workspaceChannel.removeWorkspaceRoleFromGuild(
      guildId,
      responderRoleId,
    );
    workspaceState.workspaceChannel.setLegacyChannelOverride(
      guildId,
      channelId,
      "moderator",
      ["delete_message", "delete_message"],
      ["delete_message", "create_message"],
      null,
    );

    expect(workspaceState.workspaceChannel.workspaceRolesByGuildId()[guildId]).toEqual([
      {
        roleId: everyoneRoleId,
        name: workspaceRoleNameFromInput("Everyone"),
        position: 75,
        isSystem: true,
        permissions: ["create_message"],
      },
      {
        roleId: incidentRoleId,
        name: workspaceRoleNameFromInput("Incident Commander"),
        position: 73,
        isSystem: false,
        permissions: ["manage_member_roles"],
      },
    ]);
    expect(
      workspaceState.workspaceChannel.workspaceUserRolesByGuildId()[guildId]?.[userId],
    ).toBeUndefined();
    expect(
      workspaceState.workspaceChannel.workspaceChannelOverridesByGuildId()[guildId]?.[
        channelId
      ],
    ).toEqual([
      {
        targetKind: "legacy_role",
        role: "moderator",
        allow: ["delete_message"],
        deny: ["create_message"],
        updatedAtUnix: null,
      },
    ]);
  });

  it("treats any running history operation as loading", () => {
    expect(
      isMessageHistoryLoading({
        phase: "running",
        statusMessage: "",
        errorMessage: "",
      }),
    ).toBe(true);
    expect(
      isMessageHistoryLoading({
        phase: "failed",
        statusMessage: "",
        errorMessage: "Unable to load messages.",
      }),
    ).toBe(false);
  });
});
