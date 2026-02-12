import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  channelIdFromInput,
  channelNameFromInput,
  guildIdFromInput,
  guildNameFromInput,
  type ChannelRecord,
  type WorkspaceRecord,
} from "../src/domain/chat";
import { ChannelRail } from "../src/features/app-shell/components/ChannelRail";
import { ChatHeader } from "../src/features/app-shell/components/ChatHeader";
import { MemberRail } from "../src/features/app-shell/components/MemberRail";
import { ServerRail } from "../src/features/app-shell/components/ServerRail";

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

describe("app shell extracted layout components", () => {
  it("routes server rail actions to typed callbacks", () => {
    const onSelectWorkspace = vi.fn();
    const onOpenPanel = vi.fn();

    render(() => (
      <ServerRail
        workspaces={[workspaceFixture()]}
        activeGuildId={null}
        isCreatingWorkspace={false}
        onSelectWorkspace={onSelectWorkspace}
        onOpenPanel={onOpenPanel}
      />
    ));

    fireEvent.click(screen.getByRole("button", { name: "S" }));
    expect(onSelectWorkspace).toHaveBeenCalledWith(GUILD_ID, TEXT_CHANNEL_ID);
    expect(screen.queryByRole("button", { name: "Open settings panel" })).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Open friendships panel" }));
    expect(onOpenPanel).toHaveBeenCalledWith("friendships");
  });

  it("renders voice controls in channel rail and invokes handlers", () => {
    const onJoinVoice = vi.fn();
    const onOpenSettings = vi.fn();

    render(() => (
      <ChannelRail
        activeWorkspace={workspaceFixture()}
        activeChannel={channelFixture("voice")}
        activeChannelId={VOICE_CHANNEL_ID}
        activeTextChannels={[channelFixture("text")]}
        activeVoiceChannels={[channelFixture("voice")]}
        canManageWorkspaceChannels={true}
        canShowVoiceHeaderControls={true}
        isVoiceSessionActive={false}
        isVoiceSessionForChannel={() => false}
        voiceSessionDurationLabel="0:00"
        voiceRosterEntries={[]}
        voiceStreamPermissionHints={[]}
        activeVoiceSessionLabel="war-room / Security Ops"
        rtcSnapshot={{
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
        }}
        canToggleVoiceCamera={false}
        canToggleVoiceScreenShare={false}
        isJoiningVoice={false}
        isLeavingVoice={false}
        isTogglingVoiceMic={false}
        isTogglingVoiceCamera={false}
        isTogglingVoiceScreenShare={false}
        currentUserId={"01ARZ3NDEKTSV4RRFFQ69G5FAZ"}
        resolveAvatarUrl={() => null}
        userIdFromVoiceIdentity={() => null}
        actorLabel={(value) => value}
        voiceParticipantLabel={(identity) => identity}
        onOpenUserProfile={() => {}}
        onOpenSettings={onOpenSettings}
        onCreateTextChannel={() => {}}
        onCreateVoiceChannel={() => {}}
        onSelectChannel={() => {}}
        onJoinVoice={onJoinVoice}
        onToggleVoiceMicrophone={() => {}}
        onToggleVoiceCamera={() => {}}
        onToggleVoiceScreenShare={() => {}}
        onLeaveVoice={() => {}}
      />
    ));

    fireEvent.click(screen.getByRole("button", { name: "Join Voice" }));
    expect(onJoinVoice).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Join Voice")).not.toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Open settings panel" }));
    expect(onOpenSettings).toHaveBeenCalledTimes(1);
  });

  it("keeps member rail panel actions and chat header toggles wired", () => {
    const onOpenPanel = vi.fn();
    const onToggleChannels = vi.fn();

    render(() => (
      <>
        <MemberRail
          profileLoading={false}
          profileErrorText=""
          profile={{ userId: "01ARZ3NDEKTSV4RRFFQ69G5FAZ", username: "owner" }}
          showUnauthorizedWorkspaceNote={false}
          canAccessActiveChannel={true}
          onlineMembers={["01ARZ3NDEKTSV4RRFFQ69G5FAZ"]}
          hasModerationAccess={true}
          displayUserLabel={(value) => value}
          onOpenPanel={onOpenPanel}
        />
        <ChatHeader
          activeChannel={channelFixture("text")}
          gatewayOnline={true}
          canShowVoiceHeaderControls={false}
          isVoiceSessionActive={false}
          voiceConnectionState="disconnected"
          isChannelRailCollapsed={false}
          isMemberRailCollapsed={false}
          isRefreshingSession={false}
          onToggleChannelRail={onToggleChannels}
          onToggleMemberRail={() => {}}
          onOpenPanel={onOpenPanel}
          onRefreshMessages={() => {}}
          onRefreshSession={() => {}}
          onLogout={() => {}}
        />
      </>
    ));

    fireEvent.click(screen.getByRole("button", { name: "Open moderation panel" }));
    expect(onOpenPanel).toHaveBeenCalledWith("moderation");

    fireEvent.click(screen.getByRole("button", { name: "Hide channels" }));
    expect(onToggleChannels).toHaveBeenCalledTimes(1);
  });
});
