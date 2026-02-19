import {
  For,
  Match,
  Show,
  Switch,
  createEffect,
  createSignal,
  onCleanup,
  onMount,
} from "solid-js";
import type { ChannelId, ChannelRecord, WorkspaceRecord } from "../../../domain/chat";
import type { RtcSnapshot } from "../../../lib/rtc";
import { actorAvatarGlyph, channelHeaderLabel, channelRailLabel } from "../helpers";
import type { VoiceRosterEntry } from "../types";

const JOIN_VOICE_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/User/User_Voice.svg",
  import.meta.url,
).href;
const MUTE_MIC_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Media/Volume_Off.svg",
  import.meta.url,
).href;
const UNMUTE_MIC_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Media/Volume_Max.svg",
  import.meta.url,
).href;
const HEADPHONES_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Media/Headphones.svg",
  import.meta.url,
).href;
const CAMERA_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/System/Camera.svg",
  import.meta.url,
).href;
const START_SCREEN_SHARE_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/System/Monitor_Play.svg",
  import.meta.url,
).href;
const STOP_SCREEN_SHARE_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/System/Monitor.svg",
  import.meta.url,
).href;
const LEAVE_VOICE_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Interface/Log_Out.svg",
  import.meta.url,
).href;
const SETTINGS_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Interface/Settings.svg",
  import.meta.url,
).href;
const WORKSPACE_DROPDOWN_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Arrow/Caret_Down_SM.svg",
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
interface ChannelRailProps {
  activeWorkspace: WorkspaceRecord | null;
  activeChannel: ChannelRecord | null;
  activeChannelId: ChannelId | null;
  activeTextChannels: ChannelRecord[];
  activeVoiceChannels: ChannelRecord[];
  canManageWorkspaceChannels: boolean;
  canShowVoiceHeaderControls: boolean;
  isVoiceSessionActive: boolean;
  isVoiceSessionForChannel: (channelId: ChannelId) => boolean;
  voiceSessionDurationLabel: string;
  voiceRosterEntriesForChannel: (channelId: ChannelId) => VoiceRosterEntry[];
  voiceStreamPermissionHints: string[];
  activeVoiceSessionLabel: string;
  rtcSnapshot: RtcSnapshot;
  canToggleVoiceCamera: boolean;
  canToggleVoiceScreenShare: boolean;
  isJoiningVoice: boolean;
  isLeavingVoice: boolean;
  isTogglingVoiceMic: boolean;
  isTogglingVoiceDeaf: boolean;
  isTogglingVoiceCamera: boolean;
  isTogglingVoiceScreenShare: boolean;
  currentUserId?: string | null;
  currentUserLabel?: string;
  currentUserStatusLabel?: string;
  resolveAvatarUrl: (userId: string) => string | null;
  userIdFromVoiceIdentity: (identity: string) => string | null;
  actorLabel: (actorId: string) => string;
  voiceParticipantLabel: (identity: string, isLocal: boolean) => string;
  onOpenUserProfile: (userId: string) => void;
  onOpenClientSettings: () => void;
  onOpenWorkspaceSettings: () => void;
  onCreateTextChannel: () => void;
  onCreateVoiceChannel: () => void;
  onSelectChannel: (channelId: ChannelId) => void;
  onJoinVoice: () => void;
  onToggleVoiceMicrophone: () => void;
  onToggleVoiceDeafen: () => void;
  onToggleVoiceCamera: () => void;
  onToggleVoiceScreenShare: () => void;
  onLeaveVoice: () => void;
}

