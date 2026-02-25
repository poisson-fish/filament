import { createMemo, type Accessor } from "solid-js";
import type {
  AttachmentRecord,
  ChannelId,
  ChannelPermissionSnapshot,
  GuildId,
  MediaPublishSource,
  RoleName,
  UserId,
  WorkspaceRecord,
} from "../../../domain/chat";
import type { RtcConnectionStatus, RtcSnapshot } from "../../../lib/rtc";
import type { VoiceParticipantPayload } from "../../../lib/gateway";
import {
  channelKey,
  formatVoiceDuration,
  parseChannelKey,
  voiceConnectionLabel,
} from "../helpers";
import {
  createClientPermissionLayer,
  type ClientPermissionLayer,
} from "../permissions/client-permission-layer";
import type {
  WorkspaceChannelOverridesByGuildId,
  WorkspaceRolesByGuildId,
  WorkspaceUserRolesByGuildId,
} from "../state/workspace-state";
import type {
  OverlayPanel,
  VoiceRosterEntry,
  VoiceSessionCapabilities,
} from "../types";

const ACTIVE_VOICE_CONNECTION_STATES = new Set<RtcConnectionStatus>([
  "connecting",
  "connected",
  "reconnecting",
]);

export interface CreateAppShellSelectorsOptions {
  workspaces: Accessor<WorkspaceRecord[]>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  currentUserId: Accessor<UserId | null>;
  channelPermissions: Accessor<ChannelPermissionSnapshot | null>;
  workspaceRolesByGuildId: Accessor<WorkspaceRolesByGuildId>;
  workspaceUserRolesByGuildId: Accessor<WorkspaceUserRolesByGuildId>;
  workspaceChannelOverridesByGuildId: Accessor<WorkspaceChannelOverridesByGuildId>;
  viewAsRoleSimulatorEnabled: Accessor<boolean>;
  viewAsRoleSimulatorRole: Accessor<RoleName>;
  voiceSessionChannelKey: Accessor<string | null>;
  attachmentByChannel: Accessor<Record<string, AttachmentRecord[]>>;
  rtcSnapshot: Accessor<RtcSnapshot>;
  voiceParticipantsByChannel: Accessor<Record<string, VoiceParticipantPayload[]>>;
  voiceSessionCapabilities: Accessor<VoiceSessionCapabilities>;
  voiceSessionStartedAtUnixMs: Accessor<number | null>;
  voiceDurationClockUnixMs: Accessor<number>;
  activeOverlayPanel: Accessor<OverlayPanel | null>;
  permissionLayer?: ClientPermissionLayer;
}

export interface CreateAppShellSelectorsResult {
  activeWorkspace: Accessor<WorkspaceRecord | null>;
  activeChannel: Accessor<WorkspaceRecord["channels"][number] | null>;
  activeTextChannels: Accessor<WorkspaceRecord["channels"]>;
  activeVoiceChannels: Accessor<WorkspaceRecord["channels"]>;
  canAccessActiveChannel: Accessor<boolean>;
  canPublishVoiceCamera: Accessor<boolean>;
  canPublishVoiceScreenShare: Accessor<boolean>;
  canSubscribeVoiceStreams: Accessor<boolean>;
  canManageWorkspaceChannels: Accessor<boolean>;
  canManageSearchMaintenance: Accessor<boolean>;
  canManageWorkspaceRoles: Accessor<boolean>;
  canManageMemberRoles: Accessor<boolean>;
  hasRoleManagementAccess: Accessor<boolean>;
  canManageRoles: Accessor<boolean>;
  canManageChannelOverrides: Accessor<boolean>;
  canBanMembers: Accessor<boolean>;
  canDeleteMessages: Accessor<boolean>;
  hasModerationAccess: Accessor<boolean>;
  canDismissWorkspaceCreateForm: Accessor<boolean>;
  activeVoiceSessionLabel: Accessor<string>;
  activeAttachments: Accessor<AttachmentRecord[]>;
  voiceConnectionState: Accessor<string>;
  isVoiceSessionActive: Accessor<boolean>;
  isVoiceSessionForActiveChannel: Accessor<boolean>;
  isVoiceSessionForChannel: (channelId: ChannelId) => boolean;
  voiceRosterEntriesForChannel: (channelId: ChannelId) => VoiceRosterEntry[];
  canToggleVoiceCamera: Accessor<boolean>;
  canToggleVoiceScreenShare: Accessor<boolean>;
  canShowVoiceHeaderControls: Accessor<boolean>;
  voiceRosterEntries: Accessor<VoiceRosterEntry[]>;
  voiceStreamPermissionHints: Accessor<string[]>;
  voiceSessionDurationLabel: Accessor<string>;
  canCloseActivePanel: Accessor<boolean>;
}

