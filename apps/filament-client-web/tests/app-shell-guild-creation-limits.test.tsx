import { fireEvent, render, screen } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";

const SESSION_STORAGE_KEY = "filament.auth.session.v1";
const ACCESS_TOKEN = "A".repeat(64);
const REFRESH_TOKEN = "B".repeat(64);
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const USERNAME = "alice";
const EXISTING_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const EXISTING_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const CREATED_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const CREATED_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";
const WORKSPACE_CACHE_KEY = "filament.workspace.cache.v1";

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

describe("app shell guild creation limits", () => {
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

  it("surfaces server guild creation limit errors in create workspace flow", async () => {
    window.sessionStorage.setItem(
      SESSION_STORAGE_KEY,
      JSON.stringify({
        accessToken: ACCESS_TOKEN,
        refreshToken: REFRESH_TOKEN,
        expiresAtUnix: Math.floor(Date.now() / 1000) + 3600,
      }),
    );

    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = requestUrl(input);
      const method = init?.method ?? "GET";

      if (url.includes("/auth/me")) {
        return jsonResponse({ user_id: USER_ID, username: USERNAME });
      }
      if (method === "GET" && url.endsWith("/guilds")) {
        return jsonResponse({ guilds: [] });
      }
      if (method === "POST" && url.includes("/guilds")) {
        return jsonResponse({ error: "guild_creation_limit_reached" }, 403);
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

    const createButton = await screen.findByRole("button", { name: /^Create workspace$/ });
    await fireEvent.click(createButton);

    expect(await screen.findByText("Guild creation limit reached for this account.")).toBeInTheDocument();
  });

  it("allows creating another workspace when an authenticated user already has one", async () => {
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
          guildId: EXISTING_GUILD_ID,
          guildName: "Existing Guild",
          visibility: "private",
          channels: [{ channelId: EXISTING_CHANNEL_ID, name: "general" }],
        },
      ]),
    );

    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = requestUrl(input);
      const method = init?.method ?? "GET";

      if (url.includes("/auth/me")) {
        return jsonResponse({ user_id: USER_ID, username: USERNAME });
      }
      if (method === "GET" && url.endsWith("/guilds")) {
        return jsonResponse({
          guilds: [{ guild_id: EXISTING_GUILD_ID, name: "Existing Guild", visibility: "private" }],
        });
      }
      if (method === "GET" && url.endsWith(`/guilds/${EXISTING_GUILD_ID}/channels`)) {
        return jsonResponse({
          channels: [{ channel_id: EXISTING_CHANNEL_ID, name: "general" }],
        });
      }
      if (
        method === "GET" &&
        url.includes(`/guilds/${EXISTING_GUILD_ID}/channels/${EXISTING_CHANNEL_ID}/permissions/self`)
      ) {
        return jsonResponse({ role: "owner", permissions: ["create_message", "manage_roles"] });
      }
      if (
        method === "GET" &&
        url.includes(`/guilds/${EXISTING_GUILD_ID}/channels/${EXISTING_CHANNEL_ID}/messages?limit=50`)
      ) {
        return jsonResponse({ messages: [], next_before: null });
      }
      if (method === "POST" && url.endsWith("/guilds")) {
        return jsonResponse({ guild_id: CREATED_GUILD_ID, name: "Security Ops", visibility: "private" });
      }
      if (method === "POST" && url.includes(`/guilds/${CREATED_GUILD_ID}/channels`)) {
        return jsonResponse({ channel_id: CREATED_CHANNEL_ID, name: "incident-room" });
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

    const openCreateForm = await screen.findByRole("button", { name: "New workspace" });
    await fireEvent.click(openCreateForm);
    const createButton = await screen.findByRole("button", { name: /^Create workspace$/ });
    await fireEvent.click(createButton);

    expect(await screen.findByText("Workspace created.")).toBeInTheDocument();
    expect(await screen.findByTitle("Security Ops (private)")).toBeInTheDocument();
  });
});
