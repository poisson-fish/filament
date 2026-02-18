import { Show } from "solid-js";
import type { ChannelRecord } from "../../../domain/chat";
import { channelHeaderLabel } from "../helpers";
import type { OverlayPanel } from "../types";

const TOGGLE_CHANNELS_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/System/Bar_Left.svg",
  import.meta.url,
).href;
const WORKSPACE_TOOLS_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/System/Window_Sidebar.svg",
  import.meta.url,
).href;
const DIRECTORY_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Navigation/Compass.svg",
  import.meta.url,
).href;
const FRIENDS_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/User/Users.svg",
  import.meta.url,
).href;
const REFRESH_MESSAGES_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Arrow/Arrows_Reload_01.svg",
  import.meta.url,
).href;
const REFRESH_SESSION_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Arrow/Arrow_Reload_02.svg",
  import.meta.url,
).href;
const LOGOUT_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Interface/Log_Out.svg",
  import.meta.url,
).href;

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
        <button
          type="button"
          class="header-icon-button"
          aria-label={props.isChannelRailCollapsed ? "Show channels" : "Hide channels"}
          title={props.isChannelRailCollapsed ? "Show channels" : "Hide channels"}
          onClick={props.onToggleChannelRail}
        >
          <span class="icon-mask" style={`--icon-url: url("${TOGGLE_CHANNELS_ICON_URL}")`} aria-hidden="true" />
        </button>
        <button
          type="button"
          class="header-icon-button"
          aria-label={
            props.isMemberRailCollapsed
              ? "Show workspace tools rail"
              : "Hide workspace tools rail"
          }
          title={
            props.isMemberRailCollapsed
              ? "Show workspace tools rail"
              : "Hide workspace tools rail"
          }
          onClick={props.onToggleMemberRail}
        >
          <span class="icon-mask" style={`--icon-url: url("${WORKSPACE_TOOLS_ICON_URL}")`} aria-hidden="true" />
        </button>
        <button
          type="button"
          class="header-icon-button"
          aria-label="Directory"
          title="Directory"
          onClick={() => props.onOpenPanel("public-directory")}
        >
          <span class="icon-mask" style={`--icon-url: url("${DIRECTORY_ICON_URL}")`} aria-hidden="true" />
        </button>
        <button
          type="button"
          class="header-icon-button"
          aria-label="Friends"
          title="Friends"
          onClick={() => props.onOpenPanel("friendships")}
        >
          <span class="icon-mask" style={`--icon-url: url("${FRIENDS_ICON_URL}")`} aria-hidden="true" />
        </button>
        <button
          type="button"
          class="header-icon-button"
          aria-label="Refresh"
          title="Refresh messages"
          onClick={props.onRefreshMessages}
        >
          <span class="icon-mask" style={`--icon-url: url("${REFRESH_MESSAGES_ICON_URL}")`} aria-hidden="true" />
        </button>
        <button
          type="button"
          class="header-icon-button"
          aria-label={props.isRefreshingSession ? "Refreshing..." : "Refresh session"}
          title={props.isRefreshingSession ? "Refreshing session..." : "Refresh session"}
          onClick={props.onRefreshSession}
          disabled={props.isRefreshingSession}
        >
          <span class="icon-mask" style={`--icon-url: url("${REFRESH_SESSION_ICON_URL}")`} aria-hidden="true" />
        </button>
        <button type="button" class="header-icon-button logout" aria-label="Logout" title="Logout" onClick={props.onLogout}>
          <span class="icon-mask" style={`--icon-url: url("${LOGOUT_ICON_URL}")`} aria-hidden="true" />
        </button>
      </div>
    </header>
  );
}
