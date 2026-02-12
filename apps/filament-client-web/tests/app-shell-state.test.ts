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
import { createWorkspaceState } from "../src/features/app-shell/state/workspace-state";
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

    expect(workspaceState.workspaces()).toEqual([]);
    expect(workspaceState.activeGuildId()).toBeNull();
    expect(workspaceState.activeChannelId()).toBeNull();
    expect(workspaceState.workspaceBootstrapDone()).toBe(false);
    expect(workspaceState.createGuildName()).toBe("Security Ops");
    expect(workspaceState.createGuildVisibility()).toBe("private");
    expect(workspaceState.friendRequests()).toEqual({ incoming: [], outgoing: [] });
    expect(workspaceState.channelPermissions()).toBeNull();

    expect(messageState.composer()).toBe("");
    expect(messageState.messages()).toEqual([]);
    expect(messageState.nextBefore()).toBeNull();
    expect(messageState.showLoadOlderButton()).toBe(false);
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
  });

  it("provides voice and overlay defaults", () => {
    const voiceState = createVoiceState();
    const overlayState = createOverlayState();

    expect(voiceState.rtcSnapshot()).toEqual(RTC_DISCONNECTED_SNAPSHOT);
    expect(voiceState.voiceStatus()).toBe("");
    expect(voiceState.voiceError()).toBe("");
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
    expect(overlayState.isMemberRailCollapsed()).toBe(false);
  });

  it("exposes setter-accessor pairs by slice", () => {
    const workspaceState = createWorkspaceState();
    const overlayState = createOverlayState();

    workspaceState.setWorkspaceError("error");
    overlayState.setChannelRailCollapsed(true);

    expect(workspaceState.workspaceError()).toBe("error");
    expect(overlayState.isChannelRailCollapsed()).toBe(true);
  });
});
