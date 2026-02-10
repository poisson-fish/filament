import { render, screen, waitFor } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";

const SESSION_STORAGE_KEY = "filament.auth.session.v1";
const WORKSPACE_CACHE_KEY = "filament.workspace.cache.v1";

const ACCESS_TOKEN = "A".repeat(64);
const REFRESH_TOKEN = "B".repeat(64);
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const USERNAME = "alice";

const MEMBER_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const PRIVATE_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const MEMBER_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

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
        guildId: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
        guildName: "Stale Guild",
        channels: [{ channelId: "01ARZ3NDEKTSV4RRFFQ69G5FAB", name: "stale-room" }],
      },
    ]),
  );
}

describe("app shell workspace visibility", () => {
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

  it("uses server-driven workspace discovery and persists only accessible workspaces", async () => {
    seedSessionAndWorkspaceCache();

    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const url = requestUrl(input);

      if (url.includes("/auth/me")) {
        return jsonResponse({
          user_id: USER_ID,
          username: USERNAME,
        });
      }

      if (url.endsWith("/guilds")) {
        return jsonResponse({
          guilds: [
            {
              guild_id: MEMBER_GUILD_ID,
              name: "Member Guild",
              visibility: "private",
            },
            {
              guild_id: PRIVATE_GUILD_ID,
              name: "Private Guild",
              visibility: "private",
            },
          ],
        });
      }

      if (url.endsWith(`/guilds/${MEMBER_GUILD_ID}/channels`)) {
        return jsonResponse({
          channels: [{ channel_id: MEMBER_CHANNEL_ID, name: "incident-room" }],
        });
      }

      if (url.endsWith(`/guilds/${PRIVATE_GUILD_ID}/channels`)) {
        return jsonResponse({ error: "forbidden" }, 403);
      }

      if (
        url.includes(`/guilds/${MEMBER_GUILD_ID}/channels/${MEMBER_CHANNEL_ID}/permissions/self`) ||
        url.includes(`/guilds/${MEMBER_GUILD_ID}/channels/${MEMBER_CHANNEL_ID}/messages?limit=50`)
      ) {
        if (url.includes("/permissions/self")) {
          return jsonResponse({
            role: "member",
            permissions: ["create_message", "subscribe_streams"],
          });
        }
        return jsonResponse({ messages: [], next_before: null });
      }

      if (url.includes("/guilds/public")) {
        return jsonResponse({ guilds: [] });
      }

      return jsonResponse({ error: "not_found" }, 404);
    });

    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    expect(await screen.findByTitle("Member Guild (private)")).toBeInTheDocument();
    await waitFor(() => {
      expect(screen.queryByTitle("Private Guild (private)")).not.toBeInTheDocument();
    });

    const persisted = JSON.parse(window.localStorage.getItem(WORKSPACE_CACHE_KEY) ?? "[]") as Array<{
      guildId: string;
    }>;
    expect(persisted).toHaveLength(1);
    expect(persisted[0]?.guildId).toBe(MEMBER_GUILD_ID);
  });
});
