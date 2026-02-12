import { createRoot, createSignal } from "solid-js";
import { describe, expect, it } from "vitest";
import {
  channelIdFromInput,
  channelNameFromInput,
  guildIdFromInput,
  guildNameFromInput,
  type AttachmentRecord,
  type ChannelPermissionSnapshot,
  type WorkspaceRecord,
} from "../src/domain/chat";
import type { RtcSnapshot } from "../src/lib/rtc";
import { channelKey } from "../src/features/app-shell/helpers";
import {
  buildVoiceRosterEntries,
  createAppShellSelectors,
} from "../src/features/app-shell/selectors/create-app-shell-selectors";
import type { OverlayPanel, VoiceSessionCapabilities } from "../src/features/app-shell/types";

const GUILD_A = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAA");
const GUILD_B = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAB");
const TEXT_A = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAC");
const VOICE_A = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAD");
const TEXT_B = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAE");

function workspaceFixture(): WorkspaceRecord[] {
  return [
    {
      guildId: GUILD_A,
      guildName: guildNameFromInput("Security Ops"),
      visibility: "private",
      channels: [
        { channelId: TEXT_A, name: channelNameFromInput("incident-room"), kind: "text" },
        { channelId: VOICE_A, name: channelNameFromInput("war-room"), kind: "voice" },
      ],
    },
    {
      guildId: GUILD_B,
      guildName: guildNameFromInput("NOC"),
      visibility: "private",
      channels: [{ channelId: TEXT_B, name: channelNameFromInput("alerts"), kind: "text" }],
    },
  ];
}

function rtcSnapshotFixture(): RtcSnapshot {
  return {
    connectionStatus: "disconnected",
    localParticipantIdentity: null,
    isMicrophoneEnabled: false,
    isCameraEnabled: false,
    isScreenShareEnabled: false,
    participants: [],
    videoTracks: [],
    activeSpeakerIdentities: [],
    lastErrorCode: null,
    lastErrorMessage: null,
  };
}

function permissionsFixture(): ChannelPermissionSnapshot {
  return {
    role: "member",
    permissions: ["create_message"],
  };
}

function voiceCapabilitiesFixture(): VoiceSessionCapabilities {
  return {
    canSubscribe: false,
    publishSources: ["microphone"],
  };
}

function createSelectorHarness() {
  const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>(workspaceFixture());
  const [activeGuildId, setActiveGuildId] = createSignal<typeof GUILD_A | typeof GUILD_B | null>(
    GUILD_A,
  );
  const [activeChannelId, setActiveChannelId] = createSignal<
    typeof TEXT_A | typeof VOICE_A | typeof TEXT_B | null
  >(TEXT_A);
  const [channelPermissions, setChannelPermissions] =
    createSignal<ChannelPermissionSnapshot | null>(permissionsFixture());
  const [voiceSessionChannelKey, setVoiceSessionChannelKey] = createSignal<string | null>(null);
  const [attachmentByChannel] = createSignal<Record<string, AttachmentRecord[]>>({});
  const [rtcSnapshot, setRtcSnapshot] = createSignal<RtcSnapshot>(rtcSnapshotFixture());
  const [voiceSessionCapabilities, setVoiceSessionCapabilities] =
    createSignal<VoiceSessionCapabilities>(voiceCapabilitiesFixture());
  const [voiceSessionStartedAtUnixMs] = createSignal<number | null>(null);
  const [voiceDurationClockUnixMs] = createSignal(0);
  const [activeOverlayPanel, setActiveOverlayPanel] = createSignal<OverlayPanel | null>(null);

  const selectors = createAppShellSelectors({
    workspaces,
    activeGuildId,
    activeChannelId,
    channelPermissions,
    voiceSessionChannelKey,
    attachmentByChannel,
    rtcSnapshot,
    voiceSessionCapabilities,
    voiceSessionStartedAtUnixMs,
    voiceDurationClockUnixMs,
    activeOverlayPanel,
  });

  return {
    selectors,
    setWorkspaces,
    setActiveGuildId,
    setActiveChannelId,
    setChannelPermissions,
    setVoiceSessionChannelKey,
    setRtcSnapshot,
    setVoiceSessionCapabilities,
    setActiveOverlayPanel,
  };
}

