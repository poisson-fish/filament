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
  const statusBadgeClass =
    "inline-flex items-center rounded-full border px-[0.45rem] py-[0.2rem] text-[0.72rem] leading-[1.2]";
  const headerIconButtonClass =
    "inline-flex h-[2rem] w-[2rem] items-center justify-center rounded-[0.5rem] border border-line bg-bg-3 p-0 text-ink-1 transition-colors duration-[120ms] ease-out enabled:hover:bg-bg-4 disabled:cursor-default disabled:opacity-62";

  return (
    <header class="chat-header flex items-center justify-between gap-[0.68rem] border-b border-line px-[0.95rem] py-[0.74rem] [@media(max-width:900px)]:flex-col [@media(max-width:900px)]:items-start [@media(max-width:900px)]:gap-[0.48rem]">
      <div class="min-w-0">
        <h3 class="m-0 text-[1.24rem] font-[780] leading-[1.2] tracking-[0.005em] text-ink-0">
          {props.activeChannel
            ? channelHeaderLabel({ kind: props.activeChannel.kind, name: props.activeChannel.name })
            : "#no-channel"}
        </h3>
        <p class="mt-[0.14rem] text-[0.8rem] text-ink-2">
          Gateway {props.gatewayOnline ? "connected" : "disconnected"}
        </p>
      </div>
      <div class="flex flex-wrap items-center justify-start gap-[0.28rem]">
        <span
          classList={{
            [statusBadgeClass]: true,
            "border-ok bg-bg-3 text-ok": props.gatewayOnline,
            "border-danger bg-bg-3 text-danger": !props.gatewayOnline,
          }}
        >
          {props.gatewayOnline ? "Live" : "Offline"}
        </span>
        <Show when={props.canShowVoiceHeaderControls || props.isVoiceSessionActive}>
          <span
            classList={{
              [statusBadgeClass]: true,
              "border-line bg-bg-3 text-ink-1": props.voiceConnectionState === "disconnected",
              "border-brand bg-bg-3 text-brand":
                props.voiceConnectionState === "connecting" || props.voiceConnectionState === "reconnecting",
              "border-ok bg-bg-3 text-ok": props.voiceConnectionState === "connected",
              "border-danger bg-bg-3 text-danger": props.voiceConnectionState === "error",
            }}
          >
            Voice {props.voiceConnectionState}
          </span>
        </Show>
        <button
          type="button"
          class={headerIconButtonClass}
          aria-label={props.isChannelRailCollapsed ? "Show channels" : "Hide channels"}
          title={props.isChannelRailCollapsed ? "Show channels" : "Hide channels"}
          onClick={props.onToggleChannelRail}
        >
          <span
            class="icon-mask h-[1rem] w-[1rem]"
            style={`--icon-url: url("${TOGGLE_CHANNELS_ICON_URL}")`}
            aria-hidden="true"
          />
        </button>
        <button
          type="button"
          class={headerIconButtonClass}
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
          <span
            class="icon-mask h-[1rem] w-[1rem]"
            style={`--icon-url: url("${WORKSPACE_TOOLS_ICON_URL}")`}
            aria-hidden="true"
          />
        </button>
        <button
          type="button"
          class={headerIconButtonClass}
          aria-label="Directory"
          title="Directory"
          onClick={() => props.onOpenPanel("public-directory")}
        >
          <span class="icon-mask h-[1rem] w-[1rem]" style={`--icon-url: url("${DIRECTORY_ICON_URL}")`} aria-hidden="true" />
        </button>
        <button
          type="button"
          class={headerIconButtonClass}
          aria-label="Friends"
          title="Friends"
          onClick={() => props.onOpenPanel("friendships")}
        >
          <span class="icon-mask h-[1rem] w-[1rem]" style={`--icon-url: url("${FRIENDS_ICON_URL}")`} aria-hidden="true" />
        </button>
        <button
          type="button"
          class={headerIconButtonClass}
          aria-label="Refresh"
          title="Refresh messages"
          onClick={props.onRefreshMessages}
        >
          <span
            class="icon-mask h-[1rem] w-[1rem]"
            style={`--icon-url: url("${REFRESH_MESSAGES_ICON_URL}")`}
            aria-hidden="true"
          />
        </button>
        <button
          type="button"
          class={headerIconButtonClass}
          aria-label={props.isRefreshingSession ? "Refreshing..." : "Refresh session"}
          title={props.isRefreshingSession ? "Refreshing session..." : "Refresh session"}
          onClick={props.onRefreshSession}
          disabled={props.isRefreshingSession}
        >
          <span
            class="icon-mask h-[1rem] w-[1rem]"
            style={`--icon-url: url("${REFRESH_SESSION_ICON_URL}")`}
            aria-hidden="true"
          />
        </button>
        <button
          type="button"
          class={`${headerIconButtonClass} border-danger-panel-strong bg-danger-panel text-danger-ink enabled:hover:bg-danger-panel-strong`}
          aria-label="Logout"
          title="Logout"
          onClick={props.onLogout}
        >
          <span class="icon-mask h-[1rem] w-[1rem]" style={`--icon-url: url("${LOGOUT_ICON_URL}")`} aria-hidden="true" />
        </button>
      </div>
    </header>
  );
}
