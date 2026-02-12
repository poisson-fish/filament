import { accessTokenFromInput } from "../src/domain/auth";
import { channelIdFromInput, guildIdFromInput } from "../src/domain/chat";
import { connectGateway, resolveGatewayUrl } from "../src/lib/gateway";

describe("gateway URL resolution", () => {
  const token = accessTokenFromInput("A".repeat(64));

  afterEach(() => {
    vi.unstubAllEnvs();
  });

  it("uses explicit gateway env URL", () => {
    vi.stubEnv("VITE_FILAMENT_GATEWAY_WS_URL", "wss://chat.example.com");
    vi.stubEnv("VITE_FILAMENT_API_BASE_URL", "https://api.example.com");
    expect(resolveGatewayUrl(token)).toBe(`wss://chat.example.com/gateway/ws?access_token=${encodeURIComponent(token)}`);
  });

  it("derives ws URL from API base URL", () => {
    vi.stubEnv("VITE_FILAMENT_API_BASE_URL", "https://api.filament.example/api");
    expect(resolveGatewayUrl(token)).toBe(`wss://api.filament.example/gateway/ws?access_token=${encodeURIComponent(token)}`);
  });

  it("falls back to relative gateway path", () => {
    vi.stubEnv("VITE_FILAMENT_API_BASE_URL", "/api");
    expect(resolveGatewayUrl(token)).toBe(`/gateway/ws?access_token=${encodeURIComponent(token)}`);
  });
});

class MockWebSocket {
  static readonly CONNECTING = 0;
  static readonly OPEN = 1;
  static readonly CLOSING = 2;
  static readonly CLOSED = 3;
  static instances: MockWebSocket[] = [];

  readonly url: string;
  readyState = MockWebSocket.CONNECTING;
  sent: string[] = [];
  onopen: ((event: Event) => void) | null = null;
  onmessage: ((event: MessageEvent) => void) | null = null;
  onclose: ((event: CloseEvent) => void) | null = null;

  constructor(url: string) {
    this.url = url;
    MockWebSocket.instances.push(this);
  }

  send(data: string): void {
    this.sent.push(data);
  }

  close(): void {
    this.readyState = MockWebSocket.CLOSED;
    this.onclose?.({} as CloseEvent);
  }

  emitOpen(): void {
    this.readyState = MockWebSocket.OPEN;
    this.onopen?.(new Event("open"));
  }

  emitMessage(data: unknown): void {
    this.onmessage?.({ data } as MessageEvent);
  }

  static reset(): void {
    MockWebSocket.instances = [];
  }
}

const DEFAULT_TOKEN = accessTokenFromInput("A".repeat(64));
const DEFAULT_GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const DEFAULT_CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const ULID_ALPHABET = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

function ulidFromIndex(index: number): string {
  let value = index;
  let suffix = "";
  for (let i = 0; i < 4; i += 1) {
    suffix = ULID_ALPHABET[value % ULID_ALPHABET.length] + suffix;
    value = Math.floor(value / ULID_ALPHABET.length);
  }
  return `01ARZ3NDEKTSV4RRFFQ69G${suffix}`;
}

