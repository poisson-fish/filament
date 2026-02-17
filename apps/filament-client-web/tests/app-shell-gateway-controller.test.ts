import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  channelFromResponse,
  friendRequestIdFromInput,
  messageIdFromInput,
  channelIdFromInput,
  guildIdFromInput,
  guildNameFromInput,
  messageContentFromInput,
  messageFromResponse,
  reactionEmojiFromInput,
  type WorkspaceRecord,
  type FriendRecord,
  type FriendRequestList,
} from "../src/domain/chat";
import { type VoiceParticipantPayload } from "../src/lib/gateway";
import {
  applyMessageDelete,
  applyMessageUpdate,
  applyChannelCreate,
  applyWorkspaceUpdate,
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
const VOICE_CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAZ");
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

  it("applies message update payload to known messages only", () => {
    const current = messageFixture({
      guildId: GUILD_ID,
      channelId: CHANNEL_ID,
      messageId: MESSAGE_ID,
      authorId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      content: "before",
    });
    expect(
      applyMessageUpdate([current], {
        guildId: GUILD_ID,
        channelId: CHANNEL_ID,
        messageId: MESSAGE_ID,
        updatedFields: {
          content: messageContentFromInput("after"),
          markdownTokens: [{ type: "text", text: "after" }],
        },
        updatedAtUnix: 2,
      }),
    ).toEqual([
      {
        ...current,
        content: "after",
        markdownTokens: [{ type: "text", text: "after" }],
      },
    ]);
    expect(
      applyMessageUpdate([current], {
        guildId: GUILD_ID,
        channelId: CHANNEL_ID,
        messageId: messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAF"),
        updatedFields: {
          content: messageContentFromInput("ignored"),
        },
        updatedAtUnix: 3,
      }),
    ).toEqual([current]);
  });

  it("applies message delete payload by removing the target message", () => {
    const first = messageFixture({
      guildId: GUILD_ID,
      channelId: CHANNEL_ID,
      messageId: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
      authorId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      content: "first",
    });
    const second = messageFixture({
      guildId: GUILD_ID,
      channelId: CHANNEL_ID,
      messageId: "01ARZ3NDEKTSV4RRFFQ69G5FAC",
      authorId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      content: "second",
    });
    expect(
      applyMessageDelete([first, second], {
        guildId: GUILD_ID,
        channelId: CHANNEL_ID,
        messageId: first.messageId,
        deletedAtUnix: 4,
      }),
    ).toEqual([second]);
  });

  it("applies channel create updates idempotently for a workspace", () => {
    const existingChannel = channelFromResponse({
      channel_id: CHANNEL_ID,
      name: "existing",
      kind: "text",
    });
    const created = channelFromResponse({
      channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FZZ",
      name: "bridge-call",
      kind: "voice",
    });

    const initial = [
      {
        guildId: GUILD_ID,
        guildName: guildNameFromInput("Ops"),
        visibility: "private" as const,
        channels: [existingChannel],
      },
    ];
    const once = applyChannelCreate(initial, { guildId: GUILD_ID, channel: created });
    const twice = applyChannelCreate(once, { guildId: GUILD_ID, channel: created });

    expect(once[0]?.channels.map((entry) => entry.channelId)).toEqual([
      CHANNEL_ID,
      created.channelId,
    ]);
    expect(twice).toEqual(once);
  });

  it("applies workspace updates to name and visibility in-place", () => {
    const initial: WorkspaceRecord[] = [
      {
        guildId: GUILD_ID,
        guildName: guildNameFromInput("Ops"),
        visibility: "private",
        channels: [],
      },
    ];

    expect(
      applyWorkspaceUpdate(initial, {
        guildId: GUILD_ID,
        updatedFields: {
          name: guildNameFromInput("Ops Prime"),
          visibility: "public",
        },
        updatedAtUnix: 1,
      }),
    ).toEqual([
      {
        guildId: GUILD_ID,
        guildName: "Ops Prime",
        visibility: "public",
        channels: [],
      },
    ]);
  });

  it("wires gateway events and closes subscriptions on channel access loss", async () => {
    const [session] = createSignal(SESSION);
    const [activeGuildId] = createSignal(GUILD_ID);
    const [activeChannelId] = createSignal(CHANNEL_ID);
    const [canAccessActiveChannel, setCanAccessActiveChannel] = createSignal(true);
    const [gatewayOnline, setGatewayOnline] = createSignal(false);
    const [onlineMembers, setOnlineMembers] = createSignal<string[]>([]);
    const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([
      {
        guildId: GUILD_ID,
        guildName: guildNameFromInput("Ops"),
        visibility: "private" as const,
        channels: [
          channelFromResponse({
            channel_id: CHANNEL_ID,
            name: "incident-room",
            kind: "text",
          }),
            channelFromResponse({
              channel_id: VOICE_CHANNEL_ID,
              name: "ops-voice",
              kind: "voice",
            }),
        ],
      },
    ]);
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
    const [resolvedUsernames, setResolvedUsernames] = createSignal<Record<string, string>>(
      {},
    );
    const [avatarVersionByUserId, setAvatarVersionByUserId] = createSignal<Record<string, number>>(
      {},
    );
    const [profileDraftUsername, setProfileDraftUsername] = createSignal("");
    const [profileDraftAbout, setProfileDraftAbout] = createSignal("");
    const [friends, setFriends] = createSignal<FriendRecord[]>([]);
    const [friendRequests, setFriendRequests] = createSignal<FriendRequestList>({
      incoming: [],
      outgoing: [],
    });
    const [voiceParticipantsByChannel, setVoiceParticipantsByChannel] = createSignal<
      Record<string, VoiceParticipantPayload[]>
    >({});

    const scrollMessageListToBottomMock = vi.fn();
    const closeGatewayMock = vi.fn();
    const setSubscribedChannelsMock = vi.fn();
    const onWorkspacePermissionsChanged = vi.fn();
    const onGatewayConnectionChange = vi.fn();
    let handlers: any = null;
    const connectGatewayMock = vi.fn((_token, _guildId, _channelId, nextHandlers) => {
      handlers = nextHandlers;
      return {
        setSubscribedChannels: setSubscribedChannelsMock,
        close: closeGatewayMock,
      };
    });

    createRoot(() =>
      createGatewayController(
        {
          session,
          activeGuildId,
          activeChannelId,
          workspaces,
          canAccessActiveChannel,
          setGatewayOnline,
          setOnlineMembers,
          setWorkspaces,
          setMessages,
          setReactionState,
          setResolvedUsernames,
          setAvatarVersionByUserId,
          setProfileDraftUsername,
          setProfileDraftAbout,
          setFriends,
          setFriendRequests,
          setVoiceParticipantsByChannel,
          isMessageListNearBottom: () => true,
          scrollMessageListToBottom: scrollMessageListToBottomMock,
          onGatewayConnectionChange,
          onWorkspacePermissionsChanged,
        },
        {
          connectGateway: connectGatewayMock,
        },
      ),
    );

    expect(connectGatewayMock).toHaveBeenCalledTimes(1);
    expect(setSubscribedChannelsMock).toHaveBeenCalledWith(GUILD_ID, [
      CHANNEL_ID,
      VOICE_CHANNEL_ID,
    ]);
    if (!handlers) {
      throw new Error("missing handlers");
    }

    handlers.onOpenStateChange(true);
    expect(gatewayOnline()).toBe(true);
    expect(onGatewayConnectionChange).toHaveBeenCalledWith(true);
    handlers.onReady({
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
    });

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

    handlers.onVoiceParticipantSync({
      guildId: GUILD_ID,
      channelId: CHANNEL_ID,
      participants: [
        {
          userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
          identity: "u.01ARZ3NDEKTSV4RRFFQ69G5FAB.session",
          joinedAtUnix: 1,
          updatedAtUnix: 1,
          isMuted: false,
          isDeafened: false,
          isSpeaking: false,
          isVideoEnabled: false,
          isScreenShareEnabled: false,
        },
      ],
      syncedAtUnix: 1,
    });
    handlers.onVoiceStreamPublish({
      guildId: GUILD_ID,
      channelId: CHANNEL_ID,
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      identity: "u.01ARZ3NDEKTSV4RRFFQ69G5FAB.session",
      stream: "camera",
      publishedAtUnix: 2,
    });
    handlers.onVoiceParticipantUpdate({
      guildId: GUILD_ID,
      channelId: CHANNEL_ID,
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      identity: "u.01ARZ3NDEKTSV4RRFFQ69G5FAB.session",
      updatedFields: { isSpeaking: true },
      updatedAtUnix: 3,
    });
    handlers.onVoiceStreamUnpublish({
      guildId: GUILD_ID,
      channelId: CHANNEL_ID,
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      identity: "u.01ARZ3NDEKTSV4RRFFQ69G5FAB.session",
      stream: "camera",
      unpublishedAtUnix: 4,
    });
    handlers.onVoiceParticipantLeave({
      guildId: GUILD_ID,
      channelId: CHANNEL_ID,
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      identity: "u.01ARZ3NDEKTSV4RRFFQ69G5FAB.session",
      leftAtUnix: 5,
    });
    expect(voiceParticipantsByChannel()[`${GUILD_ID}|${CHANNEL_ID}`]).toEqual([]);

    handlers.onVoiceParticipantSync({
      guildId: GUILD_ID,
      channelId: CHANNEL_ID,
      participants: [
        {
          userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
          identity: "u.01ARZ3NDEKTSV4RRFFQ69G5FAB.session.current",
          joinedAtUnix: 6,
          updatedAtUnix: 6,
          isMuted: false,
          isDeafened: false,
          isSpeaking: false,
          isVideoEnabled: false,
          isScreenShareEnabled: false,
        },
      ],
      syncedAtUnix: 6,
    });
    handlers.onVoiceParticipantLeave({
      guildId: GUILD_ID,
      channelId: CHANNEL_ID,
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      identity: "u.01ARZ3NDEKTSV4RRFFQ69G5FAB.session.stale",
      leftAtUnix: 7,
    });
    expect(voiceParticipantsByChannel()[`${GUILD_ID}|${CHANNEL_ID}`]).toEqual([]);

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

    handlers.onMessageUpdate({
      guildId: GUILD_ID,
      channelId: CHANNEL_ID,
      messageId: messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAC"),
      updatedFields: {
        content: "incoming edited",
        markdownTokens: [{ type: "text", text: "incoming edited" }],
      },
      updatedAtUnix: 2,
    });
    expect(messages().map((entry) => entry.content)).toEqual([
      "existing",
      "incoming edited",
    ]);

    handlers.onMessageDelete({
      guildId: GUILD_ID,
      channelId: CHANNEL_ID,
      messageId: messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAC"),
      deletedAtUnix: 3,
    });
    expect(messages().map((entry) => entry.content)).toEqual(["existing"]);

    handlers.onChannelCreate({
      guildId: GUILD_ID,
      channel: channelFromResponse({
        channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FZY",
        name: "voice-bridge",
        kind: "voice",
      }),
    });
    handlers.onWorkspaceUpdate({
      guildId: GUILD_ID,
      updatedFields: {
        name: guildNameFromInput("Ops Oncall"),
      },
      updatedAtUnix: 4,
    });
    handlers.onWorkspaceRoleUpdate({
      guildId: GUILD_ID,
      roleId: "01ARZ3NDEKTSV4RRFFQ69G5FAZ",
      updatedFields: {
        name: "ops_admin",
      },
      updatedAtUnix: 5,
    });
    expect(workspaces()[0]?.channels.map((entry) => entry.name)).toEqual([
      "incident-room",
      "ops-voice",
      "voice-bridge",
    ]);
    expect(workspaces()[0]?.guildName).toBe("Ops Oncall");
    expect(onWorkspacePermissionsChanged).toHaveBeenCalledWith(GUILD_ID);

    handlers.onWorkspaceMemberRemove({
      guildId: GUILD_ID,
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      reason: "kick",
      removedAtUnix: 5,
    });
    expect(workspaces()).toEqual([]);

    handlers.onProfileUpdate({
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      updatedFields: {
        username: "alice-updated",
        aboutMarkdown: "updated about",
        aboutMarkdownTokens: [{ type: "text", text: "updated about" }],
      },
      updatedAtUnix: 6,
    });
    handlers.onProfileAvatarUpdate({
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      avatarVersion: 8,
      updatedAtUnix: 7,
    });
    handlers.onFriendRequestCreate({
      requestId: friendRequestIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FC0"),
      senderUserId: "01ARZ3NDEKTSV4RRFFQ69G5FC1",
      senderUsername: "bob",
      recipientUserId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      recipientUsername: "alice-updated",
      createdAtUnix: 8,
    });
    handlers.onFriendRequestUpdate({
      requestId: friendRequestIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FC0"),
      state: "accepted",
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      friendUserId: "01ARZ3NDEKTSV4RRFFQ69G5FC1",
      friendUsername: "bob",
      friendshipCreatedAtUnix: 9,
      updatedAtUnix: 9,
    });
    handlers.onFriendRemove({
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
      friendUserId: "01ARZ3NDEKTSV4RRFFQ69G5FC1",
      removedAtUnix: 10,
    });

    expect(resolvedUsernames()["01ARZ3NDEKTSV4RRFFQ69G5FAB"]).toBe("alice-updated");
    expect(resolvedUsernames()["01ARZ3NDEKTSV4RRFFQ69G5FC1"]).toBe("bob");
    expect(avatarVersionByUserId()["01ARZ3NDEKTSV4RRFFQ69G5FAB"]).toBe(8);
    expect(profileDraftUsername()).toBe("alice-updated");
    expect(profileDraftAbout()).toBe("updated about");
    expect(friendRequests().incoming).toEqual([]);
    expect(friends()).toEqual([]);

    setCanAccessActiveChannel(false);
    await flush();
    expect(closeGatewayMock).toHaveBeenCalledTimes(1);
    expect(gatewayOnline()).toBe(false);
    expect(onlineMembers()).toEqual([]);

    handlers.onOpenStateChange(false);
    expect(onGatewayConnectionChange).toHaveBeenCalledWith(false);
  });
});
