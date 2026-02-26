import {
  attachmentFilenameFromInput,
  attachmentFromResponse,
  auditCursorFromInput,
  channelFromResponse,
  channelKindFromInput,
  channelPermissionSnapshotFromResponse,
  directoryJoinErrorCodeFromInput,
  directoryJoinResultFromResponse,
  channelIdFromInput,
  friendListFromResponse,
  friendRequestCreateFromResponse,
  friendRequestListFromResponse,
  guildAuditPageFromResponse,
  guildIpBanApplyResultFromResponse,
  guildIpBanPageFromResponse,
  guildIpBanIdFromInput,
  guildRoleListFromResponse,
  guildVisibilityFromInput,
  guildIdFromInput,
  ipNetworkFromInput,
  markdownTokensFromResponse,
  messageContentFromInput,
  messageFromResponse,
  permissionFromInput,
  profileFromResponse,
  profileAboutFromInput,
  PROFILE_ABOUT_MAX_CHARS,
  publicGuildDirectoryFromResponse,
  reactionEmojiFromInput,
  reactionFromResponse,
  roleColorHexFromInput,
  roleFromInput,
  searchQueryFromInput,
  userLookupListFromResponse,
  voiceTokenFromResponse,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
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
    expect(messageContentFromInput("")).toBe("");
    expect(() => messageContentFromInput("A".repeat(2001))).toThrow();
  });

  it("enforces profile about length cap at 2048 characters", () => {
    expect(profileAboutFromInput("A".repeat(PROFILE_ABOUT_MAX_CHARS))).toHaveLength(
      PROFILE_ABOUT_MAX_CHARS,
    );
    expect(() => profileAboutFromInput("A".repeat(PROFILE_ABOUT_MAX_CHARS + 1))).toThrow(
      `About must be 0-${PROFILE_ABOUT_MAX_CHARS} characters.`,
    );
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
    expect(message.attachments).toEqual([]);
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
      channels: [{ channelId: "01ARZ3NDEKTSV4RRFFQ69G5FAV", name: "incident-room", kind: "voice" }],
    });

    expect(workspace.channels[0]?.name).toBe("incident-room");
    expect(workspace.channels[0]?.kind).toBe("voice");
    expect(workspace.visibility).toBe("public");
  });

  it("defaults cached workspace visibility to private when omitted", () => {
    const workspace = workspaceFromStorage({
      guildId: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      guildName: "Security",
      channels: [{ channelId: "01ARZ3NDEKTSV4RRFFQ69G5FAV", name: "incident-room" }],
    });
    expect(workspace.visibility).toBe("private");
    expect(workspace.channels[0]?.kind).toBe("text");
  });

  it("validates channel kind parsing", () => {
    expect(channelKindFromInput("text")).toBe("text");
    expect(channelKindFromInput("voice")).toBe("voice");
    expect(() => channelKindFromInput("video")).toThrow();

    const channel = channelFromResponse({
      channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      name: "incident-room",
      kind: "voice",
    });
    expect(channel.kind).toBe("voice");
    expect(() =>
      channelFromResponse({
        channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        name: "incident-room",
      }),
    ).toThrow();
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
      { type: "fenced_code", language: "RuSt", code: "fn main() {}\n" },
      { type: "paragraph_end" },
    ]);
    expect(tokens.length).toBe(7);
    expect(tokens[5]).toEqual({
      type: "fenced_code",
      language: "rust",
      code: "fn main() {}\n",
    });
    expect(() => markdownTokensFromResponse([{ type: "unknown" }])).toThrow();
    expect(() =>
      markdownTokensFromResponse([
        { type: "fenced_code", language: "rust<script>", code: "fn" },
      ]),
    ).toThrow();
    expect(() =>
      markdownTokensFromResponse([
        { type: "fenced_code", language: null, code: "plain text" },
      ]),
    ).not.toThrow();
  });

  it("rejects markdown token streams that exceed fenced code block limits", () => {
    const tokens = Array.from({ length: 65 }, () => ({
      type: "fenced_code",
      language: "txt",
      code: "x",
    }));
    expect(() => markdownTokensFromResponse(tokens)).toThrow(
      "markdown_tokens exceeded fenced code block limit.",
    );
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
    expect(permissionFromInput("manage_member_roles")).toBe("manage_member_roles");
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
          avatar_version: 4,
        },
      ],
    });
    expect(users[0]?.username).toBe("alice");
    expect(users[0]?.avatarVersion).toBe(4);
  });

  it("maps profile payloads into validated records", () => {
    const profile = profileFromResponse({
      user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      username: "alice",
      about_markdown: "hello **world**",
      about_markdown_tokens: [
        { type: "paragraph_start" },
        { type: "text", text: "hello " },
        { type: "strong_start" },
        { type: "text", text: "world" },
        { type: "strong_end" },
        { type: "paragraph_end" },
      ],
      avatar_version: 7,
    });
    expect(profile.username).toBe("alice");
    expect(profile.avatarVersion).toBe(7);
  });

  it("validates directory phase newtypes", () => {
    expect(guildIpBanIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV")).toBe(
      "01ARZ3NDEKTSV4RRFFQ69G5FAV",
    );
    expect(() => guildIpBanIdFromInput("not-ulid")).toThrow();

    expect(ipNetworkFromInput("203.0.113.19")).toBe("203.0.113.19/32");
    expect(ipNetworkFromInput("203.0.113.200/24")).toBe("203.0.113.0/24");
    expect(ipNetworkFromInput("2001:DB8::F00D/64")).toBe("2001:db8::/64");
    expect(() => ipNetworkFromInput("203.0.113.9/33")).toThrow();

    expect(auditCursorFromInput("abcDEF_123-xyz")).toBe("abcDEF_123-xyz");
    expect(() => auditCursorFromInput("bad cursor")).toThrow();

    expect(workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV")).toBe(
      "01ARZ3NDEKTSV4RRFFQ69G5FAV",
    );
    expect(() => workspaceRoleIdFromInput("bad-role-id")).toThrow();
    expect(workspaceRoleNameFromInput(" Incident Lead ")).toBe("Incident Lead");
    expect(() => workspaceRoleNameFromInput("@everyone")).toThrow();
    expect(() => workspaceRoleNameFromInput("workspace_owner")).toThrow();
    expect(roleColorHexFromInput(" #a1b2c3 ")).toBe("#A1B2C3");
    expect(() => roleColorHexFromInput("#12345")).toThrow();
    expect(() => roleColorHexFromInput("#12G45Z")).toThrow();
  });

  it("validates directory join DTO and error-code mapping", () => {
    const accepted = directoryJoinResultFromResponse({
      guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      outcome: "accepted",
    });
    expect(accepted.outcome).toBe("accepted");
    expect(accepted.joined).toBe(true);

    const alreadyMember = directoryJoinResultFromResponse({
      guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      outcome: "already_member",
    });
    expect(alreadyMember.joined).toBe(true);

    const rejected = directoryJoinResultFromResponse({
      guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      outcome: "rejected_ip_ban",
    });
    expect(rejected.joined).toBe(false);
    expect(
      directoryJoinResultFromResponse({
        guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        outcome: "rejected_user_ban",
      }).joined,
    ).toBe(false);
    expect(
      directoryJoinResultFromResponse({
        guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        outcome: "rejected_visibility",
      }).joined,
    ).toBe(false);
    expect(directoryJoinErrorCodeFromInput("directory_join_ip_banned")).toBe(
      "directory_join_ip_banned",
    );
    expect(directoryJoinErrorCodeFromInput("something_else")).toBe("unexpected_error");
    expect(() =>
      directoryJoinResultFromResponse({
        guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
        outcome: "invalid_outcome",
      }),
    ).toThrow();
  });

  it("validates redacted guild audit and ip-ban DTO payloads", () => {
    const auditPage = guildAuditPageFromResponse({
      events: [
        {
          audit_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
          actor_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAW",
          target_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAX",
          action: "directory.join.rejected.ip_ban",
          created_at_unix: 123,
          ip_ban_match: true,
          details: { source: "directory_join" },
        },
      ],
      next_cursor: "audit_cursor_1",
    });
    expect(auditPage.events[0]?.ipBanMatch).toBe(true);
    expect(auditPage.nextCursor).toBe("audit_cursor_1");
    expect(() =>
      guildAuditPageFromResponse({
        events: [
          {
            audit_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            actor_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAW",
            target_user_id: null,
            action: "directory.join.accepted",
            created_at_unix: 123,
            details: { ip_cidr: "203.0.113.0/24" },
          },
        ],
      }),
    ).toThrow();

    const ipBanPage = guildIpBanPageFromResponse({
      bans: [
        {
          ban_id: "01ARZ3NDEKTSV4RRFFQ69G5FAY",
          source_user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAW",
          reason: "raid traffic",
          created_at_unix: 100,
          expires_at_unix: null,
        },
      ],
      next_cursor: null,
    });
    expect(ipBanPage.bans[0]?.reason).toBe("raid traffic");
    expect(() =>
      guildIpBanPageFromResponse({
        bans: [
          {
            ban_id: "01ARZ3NDEKTSV4RRFFQ69G5FAY",
            ip_cidr: "203.0.113.0/24",
            created_at_unix: 100,
          },
        ],
      }),
    ).toThrow();

    const applyResult = guildIpBanApplyResultFromResponse({
      created_count: 2,
      ban_ids: ["01ARZ3NDEKTSV4RRFFQ69G5FAY", "01ARZ3NDEKTSV4RRFFQ69G5FAZ"],
    });
    expect(applyResult.createdCount).toBe(2);
    expect(applyResult.banIds).toHaveLength(2);

    const roleList = guildRoleListFromResponse({
      roles: [
        {
          role_id: "01ARZ3NDEKTSV4RRFFQ69G5FAY",
          name: "Responder",
          position: 3,
          is_system: false,
          permissions: ["create_message", "subscribe_streams"],
          color_hex: "#3366CC",
        },
      ],
    });
    expect(roleList.roles[0]?.name).toBe("Responder");
    expect(roleList.roles[0]?.permissions).toEqual([
      "create_message",
      "subscribe_streams",
    ]);
    expect(roleList.roles[0]?.colorHex).toBe("#3366CC");
    expect(() =>
      guildRoleListFromResponse({
        roles: [
          {
            role_id: "01ARZ3NDEKTSV4RRFFQ69G5FAY",
            name: "Responder",
            position: 3,
            is_system: false,
            permissions: ["create_message"],
            color_hex: "#12345",
          },
        ],
      }),
    ).toThrow();
  });
});
