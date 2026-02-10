import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";

const SESSION_STORAGE_KEY = "filament.auth.session.v1";
const WORKSPACE_CACHE_KEY = "filament.workspace.cache.v1";

const ACCESS_TOKEN = "A".repeat(64);
const REFRESH_TOKEN = "B".repeat(64);
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const USERNAME = "alice";

const GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";

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

function seedSessionAndWorkspaceCache(): void {
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
        guildName: "Member Guild",
        channels: [{ channelId: CHANNEL_ID, name: "incident-room" }],
      },
    ]),
  );
}

describe("app shell public discovery", () => {
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

  it("loads and searches the authenticated public guild directory", async () => {
    seedSessionAndWorkspaceCache();
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const url = requestUrl(input);
      if (url.includes("/auth/me")) {
        return jsonResponse({ user_id: USER_ID, username: USERNAME });
      }
      if (url.endsWith("/guilds")) {
        return jsonResponse({
          guilds: [{ guild_id: GUILD_ID, name: "Member Guild", visibility: "private" }],
        });
      }
      if (url.endsWith(`/guilds/${GUILD_ID}/channels`)) {
        return jsonResponse({
          channels: [{ channel_id: CHANNEL_ID, name: "incident-room" }],
        });
      }
      if (
        url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/permissions/self`) ||
        url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/messages?limit=50`)
      ) {
        if (url.includes("/permissions/self")) {
          return jsonResponse({
            role: "member",
            permissions: ["create_message", "subscribe_streams"],
          });
        }
        return jsonResponse({ messages: [], next_before: null });
      }
      if (url.includes("/guilds/public?q=lobby")) {
        return jsonResponse({
          guilds: [
            {
              guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAY",
              name: "Public Lobby",
              visibility: "public",
            },
          ],
        });
      }
      if (url.includes("/guilds/public")) {
        return jsonResponse({
          guilds: [
            {
              guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAZ",
              name: "Town Hall",
              visibility: "public",
            },
          ],
        });
      }
      return jsonResponse({ error: "not_found" }, 404);
    });
    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await fireEvent.click(
      await screen.findByRole("button", { name: "Open public workspace directory panel" }),
    );
    expect(await screen.findByText("Town Hall")).toBeInTheDocument();

    await fireEvent.input(screen.getByLabelText("Search"), { target: { value: "lobby" } });
    const searchButton = await screen.findByRole("button", { name: "Find public" });
    await fireEvent.click(searchButton);

    await waitFor(() => expect(screen.getByText("Public Lobby")).toBeInTheDocument());
  });
});