function createOpenGateway() {
  const onReady = vi.fn();
  const onMessageCreate = vi.fn();
  const onMessageUpdate = vi.fn();
  const onMessageDelete = vi.fn();
  const onMessageReaction = vi.fn();
  const onChannelCreate = vi.fn();
  const onWorkspaceUpdate = vi.fn();
  const onWorkspaceMemberAdd = vi.fn();
  const onWorkspaceMemberUpdate = vi.fn();
  const onWorkspaceMemberRemove = vi.fn();
  const onWorkspaceMemberBan = vi.fn();
  const onWorkspaceRoleCreate = vi.fn();
  const onWorkspaceRoleUpdate = vi.fn();
  const onWorkspaceRoleDelete = vi.fn();
  const onWorkspaceRoleReorder = vi.fn();
  const onWorkspaceRoleAssignmentAdd = vi.fn();
  const onWorkspaceRoleAssignmentRemove = vi.fn();
  const onWorkspaceChannelOverrideUpdate = vi.fn();
  const onWorkspaceIpBanSync = vi.fn();
  const onProfileUpdate = vi.fn();
  const onProfileAvatarUpdate = vi.fn();
  const onFriendRequestCreate = vi.fn();
  const onFriendRequestUpdate = vi.fn();
  const onFriendRequestDelete = vi.fn();
  const onFriendRemove = vi.fn();
  const onPresenceSync = vi.fn();
  const onPresenceUpdate = vi.fn();
  const onOpenStateChange = vi.fn();

  const client = connectGateway(
    DEFAULT_TOKEN,
    DEFAULT_GUILD_ID,
    DEFAULT_CHANNEL_ID,
    {
      onReady,
      onMessageCreate,
      onMessageUpdate,
      onMessageDelete,
      onMessageReaction,
      onChannelCreate,
      onWorkspaceUpdate,
      onWorkspaceMemberAdd,
      onWorkspaceMemberUpdate,
      onWorkspaceMemberRemove,
      onWorkspaceMemberBan,
      onWorkspaceRoleCreate,
      onWorkspaceRoleUpdate,
      onWorkspaceRoleDelete,
      onWorkspaceRoleReorder,
      onWorkspaceRoleAssignmentAdd,
      onWorkspaceRoleAssignmentRemove,
      onWorkspaceChannelOverrideUpdate,
      onWorkspaceIpBanSync,
      onProfileUpdate,
      onProfileAvatarUpdate,
      onFriendRequestCreate,
      onFriendRequestUpdate,
      onFriendRequestDelete,
      onFriendRemove,
      onPresenceSync,
      onPresenceUpdate,
      onOpenStateChange,
    },
  );

  const socket = MockWebSocket.instances.at(-1);
  if (!socket) {
    throw new Error("expected websocket instance");
  }
  socket.emitOpen();

  return {
    client,
    socket,
    onReady,
    onMessageCreate,
    onMessageUpdate,
    onMessageDelete,
    onMessageReaction,
    onChannelCreate,
    onWorkspaceUpdate,
    onWorkspaceMemberAdd,
    onWorkspaceMemberUpdate,
    onWorkspaceMemberRemove,
    onWorkspaceMemberBan,
    onWorkspaceRoleCreate,
    onWorkspaceRoleUpdate,
    onWorkspaceRoleDelete,
    onWorkspaceRoleReorder,
    onWorkspaceRoleAssignmentAdd,
    onWorkspaceRoleAssignmentRemove,
    onWorkspaceChannelOverrideUpdate,
    onWorkspaceIpBanSync,
    onProfileUpdate,
    onProfileAvatarUpdate,
    onFriendRequestCreate,
    onFriendRequestUpdate,
    onFriendRequestDelete,
    onFriendRemove,
    onPresenceSync,
    onPresenceUpdate,
    onOpenStateChange,
  };
}

