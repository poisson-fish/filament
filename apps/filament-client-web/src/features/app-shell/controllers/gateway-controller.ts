import { createEffect, onCleanup, type Accessor, type Setter } from "solid-js";
import type { AccessToken, AuthSession } from "../../../domain/auth";
import type {
  ChannelRecord,
  ChannelId,
  GuildId,
  MessageId,
  MessageRecord,
  ReactionEmoji,
  WorkspaceRecord,
} from "../../../domain/chat";
import {
  connectGateway,
  type WorkspaceChannelOverrideUpdatePayload,
  type WorkspaceIpBanSyncPayload,
  type WorkspaceRoleAssignmentAddPayload,
  type WorkspaceRoleAssignmentRemovePayload,
  type WorkspaceRoleCreatePayload,
  type WorkspaceRoleDeletePayload,
  type WorkspaceRoleReorderPayload,
  type WorkspaceRoleUpdatePayload,
  type MessageDeletePayload,
  type MessageReactionPayload,
  type MessageUpdatePayload,
  type WorkspaceMemberBanPayload,
  type WorkspaceMemberRemovePayload,
  type WorkspaceUpdatePayload,
} from "../../../lib/gateway";
import {
  clearKeysByPrefix,
  mergeMessage,
  reactionKey,
  upsertWorkspace,
  upsertReactionEntry,
  type ReactionView,
} from "../helpers";

export interface GatewayControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  canAccessActiveChannel: Accessor<boolean>;
  setGatewayOnline: Setter<boolean>;
  setOnlineMembers: Setter<string[]>;
  setWorkspaces: Setter<WorkspaceRecord[]>;
  setMessages: Setter<MessageRecord[]>;
  setReactionState: Setter<Record<string, ReactionView>>;
  isMessageListNearBottom: () => boolean;
  scrollMessageListToBottom: () => void;
  onWorkspacePermissionsChanged?: (guildId: GuildId) => void;
  onWorkspaceModerationChanged?: (payload: WorkspaceIpBanSyncPayload) => void;
}

interface GatewayClient {
  close: () => void;
}

interface GatewayHandlers {
  onReady: (payload: { userId: string }) => void;
  onOpenStateChange: (isOpen: boolean) => void;
  onMessageCreate: (message: MessageRecord) => void;
  onMessageUpdate: (payload: MessageUpdatePayload) => void;
  onMessageDelete: (payload: MessageDeletePayload) => void;
  onMessageReaction: (payload: MessageReactionPayload) => void;
  onChannelCreate: (payload: {
    guildId: GuildId;
    channel: ChannelRecord;
  }) => void;
  onWorkspaceUpdate: (payload: WorkspaceUpdatePayload) => void;
  onWorkspaceMemberAdd: (_payload: unknown) => void;
  onWorkspaceMemberUpdate: (_payload: unknown) => void;
  onWorkspaceMemberRemove: (payload: WorkspaceMemberRemovePayload) => void;
  onWorkspaceMemberBan: (payload: WorkspaceMemberBanPayload) => void;
  onWorkspaceRoleCreate: (payload: WorkspaceRoleCreatePayload) => void;
  onWorkspaceRoleUpdate: (payload: WorkspaceRoleUpdatePayload) => void;
  onWorkspaceRoleDelete: (payload: WorkspaceRoleDeletePayload) => void;
  onWorkspaceRoleReorder: (payload: WorkspaceRoleReorderPayload) => void;
  onWorkspaceRoleAssignmentAdd: (payload: WorkspaceRoleAssignmentAddPayload) => void;
  onWorkspaceRoleAssignmentRemove: (payload: WorkspaceRoleAssignmentRemovePayload) => void;
  onWorkspaceChannelOverrideUpdate: (payload: WorkspaceChannelOverrideUpdatePayload) => void;
  onWorkspaceIpBanSync: (payload: WorkspaceIpBanSyncPayload) => void;
  onPresenceSync: (payload: { guildId: GuildId; userIds: string[] }) => void;
  onPresenceUpdate: (payload: {
    guildId: GuildId;
    userId: string;
    status: "online" | "offline";
  }) => void;
}

export interface GatewayControllerDependencies {
  connectGateway: (
    accessToken: AccessToken,
    guildId: GuildId,
    channelId: ChannelId,
    handlers: GatewayHandlers,
  ) => GatewayClient;
}

export function applyMessageReactionUpdate(
  existing: Record<string, ReactionView>,
  payload: {
    messageId: MessageId;
    emoji: ReactionEmoji;
    count: number;
  },
): Record<string, ReactionView> {
  const key = reactionKey(payload.messageId, payload.emoji);
  const nextReacted =
    payload.count === 0 ? false : (existing[key]?.reacted ?? false);
  return upsertReactionEntry(existing, key, {
    count: payload.count,
    reacted: nextReacted,
  });
}

export function applyMessageUpdate(
  existing: MessageRecord[],
  payload: MessageUpdatePayload,
): MessageRecord[] {
  const index = existing.findIndex((entry) => entry.messageId === payload.messageId);
  if (index < 0) {
    return existing;
  }
  const current = existing[index]!;
  const updated: MessageRecord = {
    ...current,
    content: payload.updatedFields.content ?? current.content,
    markdownTokens: payload.updatedFields.markdownTokens ?? current.markdownTokens,
  };
  return mergeMessage(existing, updated);
}

