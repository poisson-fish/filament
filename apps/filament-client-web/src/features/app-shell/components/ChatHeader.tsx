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
    "inline-flex items-center rounded-full border px-[0.55rem] py-[0.15rem] text-[0.7rem] font-[600] tracking-[0.02em] leading-[1.2] shadow-sm";
  
  // Clean ghost-style button for the toolbar
  const headerIconButtonClass =
    "inline-flex h-[2.1rem] w-[2.1rem] items-center justify-center rounded-[0.6rem] border border-transparent bg-transparent text-ink-2 transition-all duration-[140ms] ease-out hover:bg-bg-3 hover:text-ink-0 hover:border-line-soft hover:shadow-sm focus-visible:bg-bg-3 focus-visible:text-ink-0 focus-visible:border-line-soft focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-40";

  return (
    <header class="chat-header flex items-center justify-between gap-[0.8rem] border-b border-line px-[1.1rem] py-[0.8rem] bg-bg-2 [@media(max-width:900px)]:flex-col [@media(max-width:900px)]:items-start [@media(max-width:900px)]:gap-[0.6rem]">
      <div class="min-w-0 flex flex-col gap-[0.1rem]">
        <div class="flex items-center gap-[0.6rem]">
          <h3 class="m-0 text-[1.15rem] font-[700] leading-[1.2] tracking-[0.01em] text-ink-0">
            {props.activeChannel
              ? channelHeaderLabel({ kind: props.activeChannel.kind, name: props.activeChannel.name })
              : "#no-channel"}
          </h3>
          <span
            classList={{
              [statusBadgeClass]: true,
              "border-transparent bg-ok text-bg-0 shadow-sm": props.gatewayOnline,
              "border-transparent bg-danger text-danger-ink shadow-sm": !props.gatewayOnline,
            }}
            title={props.gatewayOnline ? "Gateway connected" : "Gateway disconnected"}
          >
            {props.gatewayOnline ? "Live" : "Offline"}
          </span>
        </div>
        <Show when={props.canShowVoiceHeaderControls || props.isVoiceSessionActive}>
             <p class="text-[0.75rem] text-ink-2 flex items-center gap-2">
                <span classList={{
                    "w-2 h-2 rounded-full": true,
                    "bg-ink-2": props.voiceConnectionState === "disconnected",
                    "bg-brand animate-pulse": props.voiceConnectionState === "connecting" || props.voiceConnectionState === "reconnecting",
                    "bg-ok": props.voiceConnectionState === "connected",
                    "bg-danger": props.voiceConnectionState === "error",
                }}></span>
                Voice {props.voiceConnectionState}
             </p>
        </Show>
      </div>

      <div class="flex flex-wrap items-center justify-start gap-[0.4rem]">
        
        <div class="flex items-center gap-[0.2rem] pr-[0.4rem] border-r border-line/40 mr-[0.2rem]">
             <button
              type="button"
              class={headerIconButtonClass}
              aria-label={props.isChannelRailCollapsed ? "Show channels" : "Hide channels"}
              title={props.isChannelRailCollapsed ? "Show channels" : "Hide channels"}
              onClick={props.onToggleChannelRail}
            >
              <span
                class="icon-mask h-[1.1rem] w-[1.1rem]"
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
                class="icon-mask h-[1.1rem] w-[1.1rem]"
                style={`--icon-url: url("${WORKSPACE_TOOLS_ICON_URL}")`}
                aria-hidden="true"
              />
            </button>
        </div>

        <button
          type="button"
          class={headerIconButtonClass}
          aria-label="Directory"
          title="Directory"
          onClick={() => props.onOpenPanel("public-directory")}
        >
          <span class="icon-mask h-[1.1rem] w-[1.1rem]" style={`--icon-url: url("${DIRECTORY_ICON_URL}")`} aria-hidden="true" />
        </button>
        <button
          type="button"
          class={headerIconButtonClass}
          aria-label="Friends"
          title="Friends"
          onClick={() => props.onOpenPanel("friendships")}
        >
          <span class="icon-mask h-[1.1rem] w-[1.1rem]" style={`--icon-url: url("${FRIENDS_ICON_URL}")`} aria-hidden="true" />
        </button>
        <button
          type="button"
          class={headerIconButtonClass}
          aria-label="Refresh"
          title="Refresh messages"
          onClick={props.onRefreshMessages}
        >
          <span
            class="icon-mask h-[1.1rem] w-[1.1rem]"
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
            class={`icon-mask h-[1.1rem] w-[1.1rem] ${props.isRefreshingSession ? "animate-spin" : ""}`}
            style={`--icon-url: url("${REFRESH_SESSION_ICON_URL}")`}
            aria-hidden="true"
          />
        </button>
        
        <div class="w-px h-[1.4rem] bg-line/40 mx-[0.2rem]"></div>

        <button
          type="button"
          class={`${headerIconButtonClass} text-danger hover:bg-danger-panel hover:text-danger-ink hover:border-danger-panel-strong`}
          aria-label="Logout"
          title="Logout"
          onClick={props.onLogout}
        >
          <span class="icon-mask h-[1.1rem] w-[1.1rem]" style={`--icon-url: url("${LOGOUT_ICON_URL}")`} aria-hidden="true" />
        </button>
      </div>
    </header>
  );
}
