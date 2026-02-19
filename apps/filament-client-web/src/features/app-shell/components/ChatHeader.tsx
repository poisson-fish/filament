import { Show } from "solid-js";
import type { ChannelRecord } from "../../../domain/chat";
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
const TEXT_CHANNEL_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Communication/Chat.svg",
  import.meta.url,
).href;
const VOICE_CHANNEL_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/User/User_Voice.svg",
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
    "inline-flex items-center self-baseline rounded-full border px-[0.55rem] py-[0.15rem] text-[0.69rem] font-[700] tracking-[0.03em] leading-[1.1] shadow-sm";
  const activeChannelLabel = () => props.activeChannel?.name ?? "no-channel";
  const activeChannelIconUrl = () =>
    props.activeChannel?.kind === "voice" ? VOICE_CHANNEL_ICON_URL : TEXT_CHANNEL_ICON_URL;

  const headerIconButtonClass =
    "inline-flex h-[2.1rem] w-[2.1rem] items-center justify-center rounded-[0.62rem] border border-line-soft bg-bg-3 text-ink-1 transition-all duration-[140ms] ease-out hover:bg-bg-4 hover:text-ink-0 focus-visible:bg-bg-4 focus-visible:text-ink-0 focus-visible:outline-none disabled:cursor-not-allowed disabled:opacity-40";

  return (
    <header class="chat-header flex items-center justify-between gap-[0.8rem] border-b border-line px-[1.1rem] py-[0.8rem] bg-bg-1 [@media(max-width:900px)]:flex-col [@media(max-width:900px)]:items-start [@media(max-width:900px)]:gap-[0.6rem]">
      <div class="min-w-0 flex flex-col gap-[0.16rem]">
        <div class="flex items-center gap-[0.52rem]">
          <span
            class="icon-mask h-[1.08rem] w-[1.08rem] shrink-0 text-ink-2"
            style={`--icon-url: url("${activeChannelIconUrl()}")`}
            aria-hidden="true"
          />
          <h3 class="m-0 text-[1.06rem] font-[760] leading-[1.2] tracking-[0.01em] text-ink-0">
            {activeChannelLabel()}
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
          <p class="m-0 flex items-center gap-[0.38rem] text-[0.74rem] text-ink-2">
            <span
              classList={{
                "h-[0.52rem] w-[0.52rem] rounded-full": true,
                "bg-ink-2": props.voiceConnectionState === "disconnected",
                "bg-brand animate-pulse":
                  props.voiceConnectionState === "connecting" ||
                  props.voiceConnectionState === "reconnecting",
                "bg-ok": props.voiceConnectionState === "connected",
                "bg-danger": props.voiceConnectionState === "error",
              }}
            />
            Voice {props.voiceConnectionState}
          </p>
        </Show>
      </div>

      <div class="flex flex-wrap items-center justify-start gap-[0.36rem]">
        <div class="mr-[0.1rem] flex items-center gap-[0.2rem] border-r border-line/45 pr-[0.36rem]">
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
        <div class="mx-[0.14rem] h-[1.4rem] w-px bg-line/45" />

        <button
          type="button"
          class={`${headerIconButtonClass} border-danger-panel-strong bg-danger-panel text-danger-ink hover:bg-danger`}
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
