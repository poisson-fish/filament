import { Show } from "solid-js";
import type { ChannelRecord } from "../../../domain/chat";
import { channelHeaderLabel } from "../helpers";
import type { OverlayPanel } from "../types";

interface ChatHeaderProps {
  activeChannel: ChannelRecord | null;
  gatewayOnline: boolean;
  canShowVoiceHeaderControls: boolean;
  isVoiceSessionActive: boolean;
  voiceConnectionState: string;
  isChannelRailCollapsed: boolean;
  isMemberRailCollapsed: boolean;
  isRefreshingSession: boolean;
  onToggleChannelRail: () => void;
  onToggleMemberRail: () => void;
  onOpenPanel: (panel: OverlayPanel) => void;
  onRefreshMessages: () => void;
  onRefreshSession: () => void;
  onLogout: () => void;
}

export function ChatHeader(props: ChatHeaderProps) {
  return (
    <header class="chat-header">
      <div>
        <h3>
          {props.activeChannel
            ? channelHeaderLabel({ kind: props.activeChannel.kind, name: props.activeChannel.name })
            : "#no-channel"}
        </h3>
        <p>Gateway {props.gatewayOnline ? "connected" : "disconnected"}</p>
      </div>
      <div class="header-actions">
        <span classList={{ "gateway-badge": true, online: props.gatewayOnline }}>
          {props.gatewayOnline ? "Live" : "Offline"}
        </span>
        <Show when={props.canShowVoiceHeaderControls || props.isVoiceSessionActive}>
          <span
            classList={{
              "voice-badge": true,
              connected: props.voiceConnectionState === "connected",
              connecting: props.voiceConnectionState === "connecting",
              reconnecting: props.voiceConnectionState === "reconnecting",
              error: props.voiceConnectionState === "error",
            }}
          >
            Voice {props.voiceConnectionState}
          </span>
        </Show>
        <button type="button" onClick={props.onToggleChannelRail}>
          {props.isChannelRailCollapsed ? "Show channels" : "Hide channels"}
        </button>
        <button type="button" onClick={props.onToggleMemberRail}>
          {props.isMemberRailCollapsed ? "Show members" : "Hide members"}
        </button>
        <button type="button" onClick={() => props.onOpenPanel("public-directory")}>
          Directory
        </button>
        <button type="button" onClick={() => props.onOpenPanel("friendships")}>
          Friends
        </button>
        <button type="button" onClick={props.onRefreshMessages}>
          Refresh
        </button>
        <button type="button" onClick={props.onRefreshSession} disabled={props.isRefreshingSession}>
          {props.isRefreshingSession ? "Refreshing..." : "Refresh session"}
        </button>
        <button class="logout" onClick={props.onLogout}>
          Logout
        </button>
      </div>
    </header>
  );
}