export function applyMessageDelete(
  existing: MessageRecord[],
  payload: MessageDeletePayload,
): MessageRecord[] {
  return existing.filter((entry) => entry.messageId !== payload.messageId);
}

const DEFAULT_GATEWAY_CONTROLLER_DEPENDENCIES: GatewayControllerDependencies = {
  connectGateway,
};

export function applyPresenceUpdate(
  existing: string[],
  payload: {
    userId: string;
    status: "online" | "offline";
  },
): string[] {
  if (payload.status === "online") {
    return existing.includes(payload.userId)
      ? existing
      : [...existing, payload.userId];
  }
  return existing.filter((entry) => entry !== payload.userId);
}

export function applyChannelCreate(
  existing: WorkspaceRecord[],
  payload: {
    guildId: GuildId;
    channel: ChannelRecord;
  },
): WorkspaceRecord[] {
  return upsertWorkspace(existing, payload.guildId, (workspace) => {
    if (
      workspace.channels.some(
        (channel) => channel.channelId === payload.channel.channelId,
      )
    ) {
      return workspace;
    }
    return {
      ...workspace,
      channels: [...workspace.channels, payload.channel],
    };
  });
}

export function applyWorkspaceUpdate(
  existing: WorkspaceRecord[],
  payload: WorkspaceUpdatePayload,
): WorkspaceRecord[] {
  return upsertWorkspace(existing, payload.guildId, (workspace) => ({
    ...workspace,
    guildName: payload.updatedFields.name ?? workspace.guildName,
    visibility: payload.updatedFields.visibility ?? workspace.visibility,
  }));
}

function removeWorkspace(existing: WorkspaceRecord[], guildId: GuildId): WorkspaceRecord[] {
  return existing.filter((workspace) => workspace.guildId !== guildId);
}

export function createGatewayController(
  options: GatewayControllerOptions,
  dependencies: Partial<GatewayControllerDependencies> = {},
): void {
  const deps = {
    ...DEFAULT_GATEWAY_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };
  let connectedUserId: string | null = null;

  createEffect(() => {
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    if (!session || !guildId || !channelId || !options.canAccessActiveChannel()) {
      options.setGatewayOnline(false);
      options.setOnlineMembers([]);
      return;
    }

    const gateway = deps.connectGateway(session.accessToken, guildId, channelId, {
      onReady: (payload) => {
        connectedUserId = payload.userId;
      },
      onOpenStateChange: (isOpen) => options.setGatewayOnline(isOpen),
      onMessageCreate: (message) => {
        if (message.guildId !== guildId || message.channelId !== channelId) {
          return;
        }
        const shouldStickToBottom = options.isMessageListNearBottom();
        options.setMessages((existing) => mergeMessage(existing, message));
        if (shouldStickToBottom) {
          options.scrollMessageListToBottom();
        }
      },
      onMessageUpdate: (payload) => {
        if (payload.guildId !== guildId || payload.channelId !== channelId) {
          return;
        }
        options.setMessages((existing) => applyMessageUpdate(existing, payload));
      },
      onMessageDelete: (payload) => {
        if (payload.guildId !== guildId || payload.channelId !== channelId) {
          return;
        }
        options.setMessages((existing) => applyMessageDelete(existing, payload));
        options.setReactionState((existing) =>
          clearKeysByPrefix(existing, `${payload.messageId}|`),
        );
      },
      onMessageReaction: (payload) => {
        if (payload.guildId !== guildId || payload.channelId !== channelId) {
          return;
        }
        options.setReactionState((existing) =>
          applyMessageReactionUpdate(existing, payload),
        );
      },
      onChannelCreate: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        options.setWorkspaces((existing) => applyChannelCreate(existing, payload));
      },
      onWorkspaceUpdate: (payload) => {
        options.setWorkspaces((existing) => applyWorkspaceUpdate(existing, payload));
      },
      onWorkspaceMemberAdd: () => {},
      onWorkspaceMemberUpdate: () => {},
      onWorkspaceMemberRemove: (payload) => {
        if (payload.userId !== connectedUserId) {
          return;
        }
        options.setWorkspaces((existing) => removeWorkspace(existing, payload.guildId));
      },
      onWorkspaceMemberBan: (payload) => {
        if (payload.userId !== connectedUserId) {
          return;
        }
        options.setWorkspaces((existing) => removeWorkspace(existing, payload.guildId));
      },
      onWorkspaceRoleCreate: (payload) => {
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceRoleUpdate: (payload) => {
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceRoleDelete: (payload) => {
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceRoleReorder: (payload) => {
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceRoleAssignmentAdd: (payload) => {
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceRoleAssignmentRemove: (payload) => {
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceChannelOverrideUpdate: (payload) => {
        options.onWorkspacePermissionsChanged?.(payload.guildId);
      },
      onWorkspaceIpBanSync: (payload) => {
        options.onWorkspaceModerationChanged?.(payload);
      },
      onPresenceSync: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        options.setOnlineMembers(payload.userIds);
      },
      onPresenceUpdate: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        options.setOnlineMembers((existing) =>
          applyPresenceUpdate(existing, payload),
        );
      },
    });

    onCleanup(() => gateway.close());
  });
}
