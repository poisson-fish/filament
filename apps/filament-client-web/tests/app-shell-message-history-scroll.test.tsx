import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";

const SESSION_STORAGE_KEY = "filament.auth.session.v1";
const WORKSPACE_CACHE_KEY = "filament.workspace.cache.v1";

const ACCESS_TOKEN = "A".repeat(64);
const REFRESH_TOKEN = "B".repeat(64);

const GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const LATEST_MESSAGE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const OLDER_MESSAGE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";
const INITIAL_NEXT_BEFORE = "01ARZ3NDEKTSV4RRFFQ69G5FB0";

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

function installMessageListScrollMetrics(element: HTMLElement): {
  setScrollTop: (value: number) => void;
} {
  let scrollTopValue = 900;
  const scrollHeightValue = 1600;
  const clientHeightValue = 560;

  Object.defineProperty(element, "scrollTop", {
    configurable: true,
    get: () => scrollTopValue,
    set: (value: number) => {
      scrollTopValue = Number(value);
    },
  });
  Object.defineProperty(element, "scrollHeight", {
    configurable: true,
    get: () => scrollHeightValue,
  });
  Object.defineProperty(element, "clientHeight", {
    configurable: true,
    get: () => clientHeightValue,
  });

  return {
    setScrollTop: (value: number) => {
      scrollTopValue = value;
    },
  };
}

describe("app shell message history scrolling", () => {
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

  it("auto-loads older messages when scrolling near the top and keeps manual load hidden near latest", async () => {
    seedAuthenticatedWorkspace();

    let olderPageRequests = 0;

    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = requestUrl(input);
      const method = requestMethod(init);

      if (method === "GET" && url.includes("/auth/me")) {
        return jsonResponse({ user_id: USER_ID, username: "alice" });
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
        const parsed = new URL(url, "https://filament.test");
        const before = parsed.searchParams.get("before");
        if (!before) {
          return jsonResponse({
            messages: [
              {
                message_id: LATEST_MESSAGE_ID,
                guild_id: GUILD_ID,
                channel_id: CHANNEL_ID,
                author_id: USER_ID,
                content: "latest message",
                markdown_tokens: [{ type: "text", text: "latest message" }],
                attachments: [],
                created_at_unix: 2,
              },
            ],
            next_before: INITIAL_NEXT_BEFORE,
          });
        }

        if (before === INITIAL_NEXT_BEFORE) {
          olderPageRequests += 1;
          return jsonResponse({
            messages: [
              {
                message_id: OLDER_MESSAGE_ID,
                guild_id: GUILD_ID,
                channel_id: CHANNEL_ID,
                author_id: USER_ID,
                content: "older message",
                markdown_tokens: [{ type: "text", text: "older message" }],
                attachments: [],
                created_at_unix: 1,
              },
            ],
            next_before: null,
          });
        }
      }
      if (method === "GET" && url.includes("/guilds/public")) {
        return jsonResponse({ guilds: [] });
      }

      return jsonResponse({ error: "not_found" }, 404);
    });

    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await screen.findByRole("heading", { name: "Workspace Tools" });
    await screen.findByText("latest message");

    const messageList = document.querySelector(".message-list");
    expect(messageList).not.toBeNull();
    const metrics = installMessageListScrollMetrics(messageList as HTMLElement);

    await fireEvent.scroll(messageList as HTMLElement);
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Load older messages" })).not.toBeInTheDocument(),
    );

    metrics.setScrollTop(80);
    await fireEvent.scroll(messageList as HTMLElement);

    await waitFor(() => expect(olderPageRequests).toBe(1));
    await screen.findByText("older message");
  });
});