export interface VoiceStreamPermissionHintInput {
  isVoiceSessionForActiveChannel: boolean;
  canPublishVoiceCamera: boolean;
  canPublishVoiceScreenShare: boolean;
  canSubscribeVoiceStreams: boolean;
  voiceSessionCapabilities: VoiceSessionCapabilities;
}

function hasVoicePublishGrant(
  capabilities: VoiceSessionCapabilities,
  source: MediaPublishSource,
): boolean {
  return capabilities.publishSources.includes(source);
}

function isVoiceConnectionActive(status: RtcConnectionStatus): boolean {
  return ACTIVE_VOICE_CONNECTION_STATES.has(status);
}

export function buildVoiceRosterEntries(snapshot: RtcSnapshot): VoiceRosterEntry[] {
  const entries: VoiceRosterEntry[] = [];
  const seenIdentities = new Set<string>();
  const activeSpeakers = new Set(snapshot.activeSpeakerIdentities);
  const identitiesWithCamera = new Set<string>();
  const identitiesWithScreenShare = new Set<string>();

  for (const track of snapshot.videoTracks) {
    if (track.source === "camera") {
      identitiesWithCamera.add(track.participantIdentity);
    } else if (track.source === "screen_share") {
      identitiesWithScreenShare.add(track.participantIdentity);
    }
  }

  const localIdentity = snapshot.localParticipantIdentity;
  if (localIdentity) {
    entries.push({
      identity: localIdentity,
      isLocal: true,
      isMuted: !snapshot.isMicrophoneEnabled,
      isDeafened: snapshot.isDeafened,
      isSpeaking: activeSpeakers.has(localIdentity),
      hasCamera: identitiesWithCamera.has(localIdentity),
      hasScreenShare: identitiesWithScreenShare.has(localIdentity),
    });
    seenIdentities.add(localIdentity);
  }

  for (const participant of snapshot.participants) {
    if (seenIdentities.has(participant.identity)) {
      continue;
    }
    entries.push({
      identity: participant.identity,
      isLocal: false,
      isMuted: false,
      isDeafened: false,
      isSpeaking: activeSpeakers.has(participant.identity),
      hasCamera: identitiesWithCamera.has(participant.identity),
      hasScreenShare: identitiesWithScreenShare.has(participant.identity),
    });
    seenIdentities.add(participant.identity);
  }

  return entries;
}

function localMediaState(snapshot: RtcSnapshot): {
  isMuted: boolean;
  isDeafened: boolean;
  hasCamera: boolean;
  hasScreenShare: boolean;
} {
  return {
    isMuted: !snapshot.isMicrophoneEnabled,
    isDeafened: snapshot.isDeafened,
    hasCamera: snapshot.videoTracks.some((track) => track.isLocal && track.source === "camera"),
    hasScreenShare: snapshot.videoTracks.some(
      (track) => track.isLocal && track.source === "screen_share",
    ),
  };
}

