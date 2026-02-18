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

  return (
    <aside class="channel-rail">
      <header class="channel-rail-header">
        <button
          type="button"
          class="workspace-menu-trigger"
          aria-label="Open workspace menu"
          aria-haspopup="menu"
          aria-expanded={isWorkspaceMenuOpen()}
          onClick={toggleWorkspaceMenu}
          disabled={!props.activeWorkspace}
          ref={(element) => {
            workspaceMenuTriggerElement = element;
          }}
        >
          <h2>{props.activeWorkspace?.guildName ?? "No Workspace"}</h2>
          <span
            classList={{
              "workspace-menu-chevron": true,
              open: isWorkspaceMenuOpen(),
            }}
            aria-hidden="true"
          />
        </button>
        <Show when={props.activeWorkspace && isWorkspaceMenuOpen()}>
          <div
            class="workspace-menu"
            role="menu"
            aria-label="Workspace menu"
            ref={(element) => {
              workspaceMenuElement = element;
            }}
          >
            <button
              type="button"
              class="workspace-menu-item"
              role="menuitem"
              aria-label="Invite to workspace"
              onClick={closeWorkspaceMenu}
            >
              Invite to Workspace
            </button>
            <button
              type="button"
              class="workspace-menu-item"
              role="menuitem"
              aria-label="Open workspace settings panel"
              onClick={openWorkspaceSettingsPanel}
            >
              Server Settings
            </button>
            <div class="workspace-menu-divider" role="separator" aria-hidden="true" />
            <button
              type="button"
              class="workspace-menu-item workspace-menu-item-placeholder"
              role="menuitem"
              disabled
              aria-label="Notification settings coming soon"
            >
              <span>Notification Settings</span>
              <span class="workspace-menu-item-meta">Soon</span>
            </button>
            <button
              type="button"
              class="workspace-menu-item workspace-menu-item-placeholder"
              role="menuitem"
              disabled
              aria-label="Privacy settings coming soon"
            >
              <span>Privacy Settings</span>
              <span class="workspace-menu-item-meta">Soon</span>
            </button>
          </div>
        </Show>
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
                      {(() => {
                        const rosterEntries = () =>
                          props.voiceRosterEntriesForChannel(channel.channelId);
                        return (
                          <>
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
                      <Show
                        when={
                          props.isVoiceSessionForChannel(channel.channelId) ||
                          rosterEntries().length > 0
                        }
                      >
                        <section class="voice-channel-presence" aria-label="In-call participants">
                          <Show
                            when={rosterEntries().length > 0}
                            fallback={<p class="voice-channel-presence-empty">Waiting for participants...</p>}
                          >
                            <ul class="voice-channel-presence-tree">
                              <For each={rosterEntries()}>
                                {(entry) => (
                                  <li
                                    classList={{
                                      "voice-channel-presence-participant": true,
                                      "voice-channel-presence-participant-local": entry.isLocal,
                                      "voice-channel-presence-participant-speaking": entry.isSpeaking,
                                    }}
                                  >
                                    <Show
                                      when={props.userIdFromVoiceIdentity(entry.identity)}
                                      fallback={
                                        <span
                                          classList={{
                                            "voice-tree-avatar": true,
                                            "voice-tree-avatar-speaking": entry.isSpeaking,
                                          }}
                                          aria-hidden="true"
                                        >
                                          <span class="voice-tree-avatar-fallback">
                                            {actorAvatarGlyph(props.actorLabel(entry.identity))}
                                          </span>
                                        </span>
                                      }
                                    >
                                      {(participantUserId) => (
                                        <button
                                          type="button"
                                          class="voice-tree-avatar-button"
                                          aria-label={`Open ${props.voiceParticipantLabel(entry.identity, entry.isLocal)} profile`}
                                          onClick={() => props.onOpenUserProfile(participantUserId())}
                                        >
                                          <span
                                            classList={{
                                              "voice-tree-avatar": true,
                                              "voice-tree-avatar-speaking": entry.isSpeaking,
                                            }}
                                          >
                                            <span class="voice-tree-avatar-fallback" aria-hidden="true">
                                              {actorAvatarGlyph(props.actorLabel(entry.identity))}
                                            </span>
                                            <Show when={props.resolveAvatarUrl(participantUserId())}>
                                              <img
                                                class="voice-tree-avatar-image"
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
                                    <span class="voice-channel-presence-name">
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
                                        <span class="voice-channel-presence-badges">
                                          <Show when={entry.isMuted}>
                                            <span
                                              class="voice-participant-muted-badge"
                                              aria-label="Muted"
                                              title="Muted"
                                            >
                                              <span
                                                class="icon-mask"
                                                style={`--icon-url: url("${MUTE_MIC_ICON_URL}")`}
                                                aria-hidden="true"
                                              />
                                            </span>
                                          </Show>
                                          <Show when={entry.isDeafened}>
                                            <span
                                              class="voice-participant-deafened-badge"
                                              aria-label="Deafened"
                                              title="Deafened"
                                            >
                                              <span
                                                class="icon-mask"
                                                style={`--icon-url: url("${HEADPHONES_ICON_URL}")`}
                                                aria-hidden="true"
                                              />
                                            </span>
                                          </Show>
                                          <Show when={showLiveBadge}>
                                            <span class="voice-participant-media-badge">LIVE</span>
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
                            <div class="voice-channel-stream-hints" aria-label="Voice stream permission status">
                              <For each={props.voiceStreamPermissionHints}>{(hint) => <p>{hint}</p>}</For>
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
                        style={`--icon-url: url("${props.rtcSnapshot.isMicrophoneEnabled ? UNMUTE_MIC_ICON_URL : MUTE_MIC_ICON_URL}")`}
                        aria-hidden="true"
                      />
                    </button>
                    <button
                      type="button"
                      classList={{
                        "voice-dock-icon-button": true,
                        "is-busy": props.isTogglingVoiceDeaf,
                      }}
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
                        class="icon-mask"
                        style={`--icon-url: url("${HEADPHONES_ICON_URL}")`}
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
                        "leave-voice-button": true,
                        "voice-dock-disconnect-button": true,
                        danger: true,
                        "is-busy": props.isLeavingVoice,
                      }}
                      aria-label={props.isLeavingVoice ? "Disconnecting..." : "Disconnect"}
                      title={props.isLeavingVoice ? "Disconnecting..." : "Disconnect"}
                      onClick={props.onLeaveVoice}
                      disabled={props.isLeavingVoice || props.isJoiningVoice}
                    >
                      <span
                        class="icon-mask"
                        style={`--icon-url: url("${LEAVE_VOICE_ICON_URL}")`}
                        aria-hidden="true"
                      />
                      <span class="voice-dock-button-label" aria-hidden="true">
                        {props.isLeavingVoice ? "Disconnecting..." : "Disconnect"}
                      </span>
                    </button>
                  </Show>
                </div>
              </section>
            </Show>

            <footer class="channel-rail-account-bar" aria-label="Account controls">
              <div class="channel-rail-account-identity">
                <Show
                  when={props.currentUserId}
                  fallback={
                    <span class="channel-rail-account-avatar" aria-hidden="true">
                      <span class="channel-rail-account-avatar-fallback">
                        {actorAvatarGlyph(currentUserLabel())}
                      </span>
                    </span>
                  }
                >
                  {(currentUserId) => (
                    <button
                      type="button"
                      class="channel-rail-account-avatar-button"
                      aria-label={`Open ${currentUserLabel()} profile`}
                      onClick={() => props.onOpenUserProfile(currentUserId())}
                    >
                      <span class="channel-rail-account-avatar">
                        <span class="channel-rail-account-avatar-fallback" aria-hidden="true">
                          {actorAvatarGlyph(currentUserLabel())}
                        </span>
                        <Show when={props.resolveAvatarUrl(currentUserId())}>
                          <img
                            class="channel-rail-account-avatar-image"
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
                <div class="channel-rail-account-copy">
                  <p class="channel-rail-account-name">{currentUserLabel()}</p>
                  <p class="channel-rail-account-status">{currentUserStatusLabel()}</p>
                </div>
              </div>
              <button
                type="button"
                class="channel-rail-account-action"
                aria-label="Open client settings panel"
                title="Client settings"
                onClick={props.onOpenClientSettings}
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