export function ChannelRail(props: ChannelRailProps) {
  const [isWorkspaceMenuOpen, setWorkspaceMenuOpen] = createSignal(false);
  let workspaceMenuTriggerElement: HTMLButtonElement | undefined;
  let workspaceMenuElement: HTMLDivElement | undefined;
  const currentUserLabel = () => props.currentUserLabel ?? "unknown-user";
  const currentUserStatusLabel = () => props.currentUserStatusLabel ?? "Online";
  const activeChannelLabel = () => {
    if (!props.activeChannel) {
      return "#no-channel";
    }
    return channelHeaderLabel({
      kind: props.activeChannel.kind,
      name: props.activeChannel.name,
    });
  };
  const closeWorkspaceMenu = () => {
    setWorkspaceMenuOpen(false);
  };
  const toggleWorkspaceMenu = () => {
    if (!props.activeWorkspace) {
      return;
    }
    setWorkspaceMenuOpen((open) => !open);
  };
  const openWorkspaceSettingsPanel = () => {
    closeWorkspaceMenu();
    props.onOpenWorkspaceSettings();
  };

  createEffect(() => {
    void props.activeWorkspace?.guildId;
    closeWorkspaceMenu();
  });

  onMount(() => {
    const onPointerDown = (event: PointerEvent) => {
      if (!isWorkspaceMenuOpen() || !workspaceMenuElement) {
        return;
      }
      if (!(event.target instanceof Node)) {
        return;
      }
      if (workspaceMenuTriggerElement?.contains(event.target)) {
        return;
      }
      if (!workspaceMenuElement.contains(event.target)) {
        closeWorkspaceMenu();
      }
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeWorkspaceMenu();
      }
    };

    window.addEventListener("pointerdown", onPointerDown);
    window.addEventListener("keydown", onKeyDown);
    onCleanup(() => {
      window.removeEventListener("pointerdown", onPointerDown);
      window.removeEventListener("keydown", onKeyDown);
    });
  });

  const workspaceMenuItemClass =
    "w-full min-h-[2rem] inline-flex items-center justify-between gap-[0.44rem] rounded-[0.5rem] border border-transparent bg-transparent px-[0.5rem] py-[0.4rem] text-left text-ink-1 enabled:cursor-pointer enabled:hover:border-line-soft enabled:hover:bg-bg-3 enabled:hover:text-ink-0 focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-[1px] focus-visible:outline-brand";
  const channelGroupActionClass =
    "inline-flex h-[1.2rem] w-[1.2rem] items-center justify-center rounded-[0.35rem] border-0 bg-transparent p-0 text-[1rem] leading-none text-ink-2 enabled:hover:bg-bg-3 enabled:hover:text-ink-0";
  const channelRowBaseClass =
    "w-full min-h-[2rem] inline-flex items-center justify-between gap-[0.4rem] rounded-[0.52rem] border-0 px-[0.52rem] py-[0.28rem] text-left transition-colors duration-[120ms] ease-out";
  const voiceDockIconButtonClass =
    "inline-flex h-[2.2rem] w-[2.45rem] items-center justify-center rounded-[0.56rem] border border-line-soft bg-bg-2 p-0 text-ink-0 enabled:hover:bg-bg-3 disabled:cursor-default disabled:opacity-58";
  const voiceDockIconMaskClass = "icon-mask h-[1.05rem] w-[1.05rem]";
  const accountAvatarClass =
    "relative inline-flex h-[1.7rem] w-[1.7rem] flex-shrink-0 items-center justify-center overflow-hidden rounded-full border border-line bg-bg-3 text-[0.58rem] text-ink-0 font-[800] tracking-[0.03em] uppercase";

  return (
    <aside class="channel-rail grid min-h-0 content-stretch gap-[0.5rem] bg-bg-1 px-[0.58rem] pt-[0.72rem] pb-[0.58rem] [grid-template-rows:auto_minmax(0,1fr)]">
      <div class="grid gap-[0.16rem]">
        <header class="relative flex items-center justify-between gap-[0.5rem]">
          <button
            type="button"
            class="w-full inline-flex min-h-[2.24rem] items-center justify-between gap-[0.48rem] rounded-[0.52rem] border border-transparent bg-transparent px-[0.52rem] py-[0.38rem] text-left text-ink-0 enabled:cursor-pointer enabled:hover:bg-bg-2 disabled:cursor-default focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-[1px] focus-visible:outline-brand"
            aria-label="Open workspace menu"
            aria-haspopup="menu"
            aria-expanded={isWorkspaceMenuOpen()}
            onClick={toggleWorkspaceMenu}
            disabled={!props.activeWorkspace}
            ref={(element) => {
              workspaceMenuTriggerElement = element;
            }}
          >
            <h2 class="m-0 truncate text-[1.03rem] text-ink-0 font-[780] tracking-[0.01em]">
              {props.activeWorkspace?.guildName ?? "No Workspace"}
            </h2>
            <span
              class="icon-mask h-[0.94rem] w-[0.94rem] shrink-0 text-ink-1 transition-transform duration-[140ms] ease-out"
              classList={{
                "rotate-0": !isWorkspaceMenuOpen(),
                "rotate-180": isWorkspaceMenuOpen(),
              }}
              style={`--icon-url: url("${WORKSPACE_DROPDOWN_ICON_URL}")`}
              aria-hidden="true"
            />
          </button>
          <Show when={props.activeWorkspace && isWorkspaceMenuOpen()}>
            <div
              class="absolute left-0 right-0 top-[calc(100%+0.44rem)] z-20 grid gap-[0.18rem] rounded-[0.76rem] border border-line bg-bg-1 p-[0.4rem] shadow-panel"
              role="menu"
              aria-label="Workspace menu"
              ref={(element) => {
                workspaceMenuElement = element;
              }}
            >
              <button
                type="button"
                class={workspaceMenuItemClass}
                role="menuitem"
                aria-label="Invite to workspace"
                onClick={closeWorkspaceMenu}
              >
                Invite to Workspace
              </button>
              <button
                type="button"
                class={workspaceMenuItemClass}
                role="menuitem"
                aria-label="Open workspace settings panel"
                onClick={openWorkspaceSettingsPanel}
              >
                Server Settings
              </button>
              <div class="mx-[0.06rem] my-[0.22rem] h-px bg-line" role="separator" aria-hidden="true" />
              <button
                type="button"
                class={`${workspaceMenuItemClass} text-ink-2 enabled:hover:bg-transparent enabled:hover:text-ink-2 disabled:cursor-not-allowed disabled:opacity-100`}
                role="menuitem"
                disabled
                aria-label="Notification settings coming soon"
              >
                <span>Notification Settings</span>
                <span class="text-[0.67rem] text-ink-2 tracking-[0.06em] uppercase">Soon</span>
              </button>
              <button
                type="button"
                class={`${workspaceMenuItemClass} text-ink-2 enabled:hover:bg-transparent enabled:hover:text-ink-2 disabled:cursor-not-allowed disabled:opacity-100`}
                role="menuitem"
                disabled
                aria-label="Privacy settings coming soon"
              >
                <span>Privacy Settings</span>
                <span class="text-[0.67rem] text-ink-2 tracking-[0.06em] uppercase">Soon</span>
              </button>
            </div>
          </Show>
        </header>
        <p class="m-0 px-[0.52rem] text-[0.74rem] text-ink-2 capitalize">
          {props.activeWorkspace
            ? `${props.activeWorkspace.visibility} workspace`
            : "Hardened workspace"}
        </p>
      </div>

      <Switch>
        <Match when={!props.activeWorkspace}>
          <p class="muted">Create a workspace to begin.</p>
        </Match>
        <Match when={props.activeWorkspace}>
          <div class="grid min-h-0 gap-[0.5rem] [grid-template-rows:minmax(0,1fr)_auto_auto]">
            <nav
              aria-label="channels"
              class="grid min-h-0 content-start gap-[0.86rem] overflow-auto pr-0"
            >
              <section class="mx-[-0.58rem] grid gap-[0.16rem]">
                <div class="channel-group-header flex min-h-[2.08rem] w-full items-center justify-between border-y border-line bg-bg-2 px-[1.08rem] py-[0.48rem]">
                  <p class="m-0 text-[0.73rem] text-ink-2 tracking-[0.08em] leading-none">
                    TEXT CHANNELS
                  </p>
                  <Show when={props.canManageWorkspaceChannels}>
                    <button
                      type="button"
                      class={channelGroupActionClass}
                      aria-label="Create text channel"
                      title="Create text channel"
                      onClick={props.onCreateTextChannel}
                    >
                      +
                    </button>
                  </Show>
                </div>
                <For each={props.activeTextChannels}>
                  {(channel) => (
                    <button
                      class={channelRowBaseClass}
                      classList={{
                        "bg-transparent text-ink-1 hover:bg-bg-3 hover:text-ink-0":
                          props.activeChannelId !== channel.channelId,
                        "bg-bg-4 text-ink-0": props.activeChannelId === channel.channelId,
                      }}
                      aria-label={channelRailLabel({ kind: channel.kind, name: channel.name })}
                      onClick={() => props.onSelectChannel(channel.channelId)}
                    >
                      <span class="inline-flex min-w-0 items-center gap-[0.45rem]">
                        <span
                          class="icon-mask h-[0.94rem] w-[0.94rem] shrink-0 pr-[0.08rem] text-ink-2"
                          style={`--icon-url: url("${TEXT_CHANNEL_ICON_URL}")`}
                          aria-hidden="true"
                        />
                        <span class="truncate">{channel.name}</span>
                      </span>
                    </button>
                  )}
                </For>
              </section>

              <section class="mx-[-0.58rem] grid gap-[0.16rem]">
                <div class="channel-group-header flex min-h-[2.08rem] w-full items-center justify-between border-y border-line bg-bg-2 px-[1.08rem] py-[0.48rem]">
                  <p class="m-0 text-[0.73rem] text-ink-2 tracking-[0.08em] leading-none">
                    VOICE CHANNELS
                  </p>
                  <Show when={props.canManageWorkspaceChannels}>
                    <button
                      type="button"
                      class={channelGroupActionClass}
                      aria-label="Create voice channel"
                      title="Create voice channel"
                      onClick={props.onCreateVoiceChannel}
                    >
                      +
                    </button>
                  </Show>
                </div>
                <For each={props.activeVoiceChannels}>
                  {(channel) => (
                    <div class="mx-[0.58rem]">
                      {(() => {
                        const rosterEntries = () =>
                          props.voiceRosterEntriesForChannel(channel.channelId);
                        return (
                          <>
                            <button
                              class={channelRowBaseClass}
                              classList={{
                                "bg-transparent text-ink-1 hover:bg-bg-3 hover:text-ink-0":
                                  props.activeChannelId !== channel.channelId,
                                "bg-bg-4 text-ink-0": props.activeChannelId === channel.channelId,
                              }}
                              aria-label={channelRailLabel({ kind: channel.kind, name: channel.name })}
                              onClick={() => props.onSelectChannel(channel.channelId)}
                            >
                              <span class="inline-flex min-w-0 items-center gap-[0.45rem]">
                                <span
                                  class="icon-mask h-[0.94rem] w-[0.94rem] shrink-0 pr-[0.08rem] text-ink-2"
                                  style={`--icon-url: url("${VOICE_CHANNEL_ICON_URL}")`}
                                  aria-hidden="true"
                                />
                                <span class="truncate">{channel.name}</span>
                              </span>
                              <Show when={props.isVoiceSessionForChannel(channel.channelId)}>
                                <span class="text-[0.82rem] text-ok tabular-nums">
                                  {props.voiceSessionDurationLabel}
                                </span>
                              </Show>
                            </button>
                            <Show
                              when={
                                props.isVoiceSessionForChannel(channel.channelId) ||
                                rosterEntries().length > 0
                              }
                            >
                              <section
                                class="ml-[1.62rem] mr-[0.2rem] mt-[0.08rem] border-l-2 border-line py-[0.08rem] pl-[0.54rem] pr-0"
                                aria-label="In-call participants"
                              >
                                <Show
                                  when={rosterEntries().length > 0}
                                  fallback={
                                    <p class="m-0 py-[0.16rem] text-[0.76rem] text-ink-2">
                                      Waiting for participants...
                                    </p>
                                  }
                                >
                                  <ul class="m-0 grid list-none gap-[0.12rem] p-0">
                                    <For each={rosterEntries()}>
                                      {(entry) => (
                                        <li
                                          class="relative flex min-h-[1.7rem] items-center gap-[0.36rem] rounded-[0.42rem] bg-transparent py-[0.14rem] pl-0 pr-[0.08rem] before:absolute before:left-[-0.54rem] before:top-1/2 before:h-px before:w-[0.46rem] before:bg-line before:content-[''] before:-translate-y-1/2"
                                          classList={{
                                            "bg-brand/10": entry.isLocal,
                                          }}
                                        >
                                          <Show
                                            when={props.userIdFromVoiceIdentity(entry.identity)}
                                            fallback={
                                              <span
                                                class={`${accountAvatarClass} h-[1.3rem] w-[1.3rem] text-[0.52rem] voice-tree-avatar`}
                                                classList={{
                                                  "voice-tree-avatar-speaking ring-2 ring-ok border-ok":
                                                    entry.isSpeaking,
                                                }}
                                                aria-hidden="true"
                                              >
                                                <span class="z-[1]">{actorAvatarGlyph(props.actorLabel(entry.identity))}</span>
                                              </span>
                                            }
                                          >
                                            {(participantUserId) => (
                                              <button
                                                type="button"
                                                class="rounded-full border-0 bg-transparent p-0"
                                                aria-label={`Open ${props.voiceParticipantLabel(entry.identity, entry.isLocal)} profile`}
                                                onClick={() => props.onOpenUserProfile(participantUserId())}
                                              >
                                                <span
                                                  class={`${accountAvatarClass} h-[1.3rem] w-[1.3rem] text-[0.52rem] voice-tree-avatar`}
                                                  classList={{
                                                    "voice-tree-avatar-speaking ring-2 ring-ok border-ok":
                                                      entry.isSpeaking,
                                                  }}
                                                >
                                                  <span class="z-[1]" aria-hidden="true">
                                                    {actorAvatarGlyph(props.actorLabel(entry.identity))}
                                                  </span>
                                                  <Show when={props.resolveAvatarUrl(participantUserId())}>
                                                    <img
                                                      class="absolute inset-0 z-[2] h-full w-full rounded-[inherit] object-cover"
                                                      src={props.resolveAvatarUrl(participantUserId())!}
                                                      alt={`${props.voiceParticipantLabel(entry.identity, entry.isLocal)} avatar`}
                                                      loading="lazy"
                                                      decoding="async"
                                                      referrerPolicy="no-referrer"
                                                      onError={(event) => {
                                                        event.currentTarget.style.display = "none";
                                                      }}
                                                    />
                                                  </Show>
                                                </span>
                                              </button>
                                            )}
                                          </Show>
                                          <span class="flex-1 truncate text-left text-[0.8rem] text-ink-1">
                                            {props.voiceParticipantLabel(entry.identity, entry.isLocal)}
                                          </span>
                                          {(() => {
                                            const participantUserId = props.userIdFromVoiceIdentity(
                                              entry.identity,
                                            );
                                            const isCurrentUserParticipant =
                                              Boolean(props.currentUserId) &&
                                              participantUserId === props.currentUserId;
                                            const showLiveBadge = isCurrentUserParticipant
                                              ? props.rtcSnapshot.isCameraEnabled ||
                                                props.rtcSnapshot.isScreenShareEnabled
                                              : entry.hasCamera || entry.hasScreenShare;
                                            return (
                                              <span class="ml-auto inline-flex shrink-0 items-center gap-[0.2rem]">
                                                <Show when={entry.isMuted}>
                                                  <span
                                                    class="inline-flex h-[1.15rem] w-[1.15rem] items-center justify-center rounded-[0.34rem] text-ink-2"
                                                    aria-label="Muted"
                                                    title="Muted"
                                                  >
                                                    <span
                                                      class="icon-mask h-[0.82rem] w-[0.82rem]"
                                                      style={`--icon-url: url("${MUTE_MIC_ICON_URL}")`}
                                                      aria-hidden="true"
                                                    />
                                                  </span>
                                                </Show>
                                                <Show when={entry.isDeafened}>
                                                  <span
                                                    class="inline-flex h-[1.15rem] w-[1.15rem] items-center justify-center rounded-[0.34rem] text-ink-1"
                                                    aria-label="Deafened"
                                                    title="Deafened"
                                                  >
                                                    <span
                                                      class="icon-mask h-[0.82rem] w-[0.82rem]"
                                                      style={`--icon-url: url("${HEADPHONES_ICON_URL}")`}
                                                      aria-hidden="true"
                                                    />
                                                  </span>
                                                </Show>
                                                <Show when={showLiveBadge}>
                                                  <span class="rounded-full border border-danger-panel-strong bg-danger-panel px-[0.34rem] py-[0.08rem] text-[0.64rem] text-danger-ink tracking-[0.03em] leading-[1.2] uppercase">
                                                    LIVE
                                                  </span>
                                                </Show>
                                              </span>
                                            );
                                          })()}
                                        </li>
                                      )}
                                    </For>
                                  </ul>
                                </Show>
                                <Show
                                  when={
                                    props.isVoiceSessionForChannel(channel.channelId) &&
                                    props.voiceStreamPermissionHints.length > 0
                                  }
                                >
                                  <div
                                    class="mt-[0.3rem] grid gap-[0.18rem] rounded-[0.5rem] border border-line bg-bg-1 px-[0.4rem] py-[0.34rem]"
                                    aria-label="Voice stream permission status"
                                  >
                                    <For each={props.voiceStreamPermissionHints}>
                                      {(hint) => <p class="m-0 text-[0.68rem] text-ink-1">{hint}</p>}
                                    </For>
                                  </div>
                                </Show>
                              </section>
                            </Show>
                          </>
                        );
                      })()}
                    </div>
                  )}
                </For>
              </section>
            </nav>

            <Show when={props.canShowVoiceHeaderControls || props.isVoiceSessionActive}>
              <section
                class="grid gap-[0.48rem] rounded-[0.72rem] border border-line-soft bg-bg-2 p-[0.62rem]"
                aria-label="Voice connected dock"
              >
                <div class="flex items-center justify-between gap-[0.5rem]">
                  <p class="m-0 text-ok font-[760] tracking-[0.01em]">
                    {props.isVoiceSessionActive ? "Voice Connected" : "Voice Channel Ready"}
                  </p>
                  <Show when={props.isVoiceSessionActive}>
                    <span class="text-[0.87rem] text-ok tabular-nums">
                      {props.voiceSessionDurationLabel}
                    </span>
                  </Show>
                </div>
                <p class="m-0 text-[0.88rem] text-ink-1 [overflow-wrap:anywhere]">
                  {props.isVoiceSessionActive ? props.activeVoiceSessionLabel : activeChannelLabel()}
                </p>
                <div class="flex flex-wrap gap-[0.36rem]">
                  <Show when={props.canShowVoiceHeaderControls && !props.isVoiceSessionActive}>
                    <button
                      type="button"
                      class={voiceDockIconButtonClass}
                      aria-label={props.isJoiningVoice ? "Joining..." : "Join Voice"}
                      title={props.isJoiningVoice ? "Joining..." : "Join Voice"}
                      onClick={props.onJoinVoice}
                      disabled={props.isJoiningVoice || props.isLeavingVoice}
                    >
                      <span
                        class={`${voiceDockIconMaskClass} ${props.isJoiningVoice ? "animate-[message-action-pulse_950ms_ease-in-out_infinite]" : ""}`}
                        style={`--icon-url: url("${JOIN_VOICE_ICON_URL}")`}
                        aria-hidden="true"
                      />
                    </button>
                  </Show>
                  <Show when={props.isVoiceSessionActive}>
                    <button
                      type="button"
                      class={voiceDockIconButtonClass}
                      aria-label={
                        props.isTogglingVoiceMic
                          ? "Updating..."
                          : props.rtcSnapshot.isMicrophoneEnabled
                            ? "Mute Mic"
                            : "Unmute Mic"
                      }
                      title={
                        props.isTogglingVoiceMic
                          ? "Updating..."
                          : props.rtcSnapshot.isMicrophoneEnabled
                            ? "Mute Mic"
                            : "Unmute Mic"
                      }
                      onClick={props.onToggleVoiceMicrophone}
                      disabled={
                        props.isTogglingVoiceMic ||
                        props.rtcSnapshot.connectionStatus !== "connected" ||
                        props.isJoiningVoice ||
                        props.isLeavingVoice
                      }
                    >
                      <span
                        class={`${voiceDockIconMaskClass} ${props.isTogglingVoiceMic ? "animate-[message-action-pulse_950ms_ease-in-out_infinite]" : ""}`}
                        style={`--icon-url: url("${props.rtcSnapshot.isMicrophoneEnabled ? UNMUTE_MIC_ICON_URL : MUTE_MIC_ICON_URL}")`}
                        aria-hidden="true"
                      />
                    </button>
                    <button
                      type="button"
                      class={voiceDockIconButtonClass}
                      aria-label={
                        props.isTogglingVoiceDeaf
                          ? "Updating..."
                          : props.rtcSnapshot.isDeafened
                            ? "Undeafen Audio"
                            : "Deafen Audio"
                      }
                      title={
                        props.isTogglingVoiceDeaf
                          ? "Updating..."
                          : props.rtcSnapshot.isDeafened
                            ? "Undeafen Audio"
                            : "Deafen Audio"
                      }
                      onClick={props.onToggleVoiceDeafen}
                      disabled={
                        props.isTogglingVoiceDeaf ||
                        props.rtcSnapshot.connectionStatus !== "connected" ||
                        props.isJoiningVoice ||
                        props.isLeavingVoice
                      }
                    >
                      <span
                        class={`${voiceDockIconMaskClass} ${props.isTogglingVoiceDeaf ? "animate-[message-action-pulse_950ms_ease-in-out_infinite]" : ""}`}
                        style={`--icon-url: url("${HEADPHONES_ICON_URL}")`}
                        aria-hidden="true"
                      />
                    </button>
                    <button
                      type="button"
                      class={voiceDockIconButtonClass}
                      aria-label={
                        props.isTogglingVoiceCamera
                          ? "Updating..."
                          : props.rtcSnapshot.isCameraEnabled
                            ? "Camera Off"
                            : "Camera On"
                      }
                      title={
                        props.isTogglingVoiceCamera
                          ? "Updating..."
                          : props.rtcSnapshot.isCameraEnabled
                            ? "Camera Off"
                            : "Camera On"
                      }
                      onClick={props.onToggleVoiceCamera}
                      disabled={
                        props.isTogglingVoiceCamera ||
                        props.rtcSnapshot.connectionStatus !== "connected" ||
                        props.isJoiningVoice ||
                        props.isLeavingVoice ||
                        !props.canToggleVoiceCamera
                      }
                    >
                      <span
                        class={`${voiceDockIconMaskClass} ${props.isTogglingVoiceCamera ? "animate-[message-action-pulse_950ms_ease-in-out_infinite]" : ""}`}
                        style={`--icon-url: url("${CAMERA_ICON_URL}")`}
                        aria-hidden="true"
                      />
                    </button>
                    <button
                      type="button"
                      class={voiceDockIconButtonClass}
                      aria-label={
                        props.isTogglingVoiceScreenShare
                          ? "Updating..."
                          : props.rtcSnapshot.isScreenShareEnabled
                            ? "Stop Share"
                            : "Share Screen"
                      }
                      title={
                        props.isTogglingVoiceScreenShare
                          ? "Updating..."
                          : props.rtcSnapshot.isScreenShareEnabled
                            ? "Stop Share"
                            : "Share Screen"
                      }
                      onClick={props.onToggleVoiceScreenShare}
                      disabled={
                        props.isTogglingVoiceScreenShare ||
                        props.rtcSnapshot.connectionStatus !== "connected" ||
                        props.isJoiningVoice ||
                        props.isLeavingVoice ||
                        !props.canToggleVoiceScreenShare
                      }
                    >
                      <span
                        class={`${voiceDockIconMaskClass} ${props.isTogglingVoiceScreenShare ? "animate-[message-action-pulse_950ms_ease-in-out_infinite]" : ""}`}
                        style={`--icon-url: url("${props.rtcSnapshot.isScreenShareEnabled ? STOP_SCREEN_SHARE_ICON_URL : START_SCREEN_SHARE_ICON_URL}")`}
                        aria-hidden="true"
                      />
                    </button>
                    <button
                      type="button"
                      class="inline-flex min-h-[2.35rem] w-full flex-[1_0_100%] items-center justify-center gap-[0.42rem] rounded-[0.56rem] border border-danger-panel-strong bg-danger-panel p-0 text-danger-ink enabled:hover:bg-danger disabled:cursor-default disabled:opacity-58"
                      aria-label={props.isLeavingVoice ? "Disconnecting..." : "Disconnect"}
                      title={props.isLeavingVoice ? "Disconnecting..." : "Disconnect"}
                      onClick={props.onLeaveVoice}
                      disabled={props.isLeavingVoice || props.isJoiningVoice}
                    >
                      <span
                        class={`${voiceDockIconMaskClass} ${props.isLeavingVoice ? "animate-[message-action-pulse_950ms_ease-in-out_infinite]" : ""}`}
                        style={`--icon-url: url("${LEAVE_VOICE_ICON_URL}")`}
                        aria-hidden="true"
                      />
                      <span class="text-[0.78rem] font-[700] tracking-[0.02em]" aria-hidden="true">
                        {props.isLeavingVoice ? "Disconnecting..." : "Disconnect"}
                      </span>
                    </button>
                  </Show>
                </div>
              </section>
            </Show>

            <footer
              class="flex items-center justify-start gap-[0.5rem] rounded-[0.72rem] border border-line-soft bg-bg-2 px-[0.52rem] py-[0.48rem]"
              aria-label="Account controls"
            >
              <div class="inline-flex min-w-0 items-center gap-[0.45rem]">
                <Show
                  when={props.currentUserId}
                  fallback={
                    <span class={accountAvatarClass} aria-hidden="true">
                      <span class="z-[1]">{actorAvatarGlyph(currentUserLabel())}</span>
                    </span>
                  }
                >
                  {(currentUserId) => (
                    <button
                      type="button"
                      class="rounded-full border-0 bg-transparent p-0"
                      aria-label={`Open ${currentUserLabel()} profile`}
                      onClick={() => props.onOpenUserProfile(currentUserId())}
                    >
                      <span class={accountAvatarClass}>
                        <span class="z-[1]" aria-hidden="true">
                          {actorAvatarGlyph(currentUserLabel())}
                        </span>
                        <Show when={props.resolveAvatarUrl(currentUserId())}>
                          <img
                            class="absolute inset-0 z-[2] h-full w-full rounded-[inherit] object-cover"
                            src={props.resolveAvatarUrl(currentUserId())!}
                            alt={`${currentUserLabel()} avatar`}
                            loading="lazy"
                            decoding="async"
                            referrerPolicy="no-referrer"
                            onLoad={(event) => {
                              event.currentTarget.style.display = "";
                            }}
                            onError={(event) => {
                              event.currentTarget.style.display = "none";
                            }}
                          />
                        </Show>
                      </span>
                    </button>
                  )}
                </Show>
                <div class="grid min-w-0 gap-[0.02rem]">
                  <p class="m-0 truncate text-[0.8rem] text-ink-0 font-[740]">
                    {currentUserLabel()}
                  </p>
                  <p class="m-0 text-[0.72rem] text-ink-2">{currentUserStatusLabel()}</p>
                </div>
              </div>
              <button
                type="button"
                class="ml-auto inline-flex h-[2.1rem] w-[2.1rem] shrink-0 items-center justify-center rounded-[0.56rem] border border-line-soft bg-bg-3 p-0 text-ink-0 enabled:hover:bg-bg-4"
                aria-label="Open client settings panel"
                title="Client settings"
                onClick={props.onOpenClientSettings}
              >
                <span
                  class="icon-mask h-[1.02rem] w-[1.02rem]"
                  style={`--icon-url: url("${SETTINGS_ICON_URL}")`}
                  aria-hidden="true"
                />
              </button>
            </footer>
          </div>
        </Match>
      </Switch>
    </aside>
  );
}
