import { fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { vi } from "vitest";
import { App } from "../src/App";

const SESSION_STORAGE_KEY = "filament.auth.session.v1";
const WORKSPACE_CACHE_KEY = "filament.workspace.cache.v1";
const VOICE_SETTINGS_KEY = "filament.voice.settings.v1";

const ACCESS_TOKEN = "A".repeat(64);
const REFRESH_TOKEN = "B".repeat(64);
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const OWNER_ROLE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FB1";
const MEMBER_ROLE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FB2";

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

function seedAuthenticatedWorkspace(input?: {
  audioInputDeviceId?: string | null;
  audioOutputDeviceId?: string | null;
}): void {
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
        visibility: "private",
        channels: [{ channelId: CHANNEL_ID, name: "incident-room", kind: "text" }],
      },
    ]),
  );

  if (input) {
    window.localStorage.setItem(
      VOICE_SETTINGS_KEY,
      JSON.stringify({
        audioInputDeviceId: input.audioInputDeviceId ?? null,
        audioOutputDeviceId: input.audioOutputDeviceId ?? null,
      }),
    );
  }
}

function stubMediaDevices(
  devices: Array<{ kind: string; deviceId: string; label: string }>,
): { enumerateDevices: ReturnType<typeof vi.fn> } {
  const enumerateDevices = vi.fn(async () => devices);
  Object.defineProperty(navigator, "mediaDevices", {
    configurable: true,
    value: {
      enumerateDevices,
    },
  });
  return { enumerateDevices };
}

function stubMediaDevicesWithPermissionFlow(input: {
  enumerateDevices: () => Promise<Array<{ kind: string; deviceId: string; label: string }>>;
  getUserMedia: () => Promise<{ getTracks(): Array<{ stop(): void }> }>;
}): {
  enumerateDevices: ReturnType<typeof vi.fn>;
  getUserMedia: ReturnType<typeof vi.fn>;
} {
  const enumerateDevices = vi.fn(input.enumerateDevices);
  const getUserMedia = vi.fn(input.getUserMedia);
  Object.defineProperty(navigator, "mediaDevices", {
    configurable: true,
    value: {
      enumerateDevices,
      getUserMedia,
    },
  });
  return {
    enumerateDevices,
    getUserMedia,
  };
}

