import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it } from "vitest";
import {
  channelIdFromInput,
  channelNameFromInput,
  guildIdFromInput,
  guildNameFromInput,
  type ChannelRecord,
  type WorkspaceRecord,
} from "../src/domain/chat";
import { ChannelRail } from "../src/features/app-shell/components/ChannelRail";

const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const TEXT_CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");
const VOICE_CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAY");

function workspaceFixture(): WorkspaceRecord {
  return {
    guildId: GUILD_ID,
    guildName: guildNameFromInput("Security Ops"),
    visibility: "private",
    channels: [
      {
        channelId: TEXT_CHANNEL_ID,
        name: channelNameFromInput("incident-room"),
        kind: "text",
      },
    ],
  };
}

function channelFixture(kind: "text" | "voice"): ChannelRecord {
  return {
    channelId: kind === "voice" ? VOICE_CHANNEL_ID : TEXT_CHANNEL_ID,
    name: channelNameFromInput(kind === "voice" ? "war-room" : "incident-room"),
    kind,
  };
}

function channelRailPropsFixture(
  overrides: Partial<Parameters<typeof ChannelRail>[0]> = {},
): Parameters<typeof ChannelRail>[0] {
  return {
    activeWorkspace: workspaceFixture(),
    activeChannel: channelFixture("voice"),
    activeChannelId: VOICE_CHANNEL_ID,
    activeTextChannels: [channelFixture("text")],
    activeVoiceChannels: [channelFixture("voice")],
    canManageWorkspaceChannels: true,
    canShowVoiceHeaderControls: true,
    isVoiceSessionActive: false,
    isVoiceSessionForChannel: () => false,
    voiceSessionDurationLabel: "0:00",
    voiceRosterEntriesForChannel: () => [],
    voiceStreamPermissionHints: [],
    activeVoiceSessionLabel: "war-room / Security Ops",
    rtcSnapshot: {
      connectionStatus: "disconnected",
      localParticipantIdentity: null,
      isMicrophoneEnabled: false,
      isDeafened: false,
      isCameraEnabled: false,
      isScreenShareEnabled: false,
      participants: [],
      videoTracks: [],
      activeSpeakerIdentities: [],
      lastErrorCode: null,
      lastErrorMessage: null,
    },
    canToggleVoiceCamera: false,
    canToggleVoiceScreenShare: false,
    isJoiningVoice: false,
    isLeavingVoice: false,
    isTogglingVoiceMic: false,
    isTogglingVoiceDeaf: false,
    isTogglingVoiceCamera: false,
    isTogglingVoiceScreenShare: false,
    currentUserId: "01ARZ3NDEKTSV4RRFFQ69G5FAZ",
    resolveAvatarUrl: () => null,
    userIdFromVoiceIdentity: () => null,
    actorLabel: (value) => value,
    voiceParticipantLabel: (identity) => identity,
    onOpenUserProfile: () => undefined,
    onOpenClientSettings: () => undefined,
    onOpenWorkspaceSettings: () => undefined,
    onCreateTextChannel: () => undefined,
    onCreateVoiceChannel: () => undefined,
    onSelectChannel: () => undefined,
    onJoinVoice: () => undefined,
    onToggleVoiceMicrophone: () => undefined,
    onToggleVoiceDeafen: () => undefined,
    onToggleVoiceCamera: () => undefined,
    onToggleVoiceScreenShare: () => undefined,
    onLeaveVoice: () => undefined,
    ...overrides,
  };
}

describe("app shell channel rail", () => {
  it("renders with Uno utility classes and removes legacy internal class hooks", async () => {
    render(() => <ChannelRail {...channelRailPropsFixture()} />);

    const rail = document.querySelector("aside.channel-rail");
    expect(rail).not.toBeNull();
    expect(rail).toHaveClass("grid");
    expect(rail).toHaveClass("bg-bg-1");

    const menuTrigger = screen.getByRole("button", { name: "Open workspace menu" });
    expect(menuTrigger).toHaveClass("w-full");
    expect(menuTrigger).toHaveClass("border-line-soft");
    expect(menuTrigger).toHaveClass("enabled:hover:bg-bg-3");
    expect(screen.getByText("private workspace")).toHaveClass("px-[0.52rem]");

    await fireEvent.click(menuTrigger);
    expect(screen.getByRole("menuitem", { name: "Invite to workspace" })).toHaveClass(
      "rounded-[0.5rem]",
    );
    expect(screen.getByRole("button", { name: "Join Voice" })).toHaveClass("h-[2.2rem]");
    expect(screen.getByRole("button", { name: "war-room" })).toHaveClass("rounded-[0.52rem]");

    expect(document.querySelector(".workspace-menu-trigger")).toBeNull();
    expect(document.querySelector(".workspace-menu-item")).toBeNull();
    expect(document.querySelector(".workspace-menu-divider")).toBeNull();
    expect(document.querySelector(".channel-nav")).toBeNull();
    expect(document.querySelector(".channel-row")).toBeNull();
    expect(document.querySelector(".voice-connected-dock")).toBeNull();
    expect(document.querySelector(".voice-dock-icon-button")).toBeNull();
    expect(document.querySelector(".channel-rail-account-bar")).toBeNull();
    expect(document.querySelector(".channel-rail-account-action")).toBeNull();
  });

  it("uses token utility classes for disconnect danger styling with no inline styles", () => {
    render(() => (
      <ChannelRail
        {...channelRailPropsFixture({
          isVoiceSessionActive: true,
          canShowVoiceHeaderControls: true,
          isVoiceSessionForChannel: () => true,
        })}
      />
    ));

    const disconnectButton = screen.getByRole("button", { name: "Disconnect" });
    expect(disconnectButton).toHaveClass("bg-danger-panel");
    expect(disconnectButton).toHaveClass("border-danger-panel-strong");
    expect(disconnectButton).toHaveClass("text-danger-ink");
    expect(disconnectButton).not.toHaveAttribute("style");
  });

  it("keeps voice avatar speaking hook classes for roster-state tests", () => {
    render(() => (
      <ChannelRail
        {...channelRailPropsFixture({
          isVoiceSessionForChannel: () => true,
          voiceRosterEntriesForChannel: () => [
            {
              identity: "remote.voice",
              isLocal: false,
              isMuted: false,
              isDeafened: false,
              isSpeaking: true,
              hasCamera: false,
              hasScreenShare: false,
            },
          ],
        })}
      />
    ));

    const avatar = document.querySelector(".voice-tree-avatar");
    expect(avatar).not.toBeNull();
    expect(avatar).toHaveClass("voice-tree-avatar-speaking");
  });
});
