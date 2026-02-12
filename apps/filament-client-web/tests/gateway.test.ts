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
    const { socket, onReady, onMessageCreate, onPresenceSync, onPresenceUpdate } = createOpenGateway();
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
    expect(onPresenceSync).not.toHaveBeenCalled();
    expect(onPresenceUpdate).not.toHaveBeenCalled();
  });

  it("rejects oversized gateway event payloads before dispatch", () => {
    const { socket, onReady, onMessageCreate, onPresenceSync, onPresenceUpdate } = createOpenGateway();
    socket.emitMessage("x".repeat(70 * 1024));

    expect(onReady).not.toHaveBeenCalled();
    expect(onMessageCreate).not.toHaveBeenCalled();
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
    const { socket, onReady, onMessageCreate, onPresenceSync, onPresenceUpdate } = createOpenGateway();
    const messageId = ulidFromIndex(3);
    const authorId = ulidFromIndex(4);
    const presenceUserId = ulidFromIndex(5);

    socket.emitMessage(JSON.stringify({ v: 1, t: "ready", d: {} }));
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
    expect(onMessageCreate).toHaveBeenCalledTimes(1);
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