export function buildVoiceStreamPermissionHints(input: VoiceStreamPermissionHintInput): string[] {
  if (!input.isVoiceSessionForActiveChannel) {
    return [];
  }

  const hints: string[] = [];
  if (!input.canPublishVoiceCamera) {
    hints.push("Camera disabled: channel permission publish_video is missing.");
  } else if (!hasVoicePublishGrant(input.voiceSessionCapabilities, "camera")) {
    hints.push("Camera disabled: this voice token did not grant camera publish.");
  }
  if (!input.canPublishVoiceScreenShare) {
    hints.push("Screen share disabled: channel permission publish_screen_share is missing.");
  } else if (!hasVoicePublishGrant(input.voiceSessionCapabilities, "screen_share")) {
    hints.push("Screen share disabled: this voice token did not grant screen publish.");
  }
  if (!input.canSubscribeVoiceStreams) {
    hints.push("Remote stream subscription is denied by channel permission.");
  } else if (!input.voiceSessionCapabilities.canSubscribe) {
    hints.push("Remote stream subscription is denied for this call.");
  }
  return hints;
}

export function createAppShellSelectors(
  options: CreateAppShellSelectorsOptions,
): CreateAppShellSelectorsResult {
  const permissionLayer = options.permissionLayer ??
    createClientPermissionLayer({
      activeGuildId: options.activeGuildId,
      activeChannelId: options.activeChannelId,
      currentUserId: options.currentUserId,
      channelPermissions: options.channelPermissions,
      workspaceRolesByGuildId: options.workspaceRolesByGuildId,
      workspaceUserRolesByGuildId: options.workspaceUserRolesByGuildId,
      workspaceChannelOverridesByGuildId: options.workspaceChannelOverridesByGuildId,
      viewAsRoleSimulatorEnabled: options.viewAsRoleSimulatorEnabled,
      viewAsRoleSimulatorRole: options.viewAsRoleSimulatorRole,
    });

  const activeWorkspace = createMemo(
    () =>
      options.workspaces().find((workspace) => workspace.guildId === options.activeGuildId()) ??
      null,
  );

  const activeChannel = createMemo(
    () =>
      activeWorkspace()?.channels.find(
        (channel) => channel.channelId === options.activeChannelId(),
      ) ?? null,
  );

  const activeTextChannels = createMemo(() =>
    (activeWorkspace()?.channels ?? []).filter((channel) => channel.kind === "text"),
  );
  const activeVoiceChannels = createMemo(() =>
    (activeWorkspace()?.channels ?? []).filter((channel) => channel.kind === "voice"),
  );
  const isActiveVoiceChannel = createMemo(() => activeChannel()?.kind === "voice");

  const canAccessActiveChannel = permissionLayer.canAccessActiveChannel;
  const canPublishVoiceCamera = permissionLayer.canPublishVoiceCamera;
  const canPublishVoiceScreenShare = permissionLayer.canPublishVoiceScreenShare;
  const canSubscribeVoiceStreams = permissionLayer.canSubscribeVoiceStreams;
  const canManageWorkspaceChannels = permissionLayer.canManageWorkspaceChannels;
  const canManageSearchMaintenance = permissionLayer.canManageSearchMaintenance;
  const canManageWorkspaceRoles = permissionLayer.canManageWorkspaceRoles;
  const canManageMemberRoles = permissionLayer.canManageMemberRoles;
  const hasRoleManagementAccess = permissionLayer.hasRoleManagementAccess;
  const canManageRoles = permissionLayer.canManageRoles;
  const canManageChannelOverrides = permissionLayer.canManageChannelOverrides;
  const canBanMembers = permissionLayer.canBanMembers;
  const canDeleteMessages = permissionLayer.canDeleteMessages;
  const hasModerationAccess = permissionLayer.hasModerationAccess;
  const canDismissWorkspaceCreateForm = createMemo(() => options.workspaces().length > 0);

  const activeChannelKey = createMemo(() => {
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    return guildId && channelId ? channelKey(guildId, channelId) : null;
  });

  const activeVoiceSession = createMemo(() => {
    const key = options.voiceSessionChannelKey();
    return key ? parseChannelKey(key) : null;
  });

  const activeVoiceWorkspace = createMemo(() => {
    const voiceSession = activeVoiceSession();
    if (!voiceSession) {
      return null;
    }
    return (
      options
        .workspaces()
        .find((workspace) => workspace.guildId === voiceSession.guildId) ?? null
    );
  });

  const activeVoiceSessionChannel = createMemo(() => {
    const voiceSession = activeVoiceSession();
    const workspace = activeVoiceWorkspace();
    if (!voiceSession || !workspace) {
      return null;
    }
    return (
      workspace.channels.find(
        (channel) =>
          channel.channelId === voiceSession.channelId && channel.kind === "voice",
      ) ?? null
    );
  });

  const activeVoiceSessionLabel = createMemo(() => {
    const workspace = activeVoiceWorkspace();
    const channel = activeVoiceSessionChannel();
    if (workspace && channel) {
      return `${channel.name} / ${workspace.guildName}`;
    }
    if (channel) {
      return channel.name;
    }
    return "Unknown voice room";
  });

  const activeAttachments = createMemo(() => {
    const key = activeChannelKey();
    if (!key) {
      return [];
    }
    return options.attachmentByChannel()[key] ?? [];
  });

  const voiceConnectionState = createMemo(() => voiceConnectionLabel(options.rtcSnapshot()));

  const isVoiceSessionActive = createMemo(() =>
    isVoiceConnectionActive(options.rtcSnapshot().connectionStatus),
  );

  const isVoiceSessionForActiveChannel = createMemo(() => {
    const key = activeChannelKey();
    return (
      Boolean(key) &&
      key === options.voiceSessionChannelKey() &&
      isVoiceSessionActive()
    );
  });

  const isVoiceSessionForChannel = (channelId: ChannelId): boolean => {
    const guildId = options.activeGuildId();
    if (!guildId || !isVoiceSessionActive()) {
      return false;
    }
    return options.voiceSessionChannelKey() === channelKey(guildId, channelId);
  };

  const canToggleVoiceCamera = createMemo(
    () =>
      isVoiceSessionActive() &&
      canPublishVoiceCamera() &&
      hasVoicePublishGrant(options.voiceSessionCapabilities(), "camera"),
  );

  const canToggleVoiceScreenShare = createMemo(
    () =>
      isVoiceSessionActive() &&
      canPublishVoiceScreenShare() &&
      hasVoicePublishGrant(options.voiceSessionCapabilities(), "screen_share"),
  );

  const canShowVoiceHeaderControls = createMemo(
    () => isActiveVoiceChannel() && canAccessActiveChannel(),
  );

  const voiceRosterEntriesForChannel = (channelId: ChannelId): VoiceRosterEntry[] => {
    const activeGuildId = options.activeGuildId();
    if (!activeGuildId) {
      return [];
    }
    const key = channelKey(activeGuildId, channelId);
    const isActiveVoiceSessionChannel =
      isVoiceSessionActive() && options.voiceSessionChannelKey() === key;
    const snapshot = options.rtcSnapshot();
    const synced = options.voiceParticipantsByChannel()[key];
    if (synced && synced.length > 0) {
      const localIdentity = snapshot.localParticipantIdentity;
      const localMedia = localMediaState(snapshot);
      const activeSpeakers = new Set(snapshot.activeSpeakerIdentities);
      const mapped = synced.map((entry) => ({
        identity: entry.identity,
        isLocal: entry.identity === localIdentity,
        isMuted:
          entry.identity === localIdentity
            ? localMedia.isMuted
            : Boolean(entry.isMuted),
        isDeafened:
          entry.identity === localIdentity
            ? localMedia.isDeafened
            : Boolean(entry.isDeafened),
        isSpeaking:
          (isActiveVoiceSessionChannel && activeSpeakers.has(entry.identity)) ||
          (!isActiveVoiceSessionChannel && entry.isSpeaking),
        hasCamera: entry.identity === localIdentity ? localMedia.hasCamera : entry.isVideoEnabled,
        hasScreenShare:
          entry.identity === localIdentity
            ? localMedia.hasScreenShare
            : entry.isScreenShareEnabled,
      }));
      if (!isActiveVoiceSessionChannel) {
        return mapped;
      }
      const mergedByIdentity = new Map(mapped.map((entry) => [entry.identity, entry]));
      for (const rtcEntry of buildVoiceRosterEntries(snapshot)) {
        const existing = mergedByIdentity.get(rtcEntry.identity);
        if (!existing) {
          mergedByIdentity.set(rtcEntry.identity, rtcEntry);
          continue;
        }
        mergedByIdentity.set(rtcEntry.identity, {
          ...existing,
          isLocal: existing.isLocal || rtcEntry.isLocal,
          isMuted: rtcEntry.isLocal ? rtcEntry.isMuted : existing.isMuted,
          isDeafened: rtcEntry.isLocal ? rtcEntry.isDeafened : existing.isDeafened,
          isSpeaking: rtcEntry.isSpeaking,
          hasCamera: existing.hasCamera || rtcEntry.hasCamera,
          hasScreenShare: existing.hasScreenShare || rtcEntry.hasScreenShare,
        });
      }
      return [...mergedByIdentity.values()];
    }
    if (isActiveVoiceSessionChannel) {
      return buildVoiceRosterEntries(snapshot);
    }
    return [];
  };

  const voiceRosterEntries = createMemo<VoiceRosterEntry[]>(() =>
    (() => {
      const activeChannelId = options.activeChannelId();
      if (activeChannelId) {
        return voiceRosterEntriesForChannel(activeChannelId);
      }
      return [];
    })(),
  );

  const voiceStreamPermissionHints = createMemo(() =>
    buildVoiceStreamPermissionHints({
      isVoiceSessionForActiveChannel: isVoiceSessionForActiveChannel(),
      canPublishVoiceCamera: canPublishVoiceCamera(),
      canPublishVoiceScreenShare: canPublishVoiceScreenShare(),
      canSubscribeVoiceStreams: canSubscribeVoiceStreams(),
      voiceSessionCapabilities: options.voiceSessionCapabilities(),
    }),
  );

  const voiceSessionDurationLabel = createMemo(() => {
    if (!isVoiceSessionActive()) {
      return "0:00";
    }
    const startedAt = options.voiceSessionStartedAtUnixMs();
    if (!startedAt) {
      return "0:00";
    }
    const elapsedSeconds = Math.floor(
      (options.voiceDurationClockUnixMs() - startedAt) / 1000,
    );
    return formatVoiceDuration(elapsedSeconds);
  });

  const canCloseActivePanel = createMemo(() => {
    if (options.activeOverlayPanel() !== "workspace-create") {
      return true;
    }
    return canDismissWorkspaceCreateForm();
  });

  return {
    activeWorkspace,
    activeChannel,
    activeTextChannels,
    activeVoiceChannels,
    canAccessActiveChannel,
    canPublishVoiceCamera,
    canPublishVoiceScreenShare,
    canSubscribeVoiceStreams,
    canManageWorkspaceChannels,
    canManageSearchMaintenance,
    canManageWorkspaceRoles,
    canManageMemberRoles,
    hasRoleManagementAccess,
    canManageRoles,
    canManageChannelOverrides,
    canBanMembers,
    canDeleteMessages,
    hasModerationAccess,
    canDismissWorkspaceCreateForm,
    activeVoiceSessionLabel,
    activeAttachments,
    voiceConnectionState,
    isVoiceSessionActive,
    isVoiceSessionForActiveChannel,
    isVoiceSessionForChannel,
    voiceRosterEntriesForChannel,
    canToggleVoiceCamera,
    canToggleVoiceScreenShare,
    canShowVoiceHeaderControls,
    voiceRosterEntries,
    voiceStreamPermissionHints,
    voiceSessionDurationLabel,
    canCloseActivePanel,
  };
}