describe("gateway payload parsing", () => {
  beforeEach(() => {
    MockWebSocket.reset();
    vi.stubGlobal("WebSocket", MockWebSocket as unknown as typeof WebSocket);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("rejects invalid ULID values in presence payloads", () => {
    const { socket, onPresenceSync, onPresenceUpdate } = createOpenGateway();

    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "presence_sync",
        d: {
          guild_id: "not-a-ulid",
          user_ids: [ulidFromIndex(1)],
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "presence_update",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          user_id: "bad-user-id",
          status: "online",
        },
      }),
    );

    expect(onPresenceSync).not.toHaveBeenCalled();
    expect(onPresenceUpdate).not.toHaveBeenCalled();
  });

  it("rejects invalid envelope versions and event types fail-closed", () => {
    const {
      socket,
      onReady,
      onMessageCreate,
      onMessageUpdate,
      onMessageDelete,
      onMessageReaction,
      onChannelCreate,
      onWorkspaceUpdate,
      onWorkspaceMemberAdd,
      onWorkspaceMemberUpdate,
      onWorkspaceMemberRemove,
      onWorkspaceMemberBan,
      onWorkspaceRoleCreate,
      onWorkspaceRoleUpdate,
      onWorkspaceRoleDelete,
      onWorkspaceRoleReorder,
      onWorkspaceRoleAssignmentAdd,
      onWorkspaceRoleAssignmentRemove,
      onWorkspaceChannelOverrideUpdate,
      onWorkspaceIpBanSync,
      onPresenceSync,
      onPresenceUpdate,
    } = createOpenGateway();
    const invalidEnvelopes = [
      { v: 2, t: "ready", d: {} },
      { v: 1, t: "INVALID_TYPE", d: {} },
      { v: 1, t: "presence-sync", d: {} },
      { v: 1, t: "x".repeat(65), d: {} },
      { v: 1, t: 42, d: {} },
      { v: 1, d: {} },
    ];

    for (const envelope of invalidEnvelopes) {
      socket.emitMessage(JSON.stringify(envelope));
    }

    expect(onReady).not.toHaveBeenCalled();
    expect(onMessageCreate).not.toHaveBeenCalled();
    expect(onMessageUpdate).not.toHaveBeenCalled();
    expect(onMessageDelete).not.toHaveBeenCalled();
    expect(onMessageReaction).not.toHaveBeenCalled();
    expect(onChannelCreate).not.toHaveBeenCalled();
    expect(onWorkspaceUpdate).not.toHaveBeenCalled();
    expect(onWorkspaceMemberAdd).not.toHaveBeenCalled();
    expect(onWorkspaceMemberUpdate).not.toHaveBeenCalled();
    expect(onWorkspaceMemberRemove).not.toHaveBeenCalled();
    expect(onWorkspaceMemberBan).not.toHaveBeenCalled();
    expect(onWorkspaceRoleCreate).not.toHaveBeenCalled();
    expect(onWorkspaceRoleUpdate).not.toHaveBeenCalled();
    expect(onWorkspaceRoleDelete).not.toHaveBeenCalled();
    expect(onWorkspaceRoleReorder).not.toHaveBeenCalled();
    expect(onWorkspaceRoleAssignmentAdd).not.toHaveBeenCalled();
    expect(onWorkspaceRoleAssignmentRemove).not.toHaveBeenCalled();
    expect(onWorkspaceChannelOverrideUpdate).not.toHaveBeenCalled();
    expect(onWorkspaceIpBanSync).not.toHaveBeenCalled();
    expect(onPresenceSync).not.toHaveBeenCalled();
    expect(onPresenceUpdate).not.toHaveBeenCalled();
  });

  it("rejects oversized gateway event payloads before dispatch", () => {
    const {
      socket,
      onReady,
      onMessageCreate,
      onMessageUpdate,
      onMessageDelete,
      onMessageReaction,
      onChannelCreate,
      onWorkspaceUpdate,
      onWorkspaceMemberAdd,
      onWorkspaceMemberUpdate,
      onWorkspaceMemberRemove,
      onWorkspaceMemberBan,
      onWorkspaceRoleCreate,
      onWorkspaceRoleUpdate,
      onWorkspaceRoleDelete,
      onWorkspaceRoleReorder,
      onWorkspaceRoleAssignmentAdd,
      onWorkspaceRoleAssignmentRemove,
      onWorkspaceChannelOverrideUpdate,
      onWorkspaceIpBanSync,
      onPresenceSync,
      onPresenceUpdate,
    } = createOpenGateway();
    socket.emitMessage("x".repeat(70 * 1024));

    expect(onReady).not.toHaveBeenCalled();
    expect(onMessageCreate).not.toHaveBeenCalled();
    expect(onMessageUpdate).not.toHaveBeenCalled();
    expect(onMessageDelete).not.toHaveBeenCalled();
    expect(onMessageReaction).not.toHaveBeenCalled();
    expect(onChannelCreate).not.toHaveBeenCalled();
    expect(onWorkspaceUpdate).not.toHaveBeenCalled();
    expect(onWorkspaceMemberAdd).not.toHaveBeenCalled();
    expect(onWorkspaceMemberUpdate).not.toHaveBeenCalled();
    expect(onWorkspaceMemberRemove).not.toHaveBeenCalled();
    expect(onWorkspaceMemberBan).not.toHaveBeenCalled();
    expect(onWorkspaceRoleCreate).not.toHaveBeenCalled();
    expect(onWorkspaceRoleUpdate).not.toHaveBeenCalled();
    expect(onWorkspaceRoleDelete).not.toHaveBeenCalled();
    expect(onWorkspaceRoleReorder).not.toHaveBeenCalled();
    expect(onWorkspaceRoleAssignmentAdd).not.toHaveBeenCalled();
    expect(onWorkspaceRoleAssignmentRemove).not.toHaveBeenCalled();
    expect(onWorkspaceChannelOverrideUpdate).not.toHaveBeenCalled();
    expect(onWorkspaceIpBanSync).not.toHaveBeenCalled();
    expect(onPresenceSync).not.toHaveBeenCalled();
    expect(onPresenceUpdate).not.toHaveBeenCalled();
  });

  it("rejects oversized presence_sync user lists", () => {
    const { socket, onPresenceSync } = createOpenGateway();
    const oversizedUserIds = Array.from({ length: 1025 }, (_, index) => ulidFromIndex(index));

    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "presence_sync",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          user_ids: oversizedUserIds,
        },
      }),
    );

    expect(onPresenceSync).not.toHaveBeenCalled();
  });

  it("deduplicates repeated presence_sync user IDs while preserving order", () => {
    const { socket, onPresenceSync } = createOpenGateway();
    const firstUserId = ulidFromIndex(1);
    const secondUserId = ulidFromIndex(2);

    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "presence_sync",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          user_ids: [firstUserId, firstUserId, secondUserId, firstUserId],
        },
      }),
    );

    expect(onPresenceSync).toHaveBeenCalledTimes(1);
    expect(onPresenceSync).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      userIds: [firstUserId, secondUserId],
    });
  });

  it("keeps valid gateway payload compatibility", () => {
    const {
      socket,
      onReady,
      onMessageCreate,
      onMessageUpdate,
      onMessageDelete,
      onMessageReaction,
      onChannelCreate,
      onWorkspaceUpdate,
      onWorkspaceMemberAdd,
      onWorkspaceMemberUpdate,
      onWorkspaceMemberRemove,
      onWorkspaceMemberBan,
      onWorkspaceRoleCreate,
      onWorkspaceRoleUpdate,
      onWorkspaceRoleDelete,
      onWorkspaceRoleReorder,
      onWorkspaceRoleAssignmentAdd,
      onWorkspaceRoleAssignmentRemove,
      onWorkspaceChannelOverrideUpdate,
      onWorkspaceIpBanSync,
      onPresenceSync,
      onPresenceUpdate,
    } = createOpenGateway();
    const messageId = ulidFromIndex(3);
    const authorId = ulidFromIndex(4);
    const presenceUserId = ulidFromIndex(5);

    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "ready",
        d: {
          user_id: authorId,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "message_create",
        d: {
          message_id: messageId,
          guild_id: DEFAULT_GUILD_ID,
          channel_id: DEFAULT_CHANNEL_ID,
          author_id: authorId,
          content: "hello",
          markdown_tokens: [{ type: "text", text: "hello" }],
          attachments: [],
          created_at_unix: 1,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "message_update",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          channel_id: DEFAULT_CHANNEL_ID,
          message_id: messageId,
          updated_fields: {
            content: "updated",
            markdown_tokens: [{ type: "text", text: "updated" }],
          },
          updated_at_unix: 2,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "message_delete",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          channel_id: DEFAULT_CHANNEL_ID,
          message_id: messageId,
          deleted_at_unix: 3,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "message_reaction",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          channel_id: DEFAULT_CHANNEL_ID,
          message_id: messageId,
          emoji: "👍",
          count: 2,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "channel_create",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          channel: {
            channel_id: ulidFromIndex(7),
            name: "bridge-call",
            kind: "voice",
          },
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "workspace_update",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          updated_fields: {
            name: "Ops Live",
            visibility: "public",
          },
          updated_at_unix: 4,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "workspace_member_add",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          user_id: presenceUserId,
          role: "member",
          joined_at_unix: 5,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "workspace_member_update",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          user_id: presenceUserId,
          updated_fields: {
            role: "moderator",
          },
          updated_at_unix: 6,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "workspace_member_remove",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          user_id: presenceUserId,
          reason: "kick",
          removed_at_unix: 7,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "workspace_member_ban",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          user_id: presenceUserId,
          banned_at_unix: 8,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "workspace_role_create",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          role: {
            role_id: ulidFromIndex(30),
            name: "ops_admin",
            position: 90,
            is_system: false,
            permissions: ["manage_roles"],
          },
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "workspace_role_update",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          role_id: ulidFromIndex(30),
          updated_fields: {
            name: "ops_admin_v2",
          },
          updated_at_unix: 9,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "workspace_role_delete",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          role_id: ulidFromIndex(30),
          deleted_at_unix: 10,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "workspace_role_reorder",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          role_ids: [ulidFromIndex(31), ulidFromIndex(32)],
          updated_at_unix: 11,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "workspace_role_assignment_add",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          user_id: presenceUserId,
          role_id: ulidFromIndex(31),
          assigned_at_unix: 12,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "workspace_role_assignment_remove",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          user_id: presenceUserId,
          role_id: ulidFromIndex(31),
          removed_at_unix: 13,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "workspace_channel_override_update",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          channel_id: DEFAULT_CHANNEL_ID,
          role: "moderator",
          updated_fields: {
            allow: ["create_message"],
            deny: ["ban_member"],
          },
          updated_at_unix: 14,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "workspace_ip_ban_sync",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          summary: {
            action: "upsert",
            changed_count: 2,
          },
          updated_at_unix: 15,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "presence_sync",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          user_ids: [presenceUserId],
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "presence_update",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          user_id: presenceUserId,
          status: "offline",
        },
      }),
    );

    expect(onReady).toHaveBeenCalledTimes(1);
    expect(onReady).toHaveBeenCalledWith({ userId: authorId });
    expect(onMessageCreate).toHaveBeenCalledTimes(1);
    expect(onMessageUpdate).toHaveBeenCalledTimes(1);
    expect(onMessageDelete).toHaveBeenCalledTimes(1);
    expect(onMessageReaction).toHaveBeenCalledTimes(1);
    expect(onChannelCreate).toHaveBeenCalledTimes(1);
    expect(onWorkspaceUpdate).toHaveBeenCalledTimes(1);
    expect(onWorkspaceMemberAdd).toHaveBeenCalledTimes(1);
    expect(onWorkspaceMemberUpdate).toHaveBeenCalledTimes(1);
    expect(onWorkspaceMemberRemove).toHaveBeenCalledTimes(1);
    expect(onWorkspaceMemberBan).toHaveBeenCalledTimes(1);
    expect(onWorkspaceRoleCreate).toHaveBeenCalledTimes(1);
    expect(onWorkspaceRoleUpdate).toHaveBeenCalledTimes(1);
    expect(onWorkspaceRoleDelete).toHaveBeenCalledTimes(1);
    expect(onWorkspaceRoleReorder).toHaveBeenCalledTimes(1);
    expect(onWorkspaceRoleAssignmentAdd).toHaveBeenCalledTimes(1);
    expect(onWorkspaceRoleAssignmentRemove).toHaveBeenCalledTimes(1);
    expect(onWorkspaceChannelOverrideUpdate).toHaveBeenCalledTimes(1);
    expect(onWorkspaceIpBanSync).toHaveBeenCalledTimes(1);
    expect(onMessageReaction).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      channelId: DEFAULT_CHANNEL_ID,
      messageId,
      emoji: "👍",
      count: 2,
    });
    expect(onMessageUpdate).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      channelId: DEFAULT_CHANNEL_ID,
      messageId,
      updatedFields: {
        content: "updated",
        markdownTokens: [{ type: "text", text: "updated" }],
      },
      updatedAtUnix: 2,
    });
    expect(onMessageDelete).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      channelId: DEFAULT_CHANNEL_ID,
      messageId,
      deletedAtUnix: 3,
    });
    expect(onChannelCreate).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      channel: {
        channelId: ulidFromIndex(7),
        name: "bridge-call",
        kind: "voice",
      },
    });
    expect(onWorkspaceUpdate).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      updatedFields: {
        name: "Ops Live",
        visibility: "public",
      },
      updatedAtUnix: 4,
    });
    expect(onWorkspaceMemberAdd).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      userId: presenceUserId,
      role: "member",
      joinedAtUnix: 5,
    });
    expect(onWorkspaceMemberUpdate).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      userId: presenceUserId,
      updatedFields: {
        role: "moderator",
      },
      updatedAtUnix: 6,
    });
    expect(onWorkspaceMemberRemove).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      userId: presenceUserId,
      reason: "kick",
      removedAtUnix: 7,
    });
    expect(onWorkspaceMemberBan).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      userId: presenceUserId,
      bannedAtUnix: 8,
    });
    expect(onWorkspaceRoleCreate).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      role: {
        roleId: ulidFromIndex(30),
        name: "ops_admin",
        position: 90,
        isSystem: false,
        permissions: ["manage_roles"],
      },
    });
    expect(onWorkspaceRoleUpdate).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      roleId: ulidFromIndex(30),
      updatedFields: {
        name: "ops_admin_v2",
      },
      updatedAtUnix: 9,
    });
    expect(onWorkspaceRoleDelete).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      roleId: ulidFromIndex(30),
      deletedAtUnix: 10,
    });
    expect(onWorkspaceRoleReorder).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      roleIds: [ulidFromIndex(31), ulidFromIndex(32)],
      updatedAtUnix: 11,
    });
    expect(onWorkspaceRoleAssignmentAdd).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      userId: presenceUserId,
      roleId: ulidFromIndex(31),
      assignedAtUnix: 12,
    });
    expect(onWorkspaceRoleAssignmentRemove).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      userId: presenceUserId,
      roleId: ulidFromIndex(31),
      removedAtUnix: 13,
    });
    expect(onWorkspaceChannelOverrideUpdate).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      channelId: DEFAULT_CHANNEL_ID,
      role: "moderator",
      updatedFields: {
        allow: ["create_message"],
        deny: ["ban_member"],
      },
      updatedAtUnix: 14,
    });
    expect(onWorkspaceIpBanSync).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      summary: {
        action: "upsert",
        changedCount: 2,
      },
      updatedAtUnix: 15,
    });
    expect(onPresenceSync).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      userIds: [presenceUserId],
    });
    expect(onPresenceUpdate).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      userId: presenceUserId,
      status: "offline",
    });
  });

  it("rejects malformed message_update and message_delete payloads", () => {
    const { socket, onMessageUpdate, onMessageDelete } = createOpenGateway();
    const messageId = ulidFromIndex(8);
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "message_update",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          channel_id: DEFAULT_CHANNEL_ID,
          message_id: messageId,
          updated_fields: {},
          updated_at_unix: 2,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "message_update",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          channel_id: DEFAULT_CHANNEL_ID,
          message_id: messageId,
          updated_fields: {
            content: "ok",
          },
          updated_at_unix: 0,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "message_delete",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          channel_id: DEFAULT_CHANNEL_ID,
          message_id: messageId,
          deleted_at_unix: 0,
        },
      }),
    );

    expect(onMessageUpdate).not.toHaveBeenCalled();
    expect(onMessageDelete).not.toHaveBeenCalled();
  });

  it("keeps invalid payload handling fail-closed and allows later valid payloads", () => {
    const { socket, onPresenceSync } = createOpenGateway();
    const validUserId = ulidFromIndex(6);

    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "presence_sync",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          user_ids: ["bad-id", validUserId],
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "presence_sync",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          user_ids: [validUserId],
        },
      }),
    );

    expect(onPresenceSync).toHaveBeenCalledTimes(1);
    expect(onPresenceSync).toHaveBeenCalledWith({
      guildId: DEFAULT_GUILD_ID,
      userIds: [validUserId],
    });
  });

  it("parses profile and friendship events with strict payload validation", () => {
    const {
      socket,
      onProfileUpdate,
      onProfileAvatarUpdate,
      onFriendRequestCreate,
      onFriendRequestUpdate,
      onFriendRequestDelete,
      onFriendRemove,
    } = createOpenGateway();
    const alice = ulidFromIndex(7);
    const bob = ulidFromIndex(9);

    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "profile_update",
        d: {
          user_id: alice,
          updated_fields: {
            username: "alice-updated",
            about_markdown: "about",
            about_markdown_tokens: [{ type: "text", text: "about" }],
          },
          updated_at_unix: 3,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "profile_avatar_update",
        d: {
          user_id: alice,
          avatar_version: 5,
          updated_at_unix: 4,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "friend_request_create",
        d: {
          request_id: ulidFromIndex(10),
          sender_user_id: alice,
          sender_username: "alice",
          recipient_user_id: bob,
          recipient_username: "bob",
          created_at_unix: 5,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "friend_request_update",
        d: {
          request_id: ulidFromIndex(10),
          state: "accepted",
          user_id: alice,
          friend_user_id: bob,
          friend_username: "bob",
          friendship_created_at_unix: 6,
          updated_at_unix: 7,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "friend_request_delete",
        d: {
          request_id: ulidFromIndex(11),
          deleted_at_unix: 8,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "friend_remove",
        d: {
          user_id: alice,
          friend_user_id: bob,
          removed_at_unix: 9,
        },
      }),
    );

    expect(onProfileUpdate).toHaveBeenCalledTimes(1);
    expect(onProfileAvatarUpdate).toHaveBeenCalledTimes(1);
    expect(onFriendRequestCreate).toHaveBeenCalledTimes(1);
    expect(onFriendRequestUpdate).toHaveBeenCalledTimes(1);
    expect(onFriendRequestDelete).toHaveBeenCalledTimes(1);
    expect(onFriendRemove).toHaveBeenCalledTimes(1);
  });

  it("rejects malformed profile and friendship payloads", () => {
    const { socket, onProfileUpdate, onFriendRequestUpdate } = createOpenGateway();
    const alice = ulidFromIndex(12);
    const bob = ulidFromIndex(13);

    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "profile_update",
        d: {
          user_id: alice,
          updated_fields: {
            about_markdown_tokens: [{ type: "text", text: "missing markdown text" }],
          },
          updated_at_unix: 3,
        },
      }),
    );
    socket.emitMessage(
      JSON.stringify({
        v: 1,
        t: "friend_request_update",
        d: {
          request_id: ulidFromIndex(14),
          state: "accepted",
          user_id: alice,
          friend_user_id: bob,
          friend_username: "",
          friendship_created_at_unix: 6,
          updated_at_unix: 7,
        },
      }),
    );

    expect(onProfileUpdate).not.toHaveBeenCalled();
    expect(onFriendRequestUpdate).not.toHaveBeenCalled();
  });

  it("sends subscribe events on open and updateSubscription", () => {
    const { socket, client, onOpenStateChange } = createOpenGateway();
    const nextGuildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");
    const nextChannelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAY");

    client.updateSubscription(nextGuildId, nextChannelId);

    expect(socket.sent).toEqual([
      JSON.stringify({
        v: 1,
        t: "subscribe",
        d: {
          guild_id: DEFAULT_GUILD_ID,
          channel_id: DEFAULT_CHANNEL_ID,
        },
      }),
      JSON.stringify({
        v: 1,
        t: "subscribe",
        d: {
          guild_id: nextGuildId,
          channel_id: nextChannelId,
        },
      }),
    ]);

    client.close();
    expect(onOpenStateChange).toHaveBeenCalledWith(true);
    expect(socket.readyState).toBe(MockWebSocket.CLOSED);
  });
});
