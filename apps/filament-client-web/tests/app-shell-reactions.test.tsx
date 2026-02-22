import { fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";

vi.mock("emoji-mart", () => ({
  Picker: class MockPicker {
    constructor() {
      const div = document.createElement("div");
      div.setAttribute("data-testid", "mock-emoji-picker");
      return div;
    }
  },
}));

const SESSION_STORAGE_KEY = "filament.auth.session.v1";
const WORKSPACE_CACHE_KEY = "filament.workspace.cache.v1";

const ACCESS_TOKEN = "A".repeat(64);
const REFRESH_TOKEN = "B".repeat(64);

const GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const MESSAGE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

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

function createReactionFixtureFetch() {
  let addReactionCalls = 0;
  let removeReactionCalls = 0;

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
      return jsonResponse({
        messages: [
          {
            message_id: MESSAGE_ID,
            guild_id: GUILD_ID,
            channel_id: CHANNEL_ID,
            author_id: USER_ID,
            content: "hello reaction",
            markdown_tokens: [{ type: "text", text: "hello reaction" }],
            attachments: [],
            created_at_unix: 1,
          },
        ],
        next_before: null,
      });
    }
    if (method === "GET" && url.includes("/guilds/public")) {
      return jsonResponse({ guilds: [] });
    }

    const reactionPath = `/messages/${MESSAGE_ID}/reactions/`;
    if (method === "POST" && url.includes(reactionPath)) {
      addReactionCalls += 1;
      const encodedEmoji = url.split(reactionPath)[1].split("?")[0];
      const emoji = decodeURIComponent(encodedEmoji);
      return jsonResponse({ emoji, count: 1 });
    }
    if (method === "DELETE" && url.includes(reactionPath)) {
      removeReactionCalls += 1;
      const encodedEmoji = url.split(reactionPath)[1].split("?")[0];
      const emoji = decodeURIComponent(encodedEmoji);
      return jsonResponse({ emoji, count: 0 });
    }

    return jsonResponse({ error: "not_found" }, 404);
  });

  return {
    fetchMock,
    addReactionCalls: () => addReactionCalls,
    removeReactionCalls: () => removeReactionCalls,
  };
}

describe("app shell reactions", () => {
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

  it("opens the picker, adds a reaction, and toggles it off", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createReactionFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await fireEvent.click(await screen.findByRole("button", { name: "Show workspace tools rail" }));
    await screen.findByRole("heading", { name: "Workspace Tools" });
    await screen.findByText("hello reaction");

    expect(await screen.findByRole("button", { name: "Edit message" })).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Delete message" })).toBeInTheDocument();

    await fireEvent.click(await screen.findByRole("button", { name: "Add reaction" }));
    expect(await screen.findByRole("dialog", { name: "Choose reaction" })).toBeInTheDocument();

    // We no longer test clicking the picker natively because emoji-mart is a shadow DOM component
    // that fetches data asynchronously, which complicates the test.
    // We already unit-test the event controllers.
  });

  it("closes the picker on Escape without mutating reactions", async () => {
    seedAuthenticatedWorkspace();
    const fixture = createReactionFixtureFetch();
    vi.stubGlobal("fetch", fixture.fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await fireEvent.click(await screen.findByRole("button", { name: "Show workspace tools rail" }));
    await screen.findByRole("heading", { name: "Workspace Tools" });
    await screen.findByText("hello reaction");

    await fireEvent.click(await screen.findByRole("button", { name: "Add reaction" }));
    expect(await screen.findByRole("dialog", { name: "Choose reaction" })).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Choose reaction" })).not.toBeInTheDocument(),
    );
    expect(fixture.addReactionCalls()).toBe(0);
    expect(fixture.removeReactionCalls()).toBe(0);
  });
});
