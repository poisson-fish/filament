import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";

const SESSION_STORAGE_KEY = "filament.auth.session.v1";
const WORKSPACE_CACHE_KEY = "filament.workspace.cache.v1";

const ACCESS_TOKEN = "A".repeat(64);
const REFRESH_TOKEN = "B".repeat(64);
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
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
        channels: [{ channelId: CHANNEL_ID, name: "incident-room", kind: "text" }],
      },
    ]),
  );
}

function createSettingsFixtureFetch() {
  return vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = requestUrl(input);
    const method = requestMethod(init);

    if (method === "GET" && url.includes("/auth/me")) {
      return jsonResponse({ user_id: USER_ID, username: "owner" });
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
      return jsonResponse({ role: "owner", permissions: ["create_message"] });
    }
    if (
      method === "GET" &&
      url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/messages`)
    ) {
      return jsonResponse({ messages: [], next_before: null });
    }
    if (method === "GET" && url.includes("/guilds/public")) {
      return jsonResponse({ guilds: [] });
    }
    if (method === "GET" && url.endsWith("/friends")) {
      return jsonResponse({ friends: [] });
    }
    if (method === "GET" && url.endsWith("/friends/requests")) {
      return jsonResponse({ incoming: [], outgoing: [] });
    }
    return jsonResponse({ error: "not_found" }, 404);
  });
}

describe("app shell settings entry point", () => {
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

  it("opens and closes settings panel from global app-shell action", async () => {
    seedAuthenticatedWorkspace();
    vi.stubGlobal("fetch", createSettingsFixtureFetch());
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Open settings panel" }));
    expect(await screen.findByRole("dialog", { name: "Settings panel" })).toBeInTheDocument();
    expect(screen.getByLabelText("Settings category rail")).toBeInTheDocument();
    expect(screen.getByLabelText("Settings content pane")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Voice settings category" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.getByRole("button", { name: "Open Profile settings category" })).toBeInTheDocument();
    expect(screen.getByLabelText("Voice settings submenu")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Voice Audio Devices submenu" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.getByText(/Audio Devices page is active/i)).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Settings panel" })).not.toBeInTheDocument(),
    );
  });

  it("closes settings panel on Escape", async () => {
    seedAuthenticatedWorkspace();
    vi.stubGlobal("fetch", createSettingsFixtureFetch());
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Open settings panel" }));
    expect(await screen.findByRole("dialog", { name: "Settings panel" })).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Settings panel" })).not.toBeInTheDocument(),
    );
  });

  it("switches between Voice and Profile categories", async () => {
    seedAuthenticatedWorkspace();
    vi.stubGlobal("fetch", createSettingsFixtureFetch());
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Open settings panel" }));
    expect(await screen.findByRole("dialog", { name: "Settings panel" })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Open Profile settings category" }));
    expect(screen.getByText(/Profile settings remain a non-functional placeholder/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Open Profile settings category" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.getByRole("button", { name: "Open Voice settings category" })).not.toHaveAttribute(
      "aria-current",
    );

    fireEvent.click(screen.getByRole("button", { name: "Open Voice settings category" }));
    expect(screen.getByRole("button", { name: "Open Voice Audio Devices submenu" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(screen.getByText(/Audio Devices page is active/i)).toBeInTheDocument();
  });
});
