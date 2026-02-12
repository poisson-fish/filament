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
const ATTACHMENT_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const MESSAGE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";
const ORIGINAL_CREATE_OBJECT_URL = URL.createObjectURL;
const ORIGINAL_REVOKE_OBJECT_URL = URL.revokeObjectURL;

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

describe("app shell composer attachments", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
    const storage = createStorageMock();
    vi.stubGlobal("localStorage", storage);
    Object.defineProperty(window, "localStorage", {
      value: storage,
      configurable: true,
    });
    Object.defineProperty(URL, "createObjectURL", {
      value: vi.fn(() => "blob:preview"),
      configurable: true,
    });
    Object.defineProperty(URL, "revokeObjectURL", {
      value: vi.fn(),
      configurable: true,
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    if (ORIGINAL_CREATE_OBJECT_URL) {
      Object.defineProperty(URL, "createObjectURL", {
        value: ORIGINAL_CREATE_OBJECT_URL,
        configurable: true,
      });
    } else {
      Reflect.deleteProperty(URL, "createObjectURL");
    }
    if (ORIGINAL_REVOKE_OBJECT_URL) {
      Object.defineProperty(URL, "revokeObjectURL", {
        value: ORIGINAL_REVOKE_OBJECT_URL,
        configurable: true,
      });
    } else {
      Reflect.deleteProperty(URL, "revokeObjectURL");
    }
  });

  it("uploads staged files from the + button before posting the message", async () => {
    seedAuthenticatedWorkspace();

    let postedContent = "";
    let postedAttachmentIds: string[] = [];

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
        return jsonResponse({ messages: [], next_before: null });
      }
      if (method === "GET" && url.includes("/guilds/public")) {
        return jsonResponse({ guilds: [] });
      }
      if (
        method === "POST" &&
        url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/attachments?`)
      ) {
        return jsonResponse({
          attachment_id: ATTACHMENT_ID,
          guild_id: GUILD_ID,
          channel_id: CHANNEL_ID,
          owner_id: USER_ID,
          filename: "one.gif",
          mime_type: "image/gif",
          size_bytes: 43,
          sha256_hex: "a".repeat(64),
        });
      }
      if (method === "POST" && url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/messages`)) {
        const body = typeof init?.body === "string" ? JSON.parse(init.body) : { content: "" };
        postedContent = body.content as string;
        postedAttachmentIds = Array.isArray(body.attachment_ids) ? body.attachment_ids : [];
        return jsonResponse({
          message_id: MESSAGE_ID,
          guild_id: GUILD_ID,
          channel_id: CHANNEL_ID,
          author_id: USER_ID,
          content: postedContent,
          markdown_tokens: [{ type: "paragraph_start" }, { type: "paragraph_end" }],
          attachments: [
            {
              attachment_id: ATTACHMENT_ID,
              guild_id: GUILD_ID,
              channel_id: CHANNEL_ID,
              owner_id: USER_ID,
              filename: "one.gif",
              mime_type: "image/gif",
              size_bytes: 43,
              sha256_hex: "a".repeat(64),
            },
          ],
          created_at_unix: 1,
        });
      }
      if (
        method === "GET" &&
        url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/attachments/${ATTACHMENT_ID}`)
      ) {
        return new Response(new Uint8Array([71, 73, 70, 56, 57, 97]), {
          status: 200,
          headers: { "content-type": "image/gif" },
        });
      }

      return jsonResponse({ error: "not_found" }, 404);
    });

    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await screen.findByRole("heading", { name: "Workspace Tools" });

    const composerInput = await screen.findByPlaceholderText("Message #incident-room");
    const fileInput = document.querySelector(".composer-file-input") as HTMLInputElement | null;
    expect(fileInput).not.toBeNull();

    const file = new File(["GIF89a"], "one.gif", { type: "image/gif" });
    await fireEvent.input(fileInput!, { target: { files: [file] } });

    await fireEvent.input(composerInput, { target: { value: "see attached" } });
    const composerForm = document.querySelector("form.composer") as HTMLFormElement | null;
    expect(composerForm).not.toBeNull();
    await fireEvent.submit(composerForm!);

    await waitFor(() => expect(postedAttachmentIds).toEqual([ATTACHMENT_ID]));
    expect(postedAttachmentIds).toEqual([ATTACHMENT_ID]);
    expect(postedContent).toBe("");
    await waitFor(() => {
      const hasImage = Boolean(screen.queryByAltText("one.gif"));
      const hasLoading = Boolean(screen.queryByText("Loading preview..."));
      const hasDownload = Boolean(screen.queryByText("Download one.gif"));
      expect(hasImage || hasLoading || hasDownload).toBe(true);
    });
  });

  it("renders an image preview when attachment mime is generic after reload", async () => {
    seedAuthenticatedWorkspace();

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
              content: "",
              markdown_tokens: [{ type: "paragraph_start" }, { type: "paragraph_end" }],
              attachments: [
                {
                  attachment_id: ATTACHMENT_ID,
                  guild_id: GUILD_ID,
                  channel_id: CHANNEL_ID,
                  owner_id: USER_ID,
                  filename: "camera-roll.jpg",
                  mime_type: "application/octet-stream",
                  size_bytes: 43,
                  sha256_hex: "a".repeat(64),
                },
              ],
              created_at_unix: 1,
            },
          ],
          next_before: null,
        });
      }
      if (method === "GET" && url.includes("/guilds/public")) {
        return jsonResponse({ guilds: [] });
      }
      if (
        method === "GET" &&
        url.includes(`/guilds/${GUILD_ID}/channels/${CHANNEL_ID}/attachments/${ATTACHMENT_ID}`)
      ) {
        return new Response(new Uint8Array([255, 216, 255, 224]), {
          status: 200,
          headers: { "content-type": "application/octet-stream" },
        });
      }

      return jsonResponse({ error: "not_found" }, 404);
    });

    vi.stubGlobal("fetch", fetchMock);
    vi.stubGlobal("WebSocket", undefined as unknown as typeof WebSocket);

    window.history.replaceState({}, "", "/app");
    render(() => <App />);

    await screen.findByRole("heading", { name: "Workspace Tools" });
    await screen.findByPlaceholderText("Message #incident-room");
    await screen.findByAltText("camera-roll.jpg");
    await waitFor(() => expect(screen.queryByText("Loading preview...")).toBeNull());
  });
});