function createSettingsFixtureFetch(options?: {
  profileDelayMs?: number;
  profileError?: { status: number; code: string };
  channelPermissions?: string[];
}) {
  return vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = requestUrl(input);
    const method = requestMethod(init);

    if (method === "GET" && url.includes("/auth/me")) {
      return jsonResponse({ user_id: USER_ID, username: "owner" });
    }
    if (method === "GET" && url.includes(`/users/${USER_ID}/profile`)) {
      if (options?.profileDelayMs && options.profileDelayMs > 0) {
        await new Promise((resolve) => {
          setTimeout(resolve, options.profileDelayMs);
        });
      }
      if (options?.profileError) {
        return jsonResponse({ error: options.profileError.code }, options.profileError.status);
      }
      return jsonResponse({
        user_id: USER_ID,
        username: "owner",
        about_markdown: "hello **world**",
        about_markdown_tokens: [
          { type: "paragraph_start" },
          { type: "text", text: "hello " },
          { type: "strong_start" },
          { type: "text", text: "world" },
          { type: "strong_end" },
          { type: "paragraph_end" },
        ],
        avatar_version: 1,
        banner_version: 0,
      });
    }
    if (method === "GET" && url.endsWith("/guilds")) {
      return jsonResponse({
        guilds: [{ guild_id: GUILD_ID, name: "Security Ops", visibility: "private" }],
      });
    }
    if (method === "PATCH" && url.endsWith(`/guilds/${GUILD_ID}`)) {
      const body = init?.body ? JSON.parse(String(init.body)) : {};
      return jsonResponse({
        guild_id: GUILD_ID,
        name: typeof body.name === "string" ? body.name : "Security Ops",
        visibility: body.visibility === "public" ? "public" : "private",
      });
    }
    if (method === "GET" && url.endsWith(`/guilds/${GUILD_ID}/channels`)) {
      return jsonResponse({
        channels: [{ channel_id: CHANNEL_ID, name: "incident-room", kind: "text" }],
      });
    }
    if (method === "GET" && url.endsWith(`/guilds/${GUILD_ID}/roles`)) {
      return jsonResponse({
        roles: [
          {
            role_id: OWNER_ROLE_ID,
            name: "owner",
            position: 0,
            is_system: true,
            permissions: [
              "manage_roles",
              "manage_workspace_roles",
              "manage_member_roles",
              "create_message",
              "publish_video",
              "publish_screen_share",
              "subscribe_streams",
            ],
            color_hex: null,
          },
          {
            role_id: MEMBER_ROLE_ID,
            name: "member",
            position: 1,
            is_system: true,
            permissions: ["create_message", "subscribe_streams"],
            color_hex: null,
          },
        ],
        default_join_role_id: MEMBER_ROLE_ID,
      });
    }
    if (
      method === "GET" &&
      url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/permissions/self`)
    ) {
      const permissions = options?.channelPermissions ?? ["create_message", "manage_roles"];
      return jsonResponse({ role: permissions.includes("manage_roles") ? "owner" : "member", permissions });
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

async function openWorkspaceSettingsPanelFromWorkspaceMenu(): Promise<void> {
  const menuButton = await screen.findByRole("button", { name: "Open workspace menu" });
  await waitFor(() => expect(menuButton).not.toBeDisabled());
  fireEvent.click(menuButton);
  fireEvent.click(await screen.findByRole("menuitem", { name: "Open workspace settings panel" }));
}

async function openClientSettingsPanelFromAccountGear(): Promise<void> {
  fireEvent.click(await screen.findByRole("button", { name: "Open client settings panel" }));
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

  it("opens and closes client settings panel from account gear", async () => {
    seedAuthenticatedWorkspace();
    vi.stubGlobal("fetch", createSettingsFixtureFetch());
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    const media = stubMediaDevices([
      { kind: "audioinput", deviceId: "mic-1", label: "Desk Mic" },
      { kind: "audiooutput", deviceId: "spk-1", label: "Desk Speaker" },
    ]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await openClientSettingsPanelFromAccountGear();
    expect(await screen.findByRole("dialog", { name: "Client settings panel" })).toBeInTheDocument();
    expect(await screen.findByLabelText("Settings category rail")).toBeInTheDocument();
    expect(await screen.findByLabelText("Settings content pane")).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Open Voice settings category" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(await screen.findByRole("button", { name: "Open Profile settings category" })).toBeInTheDocument();
    expect(await screen.findByLabelText("Voice settings submenu")).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Open Voice Audio Devices submenu" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    expect(await screen.findByLabelText("Select microphone device")).toBeInTheDocument();
    expect(await screen.findByLabelText("Select speaker device")).toBeInTheDocument();
    await waitFor(() => expect(media.enumerateDevices).toHaveBeenCalledTimes(1));

    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Client settings panel" })).not.toBeInTheDocument(),
    );
  });

  it("opens workspace settings panel from workspace menu", async () => {
    seedAuthenticatedWorkspace();
    vi.stubGlobal("fetch", createSettingsFixtureFetch());
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    stubMediaDevices([]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await openWorkspaceSettingsPanelFromWorkspaceMenu();
    expect(await screen.findByRole("dialog", { name: "Workspace settings panel" })).toBeInTheDocument();
    expect(await screen.findByLabelText("Workspace settings name")).toBeInTheDocument();
    expect(await screen.findByLabelText("Workspace settings visibility")).toBeInTheDocument();
  });

  it("closes client settings panel on Escape", async () => {
    seedAuthenticatedWorkspace();
    vi.stubGlobal("fetch", createSettingsFixtureFetch());
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    stubMediaDevices([]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await openClientSettingsPanelFromAccountGear();
    expect(await screen.findByRole("dialog", { name: "Client settings panel" })).toBeInTheDocument();

    fireEvent.keyDown(window, { key: "Escape" });
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "Client settings panel" })).not.toBeInTheDocument(),
    );
  });

  it("switches between Voice and Profile categories", async () => {
    seedAuthenticatedWorkspace();
    vi.stubGlobal("fetch", createSettingsFixtureFetch());
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    stubMediaDevices([]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await openClientSettingsPanelFromAccountGear();
    expect(await screen.findByRole("dialog", { name: "Client settings panel" })).toBeInTheDocument();

    fireEvent.click(await screen.findByRole("button", { name: "Open Profile settings category" }));
    expect(screen.getByLabelText("Profile username")).toBeInTheDocument();
    expect(screen.getByLabelText("Profile about markdown")).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Save profile" })).toBeInTheDocument();
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
    expect(screen.getByLabelText("Select microphone device")).toBeInTheDocument();
  });

  it("persists microphone and speaker selections from Audio Devices", async () => {
    seedAuthenticatedWorkspace({
      audioInputDeviceId: "mic-1",
      audioOutputDeviceId: "spk-1",
    });
    vi.stubGlobal("fetch", createSettingsFixtureFetch());
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    stubMediaDevices([
      { kind: "audioinput", deviceId: "mic-1", label: "Desk Mic" },
      { kind: "audioinput", deviceId: "mic-2", label: "USB Mic" },
      { kind: "audiooutput", deviceId: "spk-1", label: "Desk Speaker" },
      { kind: "audiooutput", deviceId: "spk-2", label: "Headphones" },
    ]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await openClientSettingsPanelFromAccountGear();
    expect(await screen.findByRole("dialog", { name: "Client settings panel" })).toBeInTheDocument();

    const microphoneSelect = await screen.findByLabelText("Select microphone device");
    const speakerSelect = await screen.findByLabelText("Select speaker device");
    await screen.findByText("USB Mic");
    await screen.findByText("Headphones");

    fireEvent.change(microphoneSelect, {
      target: { value: "mic-2" },
    });
    fireEvent.change(speakerSelect, {
      target: { value: "spk-2" },
    });

    await waitFor(() => {
      const raw = window.localStorage.getItem(VOICE_SETTINGS_KEY);
      expect(raw).not.toBeNull();
      expect(JSON.parse(raw!)).toEqual({
        audioInputDeviceId: "mic-2",
        audioOutputDeviceId: "spk-2",
      });
    });
  });

  it("requests microphone permission on Refresh devices when inventory is empty", async () => {
    seedAuthenticatedWorkspace();
    vi.stubGlobal("fetch", createSettingsFixtureFetch());
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    const stopTrack = vi.fn();
    const mediaDevices = stubMediaDevicesWithPermissionFlow({
      enumerateDevices: async () =>
        mediaDevices.getUserMedia.mock.calls.length > 0
          ? [{ kind: "audioinput", deviceId: "mic-1", label: "Desk Mic" }]
          : [],
      getUserMedia: async () => ({
        getTracks: () => [{ stop: stopTrack }],
      }),
    });

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await openClientSettingsPanelFromAccountGear();
    expect(await screen.findByRole("dialog", { name: "Client settings panel" })).toBeInTheDocument();

    const refreshButton = await screen.findByRole("button", { name: "Refresh devices" });
    fireEvent.click(refreshButton);

    await waitFor(() => {
      expect(mediaDevices.getUserMedia).toHaveBeenCalledTimes(1);
      expect(stopTrack).toHaveBeenCalledTimes(1);
    });
    expect(await screen.findByText("Desk Mic")).toBeInTheDocument();
  });

  it("saves workspace name and visibility from workspace settings panel", async () => {
    seedAuthenticatedWorkspace();
    const fetchMock = createSettingsFixtureFetch();
    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    stubMediaDevices([]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await openWorkspaceSettingsPanelFromWorkspaceMenu();
    expect(await screen.findByRole("dialog", { name: "Workspace settings panel" })).toBeInTheDocument();

    fireEvent.input(screen.getByLabelText("Workspace settings name"), {
      target: { value: "Blue Team" },
    });
    fireEvent.change(screen.getByLabelText("Workspace settings visibility"), {
      target: { value: "public" },
    });
    const saveButton = screen.getByRole("button", { name: "Save workspace" });
    await waitFor(() => expect(saveButton).not.toBeDisabled());
    fireEvent.click(saveButton);

    expect(await screen.findByText("Workspace settings saved.")).toBeInTheDocument();
    expect(fetchMock).toHaveBeenCalledWith(
      expect.stringContaining(`/guilds/${GUILD_ID}`),
      expect.objectContaining({
        method: "PATCH",
      }),
    );
  });

  it("keeps workspace settings disabled for users without workspace role-management permission", async () => {
    seedAuthenticatedWorkspace();
    vi.stubGlobal(
      "fetch",
      createSettingsFixtureFetch({ channelPermissions: ["create_message"] }),
    );
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    stubMediaDevices([]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await openWorkspaceSettingsPanelFromWorkspaceMenu();
    expect(await screen.findByRole("dialog", { name: "Workspace settings panel" })).toBeInTheDocument();
    expect(await screen.findByRole("button", { name: "Save workspace" })).toBeDisabled();
    expect(
      await screen.findByText("You need workspace role-management permissions to update these settings."),
    ).toBeInTheDocument();
  });

  it("opens profile panel when clicking avatar controls, renders loading, and closes", async () => {
    seedAuthenticatedWorkspace();
    vi.stubGlobal("fetch", createSettingsFixtureFetch({ profileDelayMs: 80 }));
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    stubMediaDevices([]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Open owner profile" }));
    const dialog = await screen.findByRole("dialog", { name: "User profile panel" });
    expect(dialog).toBeInTheDocument();
    expect(await within(dialog).findByText("Loading profile...")).toBeInTheDocument();
    expect(await within(dialog).findByText("owner")).toBeInTheDocument();

    fireEvent.click(within(dialog).getByRole("button", { name: "Close" }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "User profile panel" })).not.toBeInTheDocument(),
    );
  });

  it("renders profile error state when profile lookup fails", async () => {
    seedAuthenticatedWorkspace();
    vi.stubGlobal(
      "fetch",
      createSettingsFixtureFetch({
        profileError: {
          status: 404,
          code: "not_found",
        },
      }),
    );
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);
    stubMediaDevices([]);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    fireEvent.click(await screen.findByRole("button", { name: "Open owner profile" }));
    const dialog = await screen.findByRole("dialog", { name: "User profile panel" });
    expect(dialog).toBeInTheDocument();
    expect(await within(dialog).findByText("Requested resource was not found.")).toBeInTheDocument();
  });
});
