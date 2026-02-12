import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";

const SESSION_STORAGE_KEY = "filament.auth.session.v1";
const WORKSPACE_CACHE_KEY = "filament.workspace.cache.v1";

const ACCESS_TOKEN = "A".repeat(64);
const REFRESH_TOKEN = "B".repeat(64);

const GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const ALICE_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const BOB_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const REQUEST_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

function createStorageMock(): Storage {
  const store = new Map<string, string>();
  return {
    get length() {
      return store.size;
    },
    clear() {
      store.clear();
    },
    getItem(key: string) {
      return store.has(key) ? store.get(key)! : null;
    },
    key(index: number) {
      return [...store.keys()][index] ?? null;
    },
    removeItem(key: string) {
      store.delete(key);
    },
    setItem(key: string, value: string) {
      store.set(key, value);
    },
  };
}

function jsonResponse(body: unknown, status = 200): Response {
  return new Response(JSON.stringify(body), {
    status,
    headers: { "content-type": "application/json" },
  });
}

function requestUrl(input: RequestInfo | URL): string {
  if (typeof input === "string") {
    return input;
  }
  if (input instanceof URL) {
    return input.toString();
  }
  return input.url;
}

function requestMethod(init?: RequestInit): string {
  return (init?.method ?? "GET").toUpperCase();
}

function seedAuthenticatedWorkspace(): void {
  window.sessionStorage.setItem(
    SESSION_STORAGE_KEY,
    JSON.stringify({
      accessToken: ACCESS_TOKEN,
      refreshToken: REFRESH_TOKEN,
      expiresAtUnix: Math.floor(Date.now() / 1000) + 3600,
    }),
  );

  window.localStorage.setItem(
    WORKSPACE_CACHE_KEY,
    JSON.stringify([
      {
        guildId: GUILD_ID,
        guildName: "Security Ops",
        channels: [{ channelId: CHANNEL_ID, name: "incident-room" }],
      },
    ]),
  );
}

describe("app shell friendship flows", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
    const storage = createStorageMock();
    vi.stubGlobal("localStorage", storage);
    Object.defineProperty(window, "localStorage", {
      value: storage,
      configurable: true,
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("accepts incoming requests and removes friends", async () => {
    seedAuthenticatedWorkspace();

    let incoming = [
      {
        request_id: REQUEST_ID,
        sender_user_id: BOB_USER_ID,
        sender_username: "bob",
        recipient_user_id: ALICE_USER_ID,
        recipient_username: "alice",
        created_at_unix: 1,
      },
    ];
    let friends: Array<{ user_id: string; username: string; created_at_unix: number }> = [];

    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = requestUrl(input);
      const method = requestMethod(init);

      if (method === "GET" && url.includes("/auth/me")) {
        return jsonResponse({ user_id: ALICE_USER_ID, username: "alice" });
      }
      if (method === "GET" && url.endsWith("/guilds")) {
        return jsonResponse({
          guilds: [{ guild_id: GUILD_ID, name: "Security Ops", visibility: "private" }],
        });
      }
      if (method === "GET" && url.endsWith(`/guilds/${GUILD_ID}/channels`)) {
        return jsonResponse({
          channels: [{ channel_id: CHANNEL_ID, name: "incident-room", kind: "text" }],
        });
      }
      if (
        method === "GET" &&
        url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/permissions/self`)
      ) {
        return jsonResponse({ role: "member", permissions: ["create_message"] });
      }
      if (method === "GET" && url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/messages`)) {
        return jsonResponse({ messages: [], next_before: null });
      }
      if (method === "GET" && url.includes("/guilds/public")) {
        return jsonResponse({ guilds: [] });
      }
      if (method === "GET" && url.includes("/friends/requests")) {
        return jsonResponse({ incoming, outgoing: [] });
      }
      if (method === "GET" && url.includes("/friends")) {
        return jsonResponse({ friends });
      }
      if (method === "POST" && url.includes(`/friends/requests/${REQUEST_ID}/accept`)) {
        incoming = [];
        friends = [{ user_id: BOB_USER_ID, username: "bob", created_at_unix: 2 }];
        return jsonResponse({ accepted: true });
      }
      if (method === "DELETE" && url.includes(`/friends/${BOB_USER_ID}`)) {
        friends = [];
        return new Response(null, { status: 204 });
      }

      return jsonResponse({ error: "not_found" }, 404);
    });

    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await fireEvent.click(await screen.findByRole("button", { name: "Friends" }));
    expect(await screen.findByText("bob")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Accept" }));

    await waitFor(() => expect(screen.getByText("Friend request accepted.")).toBeInTheDocument());
    expect(await screen.findByRole("button", { name: "Remove" })).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    await waitFor(() => expect(screen.getByText("Friend removed.")).toBeInTheDocument());
    expect(await screen.findByText("no-friends")).toBeInTheDocument();
  });
});