describe("app shell selectors", () => {
  it("selects active workspace and channel from current selection", () => {
    createRoot((dispose) => {
      const harness = createSelectorHarness();

      expect(harness.selectors.activeWorkspace()?.guildId).toBe(GUILD_A);
      expect(harness.selectors.activeChannel()?.channelId).toBe(TEXT_A);
      expect(harness.selectors.activeTextChannels().map((channel) => channel.channelId)).toEqual([
        TEXT_A,
      ]);
      expect(harness.selectors.activeVoiceChannels().map((channel) => channel.channelId)).toEqual([
        VOICE_A,
      ]);

      harness.setActiveGuildId(GUILD_B);
      expect(harness.selectors.activeWorkspace()?.guildId).toBe(GUILD_B);
      expect(harness.selectors.activeChannel()).toBeNull();

      harness.setActiveChannelId(TEXT_B);
      expect(harness.selectors.activeChannel()?.channelId).toBe(TEXT_B);
      dispose();
    });
  });

  it("derives permission flags and workspace-create close behavior", () => {
    createRoot((dispose) => {
      const harness = createSelectorHarness();

      expect(harness.selectors.canAccessActiveChannel()).toBe(true);
      expect(harness.selectors.canManageWorkspaceChannels()).toBe(false);
      expect(harness.selectors.canManageSearchMaintenance()).toBe(false);
      expect(harness.selectors.hasModerationAccess()).toBe(false);
      expect(harness.selectors.canDeleteMessages()).toBe(false);

      harness.setChannelPermissions({
        role: "moderator",
        permissions: [
          "create_message",
          "publish_video",
          "publish_screen_share",
          "subscribe_streams",
          "manage_roles",
          "manage_channel_overrides",
          "ban_member",
          "delete_message",
        ],
      });
      expect(harness.selectors.canManageWorkspaceChannels()).toBe(true);
      expect(harness.selectors.canManageSearchMaintenance()).toBe(true);
      expect(harness.selectors.hasModerationAccess()).toBe(true);
      expect(harness.selectors.canDeleteMessages()).toBe(true);

      harness.setActiveOverlayPanel("workspace-create");
      harness.setWorkspaces([]);
      expect(harness.selectors.canDismissWorkspaceCreateForm()).toBe(false);
      expect(harness.selectors.canCloseActivePanel()).toBe(false);

      harness.setWorkspaces(workspaceFixture());
      expect(harness.selectors.canDismissWorkspaceCreateForm()).toBe(true);
      expect(harness.selectors.canCloseActivePanel()).toBe(true);
      dispose();
    });
  });

  it("synthesizes voice roster entries and session permission hints", () => {
    createRoot((dispose) => {
      const harness = createSelectorHarness();
      const localIdentity = "u.01ARZ3NDEKTSV4RRFFQ69G5FAA.voice";
      const remoteIdentity = "u.01ARZ3NDEKTSV4RRFFQ69G5FAB.voice";

      const connectedSnapshot: RtcSnapshot = {
        ...rtcSnapshotFixture(),
        connectionStatus: "connected",
        localParticipantIdentity: localIdentity,
        participants: [
          { identity: localIdentity, subscribedTrackCount: 1 },
          { identity: remoteIdentity, subscribedTrackCount: 1 },
        ],
        videoTracks: [
          {
            trackSid: "local-camera",
            participantIdentity: localIdentity,
            source: "camera",
            isLocal: true,
          },
          {
            trackSid: "remote-screen",
            participantIdentity: remoteIdentity,
            source: "screen_share",
            isLocal: false,
          },
        ],
        activeSpeakerIdentities: [remoteIdentity],
      };

      expect(buildVoiceRosterEntries(connectedSnapshot)).toEqual([
        {
          identity: localIdentity,
          isLocal: true,
          isSpeaking: false,
          hasCamera: true,
          hasScreenShare: false,
        },
        {
          identity: remoteIdentity,
          isLocal: false,
          isSpeaking: true,
          hasCamera: false,
          hasScreenShare: true,
        },
      ]);

      harness.setRtcSnapshot(connectedSnapshot);
      harness.setActiveChannelId(VOICE_A);
      harness.setVoiceSessionChannelKey(channelKey(GUILD_A, VOICE_A));
      harness.setChannelPermissions({
        role: "member",
        permissions: [
          "create_message",
          "publish_video",
          "publish_screen_share",
          "subscribe_streams",
        ],
      });
      harness.setVoiceSessionCapabilities({
        canSubscribe: false,
        publishSources: ["microphone"],
      });

      expect(harness.selectors.voiceRosterEntries()).toEqual([
        {
          identity: localIdentity,
          isLocal: true,
          isSpeaking: false,
          hasCamera: true,
          hasScreenShare: false,
        },
        {
          identity: remoteIdentity,
          isLocal: false,
          isSpeaking: true,
          hasCamera: false,
          hasScreenShare: true,
        },
      ]);
      expect(harness.selectors.voiceStreamPermissionHints()).toEqual([
        "Camera disabled: this voice token did not grant camera publish.",
        "Screen share disabled: this voice token did not grant screen publish.",
        "Remote stream subscription is denied for this call.",
      ]);
      expect(harness.selectors.canToggleVoiceCamera()).toBe(false);
      expect(harness.selectors.canToggleVoiceScreenShare()).toBe(false);

      harness.setVoiceSessionCapabilities({
        canSubscribe: true,
        publishSources: ["microphone", "camera", "screen_share"],
      });
      expect(harness.selectors.voiceStreamPermissionHints()).toEqual([]);
      expect(harness.selectors.canToggleVoiceCamera()).toBe(true);
      expect(harness.selectors.canToggleVoiceScreenShare()).toBe(true);
      dispose();
    });
  });
});
