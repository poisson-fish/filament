import { createEffect, onCleanup, type Accessor, type Setter } from "solid-js";
import type { AccessToken, AuthSession } from "../../../domain/auth";
import type { ChannelId, GuildId, MessageRecord } from "../../../domain/chat";
import { connectGateway } from "../../../lib/gateway";
import { mergeMessage } from "../helpers";

export interface GatewayControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  canAccessActiveChannel: Accessor<boolean>;
  setGatewayOnline: Setter<boolean>;
  setOnlineMembers: Setter<string[]>;
  setMessages: Setter<MessageRecord[]>;
  isMessageListNearBottom: () => boolean;
  scrollMessageListToBottom: () => void;
}

interface GatewayClient {
  close: () => void;
}

interface GatewayHandlers {
  onOpenStateChange: (isOpen: boolean) => void;
  onMessageCreate: (message: MessageRecord) => void;
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

export function createGatewayController(
  options: GatewayControllerOptions,
  dependencies: Partial<GatewayControllerDependencies> = {},
): void {
  const deps = {
    ...DEFAULT_GATEWAY_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };

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
