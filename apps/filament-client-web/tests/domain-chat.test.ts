import {
  attachmentFilenameFromInput,
  attachmentFromResponse,
  channelPermissionSnapshotFromResponse,
  channelIdFromInput,
  friendListFromResponse,
  friendRequestCreateFromResponse,
  friendRequestListFromResponse,
  guildVisibilityFromInput,
  guildIdFromInput,
  markdownTokensFromResponse,
  messageContentFromInput,
  messageFromResponse,
  permissionFromInput,
  publicGuildDirectoryFromResponse,
  reactionEmojiFromInput,
  reactionFromResponse,
  roleFromInput,
  searchQueryFromInput,
  userLookupListFromResponse,
  voiceTokenFromResponse,
  workspaceFromStorage,
} from "../src/domain/chat";

describe("chat domain invariants", () => {
  it("accepts ULID ids", () => {
    const ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    expect(guildIdFromInput(ulid)).toBe(ulid);
    expect(channelIdFromInput(ulid)).toBe(ulid);
  });

  it("rejects invalid ids", () => {
    expect(() => guildIdFromInput("not-ulid")).toThrow();
  });

  it("rejects oversized message content", () => {
    expect(() => messageContentFromInput("A".repeat(2001))).toThrow();
  });

  it("maps message payloads into validated records", () => {
    const message = messageFromResponse({
      message_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      author_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      content: "hello",
      markdown_tokens: [
        { type: "paragraph_start" },
        { type: "text", text: "hello" },
        { type: "paragraph_end" },
      ],
      created_at_unix: 1,
    });

    expect(message.content).toBe("hello");
  });

  it("enforces search query policy", () => {
    expect(searchQueryFromInput("needle")).toBe("needle");
    expect(() => searchQueryFromInput("content:hello")).toThrow();
  });

  it("validates workspace cache payloads", () => {
    const workspace = workspaceFromStorage({
      guildId: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      guildName: "Security",
      visibility: "public",
      channels: [{ channelId: "01ARZ3NDEKTSV4RRFFQ69G5FAV", name: "incident-room" }],
    });

    expect(workspace.channels[0]?.name).toBe("incident-room");
    expect(workspace.visibility).toBe("public");
  });

  it("defaults cached workspace visibility to private when omitted", () => {
    const workspace = workspaceFromStorage({
      guildId: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      guildName: "Security",
      channels: [{ channelId: "01ARZ3NDEKTSV4RRFFQ69G5FAV", name: "incident-room" }],
    });
    expect(workspace.visibility).toBe("private");
  });

  it("validates reactions", () => {
    const reaction = reactionFromResponse({ emoji: "👍", count: 2 });
    expect(reaction.count).toBe(2);
    expect(reactionEmojiFromInput("thumbs_up")).toBe("thumbs_up");
    expect(() => reactionEmojiFromInput("bad emoji")).toThrow();
  });

  it("validates attachment filenames and payloads", () => {
    expect(attachmentFilenameFromInput("incident.log")).toBe("incident.log");
    expect(() => attachmentFilenameFromInput("../incident.log")).toThrow();

    const attachment = attachmentFromResponse({
      attachment_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      owner_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      filename: "capture.png",
      mime_type: "image/png",
      size_bytes: 1024,
      sha256_hex: "a".repeat(64),
    });

    expect(attachment.filename).toBe("capture.png");
  });

  it("validates markdown token stream", () => {
    const tokens = markdownTokensFromResponse([
      { type: "paragraph_start" },
      { type: "text", text: "safe" },
      { type: "link_start", href: "https://example.com" },
      { type: "text", text: "link" },
      { type: "link_end" },
      { type: "paragraph_end" },
    ]);
    expect(tokens.length).toBe(6);
    expect(() => markdownTokensFromResponse([{ type: "unknown" }])).toThrow();
  });

  it("validates voice token and role/permission enums", () => {
    const voice = voiceTokenFromResponse({
      token: "T".repeat(96),
      livekit_url: "wss://livekit.example.com",
      room: "filament.voice.abc.def",
      identity: "u.abc.123",
      can_publish: true,
      can_subscribe: true,
      publish_sources: ["microphone", "screen_share"],
      expires_in_secs: 300,
    });

    expect(voice.publishSources).toEqual(["microphone", "screen_share"]);
    expect(roleFromInput("member")).toBe("member");
    expect(permissionFromInput("create_message")).toBe("create_message");
    expect(() => permissionFromInput("bad_perm")).toThrow();
  });

  it("validates channel permission snapshot payloads", () => {
    const snapshot = channelPermissionSnapshotFromResponse({
      role: "moderator",
      permissions: ["delete_message", "create_message", "subscribe_streams"],
    });
    expect(snapshot.role).toBe("moderator");
    expect(snapshot.permissions).toContain("create_message");
    expect(() =>
      channelPermissionSnapshotFromResponse({
        role: "owner",
        permissions: ["unknown"],
      }),
    ).toThrow();
  });

  it("validates guild visibility and public directory payloads", () => {
    expect(guildVisibilityFromInput("private")).toBe("private");
    expect(guildVisibilityFromInput("public")).toBe("public");
    expect(() => guildVisibilityFromInput("internal")).toThrow();

    const directory = publicGuildDirectoryFromResponse({
      guilds: [
        {
          guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          name: "Lobby",
          visibility: "public",
        },
      ],
    });
    expect(directory.guilds[0]?.name).toBe("Lobby");
  });

  it("validates friendship payloads", () => {
    const friends = friendListFromResponse({
      friends: [
        {
          user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          username: "alice",
          created_at_unix: 1,
        },
      ],
    });
    expect(friends[0]?.username).toBe("alice");

    const requests = friendRequestListFromResponse({
      incoming: [
        {
          request_id: "01ARZ3NDEKTSV4RRFFQ69G5FAW",
          sender_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          sender_username: "alice",
          recipient_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAX",
          recipient_username: "bob",
          created_at_unix: 2,
        },
      ],
      outgoing: [],
    });
    expect(requests.incoming[0]?.senderUsername).toBe("alice");

    const create = friendRequestCreateFromResponse({
      request_id: "01ARZ3NDEKTSV4RRFFQ69G5FAY",
      sender_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      recipient_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAX",
      created_at_unix: 3,
    });
    expect(create.createdAtUnix).toBe(3);
  });

  it("validates user lookup payloads", () => {
    const users = userLookupListFromResponse({
      users: [
        {
          user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          username: "alice",
        },
      ],
    });
    expect(users[0]?.username).toBe("alice");
  });
});
