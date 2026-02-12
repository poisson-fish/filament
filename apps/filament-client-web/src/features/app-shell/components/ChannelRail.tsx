import { For, Match, Show, Switch } from "solid-js";
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
  voiceRosterEntries: VoiceRosterEntry[];
  voiceStreamPermissionHints: string[];
  activeVoiceSessionLabel: string;
  rtcSnapshot: RtcSnapshot;
  canToggleVoiceCamera: boolean;
  canToggleVoiceScreenShare: boolean;
  isJoiningVoice: boolean;
  isLeavingVoice: boolean;
  isTogglingVoiceMic: boolean;
  isTogglingVoiceCamera: boolean;
  isTogglingVoiceScreenShare: boolean;
  currentUserLabel?: string;
  currentUserStatusLabel?: string;
  actorLabel: (actorId: string) => string;
  voiceParticipantLabel: (identity: string, isLocal: boolean) => string;
  onOpenSettings: () => void;
  onCreateTextChannel: () => void;
  onCreateVoiceChannel: () => void;
  onSelectChannel: (channelId: ChannelId) => void;
  onJoinVoice: () => void;
  onToggleVoiceMicrophone: () => void;
  onToggleVoiceCamera: () => void;
  onToggleVoiceScreenShare: () => void;
  onLeaveVoice: () => void;
}

