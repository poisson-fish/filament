import { fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";

const SESSION_STORAGE_KEY = "filament.auth.session.v1";
const WORKSPACE_CACHE_KEY = "filament.workspace.cache.v1";

const ACCESS_TOKEN = "A".repeat(64);
const REFRESH_TOKEN = "B".repeat(64);
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

const GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const TEXT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const VOICE_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const CREATED_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";

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
        channels: [{ channelId: TEXT_CHANNEL_ID, name: "incident-room" }],
      },
    ]),
  );
}

describe("app shell channel kinds", () => {
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

  it("groups text and voice channels and posts selected kind when creating a channel", async () => {
    seedAuthenticatedWorkspace();
    let createChannelBody: { name: string; kind: string } | null = null;

    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
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
          channels: [
            { channel_id: TEXT_CHANNEL_ID, name: "incident-room", kind: "text" },
            { channel_id: VOICE_CHANNEL_ID, name: "bridge-call", kind: "voice" },
          ],
        });
      }
      if (
        method === "GET" &&
        url.includes(`/guilds/${GUILD_ID}/channels/`) &&
        url.includes("/permissions/self")
      ) {
        return jsonResponse({
          role: "owner",
          permissions: ["create_message", "manage_channel_overrides", "manage_roles"],
        });
      }
      if (
        method === "GET" &&
        url.includes(`/guilds/${GUILD_ID}/channels/`) &&
        url.includes("/messages")
      ) {
        return jsonResponse({ messages: [], next_before: null });
      }
      if (method === "GET" && url.includes("/guilds/public")) {
        return jsonResponse({ guilds: [] });
      }
      if (method === "POST" && url.includes(`/guilds/${GUILD_ID}/channels`)) {
        createChannelBody = JSON.parse(String(init?.body)) as { name: string; kind: string };
        return jsonResponse({
          channel_id: CREATED_CHANNEL_ID,
          name: createChannelBody.name,
          kind: createChannelBody.kind,
        });
      }

      return jsonResponse({ error: "not_found" }, 404);
    });

    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await fireEvent.click(await screen.findByRole("button", { name: "Show workspace tools rail" }));
    await screen.findByRole("heading", { name: "Workspace Tools" });
    await screen.findByText("TEXT CHANNELS");
    await screen.findByText("VOICE CHANNELS");
    await screen.findByRole("button", { name: "#incident-room" });
    await screen.findByRole("button", { name: "bridge-call" });

    const channelNav = screen.getByRole("navigation", { name: "channels" });
    expect(channelNav.querySelectorAll(".channel-group-header")).toHaveLength(2);

    const textGroupHeader = screen.getByText("TEXT CHANNELS").closest(".channel-group-header");
    expect(textGroupHeader).not.toBeNull();
    const createTextChannelButton = await screen.findByRole("button", { name: "Create text channel" });
    expect(textGroupHeader as HTMLElement).toContainElement(createTextChannelButton);

    const voiceGroupHeader = screen.getByText("VOICE CHANNELS").closest(".channel-group-header");
    expect(voiceGroupHeader).not.toBeNull();
    const createVoiceChannelButton = await screen.findByRole("button", { name: "Create voice channel" });
    expect(voiceGroupHeader as HTMLElement).toContainElement(createVoiceChannelButton);

    await fireEvent.click(createTextChannelButton);
    await fireEvent.input(screen.getByLabelText("Channel name"), {
      target: { value: "war-room" },
    });
    await fireEvent.change(screen.getByLabelText("Channel type"), {
      target: { value: "voice" },
    });
    const createChannelDialog = await screen.findByRole("dialog", { name: "Create channel panel" });
    await fireEvent.click(within(createChannelDialog).getByRole("button", { name: "Create channel" }));

    await waitFor(() =>
      expect(createChannelBody).toEqual({ name: "war-room", kind: "voice" }),
    );
    expect(await screen.findByText("Channel created.")).toBeInTheDocument();
  });

  it("sends first-channel kind from workspace create form", async () => {
    window.sessionStorage.setItem(
      SESSION_STORAGE_KEY,
      JSON.stringify({
        accessToken: ACCESS_TOKEN,
        refreshToken: REFRESH_TOKEN,
        expiresAtUnix: Math.floor(Date.now() / 1000) + 3600,
      }),
    );

    const createdGuildId = "01ARZ3NDEKTSV4RRFFQ69G5FB0";
    let firstChannelBody: { name: string; kind: string } | null = null;

    const fetchMock = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      const url = requestUrl(input);
      const method = requestMethod(init);

      if (method === "GET" && url.includes("/auth/me")) {
        return jsonResponse({ user_id: USER_ID, username: "owner" });
      }
      if (method === "GET" && url.endsWith("/guilds")) {
        return jsonResponse({ guilds: [] });
      }
      if (method === "POST" && url.endsWith("/guilds")) {
        return jsonResponse({
          guild_id: createdGuildId,
          name: "Ops",
          visibility: "private",
        });
      }
      if (method === "POST" && url.includes(`/guilds/${createdGuildId}/channels`)) {
        firstChannelBody = JSON.parse(String(init?.body)) as { name: string; kind: string };
        return jsonResponse({
          channel_id: CREATED_CHANNEL_ID,
          name: firstChannelBody.name,
          kind: firstChannelBody.kind,
        });
      }
      if (
        method === "GET" &&
        url.includes(`/guilds/${createdGuildId}/channels/${CREATED_CHANNEL_ID}/permissions/self`)
      ) {
        return jsonResponse({ role: "owner", permissions: ["create_message"] });
      }
      if (
        method === "GET" &&
        url.includes(`/guilds/${createdGuildId}/channels/${CREATED_CHANNEL_ID}/messages`)
      ) {
        return jsonResponse({ messages: [], next_before: null });
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

    await screen.findByRole("heading", { name: "Create your first workspace" });
    await fireEvent.change(screen.getByLabelText("Channel type"), { target: { value: "voice" } });
    await fireEvent.click(screen.getByRole("button", { name: "Create workspace" }));

    await waitFor(() =>
      expect(firstChannelBody).toEqual({ name: "incident-room", kind: "voice" }),
    );
  });
});
