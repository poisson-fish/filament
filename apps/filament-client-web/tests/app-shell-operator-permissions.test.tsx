import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";

const SESSION_STORAGE_KEY = "filament.auth.session.v1";
const WORKSPACE_CACHE_KEY = "filament.workspace.cache.v1";

const ACCESS_TOKEN = "A".repeat(64);
const REFRESH_TOKEN = "B".repeat(64);

const GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const OWNER_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const MEMBER_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const TARGET_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

type FixtureRole = "owner" | "member";

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

function noContentResponse(): Response {
  return new Response(null, { status: 204 });
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

function createOperatorFixtureFetch(role: FixtureRole) {
  return vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = requestUrl(input);
    const method = requestMethod(init);

    if (method === "GET" && url.includes("/auth/me")) {
      return jsonResponse({
        user_id: role === "owner" ? OWNER_USER_ID : MEMBER_USER_ID,
        username: role,
      });
    }

    if (method === "GET" && url.endsWith("/guilds")) {
      return jsonResponse({
        guilds: [{ guild_id: GUILD_ID, name: "Security Ops", visibility: "private" }],
      });
    }

    if (method === "GET" && url.endsWith(`/guilds/${GUILD_ID}/channels`)) {
      return jsonResponse({
        channels: [{ channel_id: CHANNEL_ID, name: "incident-room" }],
      });
    }

    if (
      method === "GET" &&
      url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/permissions/self`)
    ) {
      return role === "owner"
        ? jsonResponse({
            role: "owner",
            permissions: [
              "manage_roles",
              "manage_channel_overrides",
              "delete_message",
              "ban_member",
              "create_message",
              "publish_video",
              "publish_screen_share",
              "subscribe_streams",
            ],
          })
        : jsonResponse({
            role: "member",
            permissions: ["create_message", "subscribe_streams"],
          });
    }

    if (
      method === "GET" &&
      url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/messages`)
    ) {
      return jsonResponse({
        messages: [],
        next_before: null,
      });
    }

    if (method === "GET" && url.includes("/guilds/public")) {
      return jsonResponse({ guilds: [] });
    }

    if (method === "POST" && url.includes(`/guilds/${GUILD_ID}/members/${TARGET_USER_ID}`)) {
      return role === "owner"
        ? jsonResponse({ accepted: true })
        : jsonResponse({ error: "forbidden" }, 403);
    }

    if (method === "PATCH" && url.includes(`/guilds/${GUILD_ID}/members/${TARGET_USER_ID}`)) {
      return role === "owner"
        ? jsonResponse({ accepted: true })
        : jsonResponse({ error: "forbidden" }, 403);
    }

    if (
      method === "POST" &&
      url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/overrides/member`)
    ) {
      return role === "owner"
        ? jsonResponse({ accepted: true })
        : jsonResponse({ error: "forbidden" }, 403);
    }

    if (method === "POST" && url.includes(`/guilds/${GUILD_ID}/search/rebuild`)) {
      return role === "owner"
        ? noContentResponse()
        : jsonResponse({ error: "forbidden" }, 403);
    }

    if (method === "POST" && url.includes(`/guilds/${GUILD_ID}/search/reconcile`)) {
      return role === "owner"
        ? jsonResponse({ upserted: 2, deleted: 1 })
        : jsonResponse({ error: "forbidden" }, 403);
    }

    if (
      method === "POST" &&
      url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/voice/token`)
    ) {
      return role === "owner"
        ? jsonResponse({
            token: "T".repeat(96),
            livekit_url: "wss://livekit.example.com",
            room: "filament.voice.abc.def",
            identity: "u.abc.123",
            can_publish: true,
            can_subscribe: false,
            publish_sources: ["microphone"],
            expires_in_secs: 300,
          })
        : jsonResponse({ error: "forbidden" }, 403);
    }

    return jsonResponse({ error: "not_found" }, 404);
  });
}

describe("operator console permission fixtures", () => {
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

  it("executes privileged operator flows for owner fixtures", async () => {
    seedAuthenticatedWorkspace();
    vi.stubGlobal("fetch", createOperatorFixtureFetch("owner"));
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    expect(await screen.findByRole("heading", { name: "Ops Console" })).toBeInTheDocument();

    fireEvent.click(await screen.findByRole("button", { name: "Open search panel" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Rebuild Index" })).toBeEnabled(),
    );
    expect(screen.getByRole("button", { name: "Reconcile Index" })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    fireEvent.click(screen.getByRole("button", { name: "Open voice panel" }));
    expect(screen.getByRole("button", { name: "Issue token" })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    fireEvent.click(screen.getByRole("button", { name: "Open moderation panel" }));
    expect(screen.getByRole("button", { name: "Apply channel override" })).toBeEnabled();

    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.queryByRole("button", { name: "Issue token" })).not.toBeInTheDocument();
  });

  it("hides privileged operator controls for restricted member fixtures", async () => {
    seedAuthenticatedWorkspace();
    vi.stubGlobal("fetch", createOperatorFixtureFetch("member"));
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    expect(await screen.findByRole("heading", { name: "Ops Console" })).toBeInTheDocument();

    fireEvent.click(await screen.findByRole("button", { name: "Open voice panel" }));
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Issue token" })).toBeEnabled(),
    );
    fireEvent.click(screen.getByRole("button", { name: "Back" }));

    fireEvent.click(screen.getByRole("button", { name: "Open search panel" }));
    expect(screen.queryByRole("button", { name: "Rebuild Index" })).not.toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Back" }));

    expect(screen.queryByRole("button", { name: "Apply channel override" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Add" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Ban" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open moderation panel" })).not.toBeInTheDocument();
  });
});
