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

    if (
      method === "GET" &&
      url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/messages`)
    ) {
      return jsonResponse({
        messages: [],
        next_before: null,
      });
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

function hasRequest(
  fetchMock: ReturnType<typeof createOperatorFixtureFetch>,
  method: string,
  pathFragment: string,
): boolean {
  return fetchMock.mock.calls.some(([input, init]) => {
    return (
      requestMethod(init) === method.toUpperCase() &&
      requestUrl(input).includes(pathFragment)
    );
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
    const fetchMock = createOperatorFixtureFetch("owner");
    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    expect(await screen.findByRole("heading", { name: "Ops Console" })).toBeInTheDocument();

    await fireEvent.input(screen.getByLabelText("Target user ULID"), {
      target: { value: TARGET_USER_ID },
    });
    await fireEvent.change(screen.getByLabelText("Role"), {
      target: { value: "moderator" },
    });

    await fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(await screen.findByText("Member add request accepted.")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Set Role" }));
    expect(await screen.findByText("Member role updated to moderator.")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Apply channel override" }));
    expect(await screen.findByText("Channel role override updated.")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Rebuild Index" }));
    expect(await screen.findByText("Search index rebuild queued.")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Reconcile Index" }));
    expect(
      await screen.findByText(
        "Reconciled search index (upserted 2, deleted 1).",
      ),
    ).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Issue token" }));
    expect(
      await screen.findByText(
        "Voice token issued (300s, publish=true, subscribe=false).",
      ),
    ).toBeInTheDocument();

    expect(
      hasRequest(fetchMock, "POST", `/guilds/${GUILD_ID}/members/${TARGET_USER_ID}`),
    ).toBe(true);
    expect(
      hasRequest(fetchMock, "PATCH", `/guilds/${GUILD_ID}/members/${TARGET_USER_ID}`),
    ).toBe(true);
    expect(
      hasRequest(
        fetchMock,
        "POST",
        `/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/overrides/member`,
      ),
    ).toBe(true);
    expect(hasRequest(fetchMock, "POST", `/guilds/${GUILD_ID}/search/rebuild`)).toBe(
      true,
    );
    expect(hasRequest(fetchMock, "POST", `/guilds/${GUILD_ID}/search/reconcile`)).toBe(
      true,
    );
    expect(
      hasRequest(
        fetchMock,
        "POST",
        `/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/voice/token`,
      ),
    ).toBe(true);
  });

  it("surfaces permission-denied UX for restricted member fixtures", async () => {
    seedAuthenticatedWorkspace();
    const fetchMock = createOperatorFixtureFetch("member");
    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    expect(await screen.findByRole("heading", { name: "Ops Console" })).toBeInTheDocument();

    await fireEvent.input(screen.getByLabelText("Target user ULID"), {
      target: { value: TARGET_USER_ID },
    });
    await fireEvent.click(screen.getByRole("button", { name: "Add" }));
    expect(await screen.findByText("Permission denied for this action.")).toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Rebuild Index" }));
    await waitFor(() =>
      expect(hasRequest(fetchMock, "POST", `/guilds/${GUILD_ID}/search/rebuild`)).toBe(
        true,
      ),
    );

    await fireEvent.click(screen.getByRole("button", { name: "Apply channel override" }));
    await waitFor(() =>
      expect(
        hasRequest(
          fetchMock,
          "POST",
          `/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/overrides/member`,
        ),
      ).toBe(true),
    );

    await fireEvent.click(screen.getByRole("button", { name: "Issue token" }));
    await waitFor(() =>
      expect(
        hasRequest(
          fetchMock,
          "POST",
          `/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/voice/token`,
        ),
      ).toBe(true),
    );
    await waitFor(() =>
      expect(screen.getByRole("button", { name: "Issue token" })).toBeEnabled(),
    );
    expect(
      screen.queryByText("Voice token issued (300s, publish=true, subscribe=false)."),
    ).not.toBeInTheDocument();
  });
});