export function ChannelRail(props: ChannelRailProps) {
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

  return (
    <aside class="channel-rail">
      <header class="channel-rail-header">
        <h2>{props.activeWorkspace?.guildName ?? "No Workspace"}</h2>
      </header>
      <span class="channel-rail-subtitle">
        {props.activeWorkspace ? `${props.activeWorkspace.visibility} workspace` : "Hardened workspace"}
      </span>

      <Switch>
        <Match when={!props.activeWorkspace}>
          <p class="muted">Create a workspace to begin.</p>
        </Match>
        <Match when={props.activeWorkspace}>
          <div class="channel-rail-body">
            <nav aria-label="channels" class="channel-nav">
              <section class="channel-group">
                <div class="channel-group-header">
                  <p class="group-label">TEXT CHANNELS</p>
                  <Show when={props.canManageWorkspaceChannels}>
                    <button
                      type="button"
                      class="channel-group-action"
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
                      classList={{
                        active: props.activeChannelId === channel.channelId,
                        "channel-row": true,
                      }}
                      aria-label={channelRailLabel({ kind: channel.kind, name: channel.name })}
                      onClick={() => props.onSelectChannel(channel.channelId)}
                    >
                      <span class="channel-row-main">
                        <span class="channel-row-kind" aria-hidden="true">
                          #
                        </span>
                        <span>{channel.name}</span>
                      </span>
                    </button>
                  )}
                </For>
              </section>

              <section class="channel-group">
                <div class="channel-group-header">
                  <p class="group-label">VOICE CHANNELS</p>
                  <Show when={props.canManageWorkspaceChannels}>
                    <button
                      type="button"
                      class="channel-group-action"
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
                    <div class="voice-channel-entry">
                      <button
                        classList={{
                          active: props.activeChannelId === channel.channelId,
                          "channel-row": true,
                        }}
                        aria-label={channelRailLabel({ kind: channel.kind, name: channel.name })}
                        onClick={() => props.onSelectChannel(channel.channelId)}
                      >
                        <span class="channel-row-main">
                          <span class="channel-row-kind channel-row-kind-voice" aria-hidden="true">
                            VC
                          </span>
                          <span>{channel.name}</span>
                        </span>
                        <Show when={props.isVoiceSessionForChannel(channel.channelId)}>
                          <span class="channel-row-status">{props.voiceSessionDurationLabel}</span>
                        </Show>
                      </button>
                      <Show when={props.isVoiceSessionForChannel(channel.channelId)}>
                        <section class="voice-channel-presence" aria-label="In-call participants">
                          <Show
                            when={props.voiceRosterEntries.length > 0}
                            fallback={<p class="voice-channel-presence-empty">Waiting for participants...</p>}
                          >
                            <ul class="voice-channel-presence-tree">
                              <For each={props.voiceRosterEntries}>
                                {(entry) => (
                                  <li
                                    classList={{
                                      "voice-channel-presence-participant": true,
                                      "voice-channel-presence-participant-local": entry.isLocal,
                                      "voice-channel-presence-participant-speaking": entry.isSpeaking,
                                    }}
                                  >
                                    <span class="voice-tree-avatar" aria-hidden="true">
                                      {actorAvatarGlyph(props.actorLabel(entry.identity))}
                                    </span>
                                    <span
                                      classList={{
                                        "voice-channel-presence-name": true,
                                        "voice-channel-presence-name-speaking": entry.isSpeaking,
                                      }}
                                    >
                                      {props.voiceParticipantLabel(entry.identity, entry.isLocal)}
                                    </span>
                                    <span class="voice-channel-presence-badges">
                                      <Show when={entry.hasCamera}>
                                        <span class="voice-participant-media-badge video">Video</span>
                                      </Show>
                                      <Show when={entry.hasScreenShare}>
                                        <span class="voice-participant-media-badge screen">Share</span>
                                      </Show>
                                    </span>
                                  </li>
                                )}
                              </For>
                            </ul>
                          </Show>
                          <Show when={props.voiceStreamPermissionHints.length > 0}>
                            <div class="voice-channel-stream-hints" aria-label="Voice stream permission status">
                              <For each={props.voiceStreamPermissionHints}>{(hint) => <p>{hint}</p>}</For>
                            </div>
                          </Show>
                        </section>
                      </Show>
                    </div>
                  )}
                </For>
              </section>
            </nav>

            <Show when={props.canShowVoiceHeaderControls || props.isVoiceSessionActive}>
              <section class="voice-connected-dock" aria-label="Voice connected dock">
                <div class="voice-connected-dock-head">
                  <p class="voice-connected-dock-title">
                    {props.isVoiceSessionActive ? "Voice Connected" : "Voice Channel Ready"}
                  </p>
                  <Show when={props.isVoiceSessionActive}>
                    <span class="voice-connected-dock-duration">{props.voiceSessionDurationLabel}</span>
                  </Show>
                </div>
                <p class="voice-connected-dock-channel">
                  {props.isVoiceSessionActive ? props.activeVoiceSessionLabel : activeChannelLabel()}
                </p>
                <div class="voice-connected-dock-controls">
                  <Show when={props.canShowVoiceHeaderControls && !props.isVoiceSessionActive}>
                    <button
                      type="button"
                      classList={{
                        "voice-dock-icon-button": true,
                        "is-busy": props.isJoiningVoice,
                      }}
                      aria-label={props.isJoiningVoice ? "Joining..." : "Join Voice"}
                      title={props.isJoiningVoice ? "Joining..." : "Join Voice"}
                      onClick={props.onJoinVoice}
                      disabled={props.isJoiningVoice || props.isLeavingVoice}
                    >
                      <span
                        class="icon-mask"
                        style={`--icon-url: url("${JOIN_VOICE_ICON_URL}")`}
                        aria-hidden="true"
                      />
                    </button>
                  </Show>
                  <Show when={props.isVoiceSessionActive}>
                    <button
                      type="button"
                      classList={{
                        "voice-dock-icon-button": true,
                        "is-busy": props.isTogglingVoiceMic,
                      }}
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
                        class="icon-mask"
                        style={`--icon-url: url("${props.rtcSnapshot.isMicrophoneEnabled ? MUTE_MIC_ICON_URL : UNMUTE_MIC_ICON_URL}")`}
                        aria-hidden="true"
                      />
                    </button>
                    <button
                      type="button"
                      classList={{
                        "voice-dock-icon-button": true,
                        "is-busy": props.isTogglingVoiceCamera,
                      }}
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
                        class="icon-mask"
                        style={`--icon-url: url("${CAMERA_ICON_URL}")`}
                        aria-hidden="true"
                      />
                    </button>
                    <button
                      type="button"
                      classList={{
                        "voice-dock-icon-button": true,
                        "is-busy": props.isTogglingVoiceScreenShare,
                      }}
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
                        class="icon-mask"
                        style={`--icon-url: url("${props.rtcSnapshot.isScreenShareEnabled ? STOP_SCREEN_SHARE_ICON_URL : START_SCREEN_SHARE_ICON_URL}")`}
                        aria-hidden="true"
                      />
                    </button>
                    <button
                      type="button"
                      classList={{
                        "voice-dock-icon-button": true,
                        danger: true,
                        "is-busy": props.isLeavingVoice,
                      }}
                      aria-label={props.isLeavingVoice ? "Leaving..." : "Leave"}
                      title={props.isLeavingVoice ? "Leaving..." : "Leave"}
                      onClick={props.onLeaveVoice}
                      disabled={props.isLeavingVoice || props.isJoiningVoice}
                    >
                      <span
                        class="icon-mask"
                        style={`--icon-url: url("${LEAVE_VOICE_ICON_URL}")`}
                        aria-hidden="true"
                      />
                    </button>
                  </Show>
                </div>
              </section>
            </Show>

            <footer class="channel-rail-account-bar" aria-label="Account controls">
              <div class="channel-rail-account-identity">
                <span class="channel-rail-account-avatar" aria-hidden="true">
                  {actorAvatarGlyph(currentUserLabel())}
                </span>
                <div class="channel-rail-account-copy">
                  <p class="channel-rail-account-name">{currentUserLabel()}</p>
                  <p class="channel-rail-account-status">{currentUserStatusLabel()}</p>
                </div>
              </div>
              <button
                type="button"
                class="channel-rail-account-action"
                aria-label="Open settings panel"
                title="Settings"
                onClick={props.onOpenSettings}
              >
                <span class="icon-mask" style={`--icon-url: url("${SETTINGS_ICON_URL}")`} aria-hidden="true" />
              </button>
            </footer>
          </div>
        </Match>
      </Switch>
    </aside>
  );
}
