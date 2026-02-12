import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  messageIdFromInput,
  channelIdFromInput,
  guildIdFromInput,
  messageFromResponse,
  reactionEmojiFromInput,
} from "../src/domain/chat";
import {
  applyMessageReactionUpdate,
  applyPresenceUpdate,
  createGatewayController,
} from "../src/features/app-shell/controllers/gateway-controller";
import type { ReactionView } from "../src/features/app-shell/helpers";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const MESSAGE_ID = messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAA");
const THUMBS_UP = reactionEmojiFromInput("👍");

function messageFixture(input: {
  guildId: string;
  channelId: string;
  messageId: string;
  authorId: string;
  content: string;
}): ReturnType<typeof messageFromResponse> {
  return messageFromResponse({
    message_id: input.messageId,
    guild_id: input.guildId,
    channel_id: input.channelId,
    author_id: input.authorId,
    content: input.content,
    markdown_tokens: [{ type: "text", text: input.content }],
    attachments: [],
    created_at_unix: 1,
  });
}

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe("app shell gateway controller", () => {
  it("applies presence update transitions deterministically", () => {
    expect(
      applyPresenceUpdate(["alpha"], {
        userId: "beta",
        status: "online",
      }),
    ).toEqual(["alpha", "beta"]);
    expect(
      applyPresenceUpdate(["alpha", "beta"], {
        userId: "beta",
        status: "online",
      }),
    ).toEqual(["alpha", "beta"]);
    expect(
      applyPresenceUpdate(["alpha", "beta"], {
        userId: "beta",
        status: "offline",
      }),
    ).toEqual(["alpha"]);
  });

  it("applies message reaction updates without preserving impossible reacted state", () => {
    expect(
      applyMessageReactionUpdate(
        {
          [`${MESSAGE_ID}|${THUMBS_UP}`]: { count: 2, reacted: true },
        },
        {
          messageId: MESSAGE_ID,
          emoji: THUMBS_UP,
          count: 0,
        },
      ),
    ).toEqual({});
  });

  it("wires gateway events and closes subscriptions on channel access loss", async () => {
    const [session] = createSignal(SESSION);
    const [activeGuildId] = createSignal(GUILD_ID);
    const [activeChannelId] = createSignal(CHANNEL_ID);
    const [canAccessActiveChannel, setCanAccessActiveChannel] = createSignal(true);
    const [gatewayOnline, setGatewayOnline] = createSignal(false);
    const [onlineMembers, setOnlineMembers] = createSignal<string[]>([]);
    const [messages, setMessages] = createSignal([
      messageFixture({
        guildId: GUILD_ID,
        channelId: CHANNEL_ID,
        messageId: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
        authorId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
        content: "existing",
      }),
    ]);
    const [reactionState, setReactionState] = createSignal<Record<string, ReactionView>>(
      {},
    );

    const scrollMessageListToBottomMock = vi.fn();
    const closeGatewayMock = vi.fn();
    let handlers: any = null;
    const connectGatewayMock = vi.fn((_token, _guildId, _channelId, nextHandlers) => {
      handlers = nextHandlers;
      return {
        close: closeGatewayMock,
      };
    });

    createRoot(() =>
      createGatewayController(
        {
          session,
          activeGuildId,
          activeChannelId,
          canAccessActiveChannel,
          setGatewayOnline,
          setOnlineMembers,
          setMessages,
          setReactionState,
          isMessageListNearBottom: () => true,
          scrollMessageListToBottom: scrollMessageListToBottomMock,
        },
        {
          connectGateway: connectGatewayMock,
        },
      ),
    );

    expect(connectGatewayMock).toHaveBeenCalledTimes(1);
    if (!handlers) {
      throw new Error("missing handlers");
    }

    handlers.onOpenStateChange(true);
    expect(gatewayOnline()).toBe(true);

    handlers.onMessageCreate(
      messageFixture({
        guildId: GUILD_ID,
        channelId: CHANNEL_ID,
        messageId: "01ARZ3NDEKTSV4RRFFQ69G5FAC",
        authorId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
        content: "incoming",
      }),
    );
    handlers.onMessageCreate(
      messageFixture({
        guildId: "01ARZ3NDEKTSV4RRFFQ69G5FAD",
        channelId: CHANNEL_ID,
        messageId: "01ARZ3NDEKTSV4RRFFQ69G5FAE",
        authorId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
        content: "ignored",
      }),
    );
    expect(messages().map((entry) => entry.content)).toEqual(["existing", "incoming"]);
    expect(scrollMessageListToBottomMock).toHaveBeenCalledTimes(1);

    handlers.onPresenceSync({
      guildId: GUILD_ID,
      userIds: ["alpha"],
    });
    handlers.onPresenceUpdate({
      guildId: GUILD_ID,
      userId: "beta",
      status: "online",
    });
    handlers.onPresenceUpdate({
      guildId: GUILD_ID,
      userId: "alpha",
      status: "offline",
    });
    expect(onlineMembers()).toEqual(["beta"]);

    handlers.onMessageReaction({
      guildId: GUILD_ID,
      channelId: CHANNEL_ID,
      messageId: MESSAGE_ID,
      emoji: THUMBS_UP,
      count: 1,
    });
    expect(reactionState()[`${MESSAGE_ID}|${THUMBS_UP}`]).toEqual({
      count: 1,
      reacted: false,
    });

    setCanAccessActiveChannel(false);
    await flush();
    expect(closeGatewayMock).toHaveBeenCalledTimes(1);
    expect(gatewayOnline()).toBe(false);
    expect(onlineMembers()).toEqual([]);
  });
});
