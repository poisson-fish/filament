import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";
import { attachmentIdFromInput, channelIdFromInput, guildIdFromInput } from "../src/domain/chat";
import {
  downloadChannelAttachmentPreview,
  fetchGuildRoles,
  fetchHealth,
  joinPublicGuild,
} from "../src/lib/api";

function createProbeStream(chunks: Uint8Array[]): {
  stream: ReadableStream<Uint8Array>;
  emittedChunks: () => number;
  cancelCalls: () => number;
} {
  let chunkIndex = 0;
  let emitted = 0;
  let cancels = 0;
  const stream = new ReadableStream<Uint8Array>({
    pull(controller) {
      if (chunkIndex >= chunks.length) {
        controller.close();
        return;
      }
      controller.enqueue(chunks[chunkIndex]);
      chunkIndex += 1;
      emitted += 1;
    },
    cancel() {
      cancels += 1;
    },
  });
  return {
    stream,
    emittedChunks: () => emitted,
    cancelCalls: () => cancels,
  };
}

describe("api boundary hardening", () => {
  const session = {
    accessToken: accessTokenFromInput("A".repeat(64)),
    refreshToken: refreshTokenFromInput("B".repeat(64)),
    expiresAtUnix: 2_000_000_000,
  };
  const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
  const channelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB0");
  const attachmentId = attachmentIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB1");

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    vi.unstubAllEnvs();
    vi.useRealTimers();
  });

  it("rejects oversized json responses before consuming full payload", async () => {
    const chunk = new TextEncoder().encode("A".repeat(4096));
    const chunks = Array.from({ length: 40 }, () => chunk);
    const probe = createProbeStream(chunks);
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(probe.stream, {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
      ),
    );

    await expect(fetchHealth()).rejects.toMatchObject({ status: 200, code: "oversized_response" });
    expect(probe.cancelCalls()).toBeGreaterThan(0);
    expect(probe.emittedChunks()).toBeLessThan(chunks.length);
  });

  it("fast-fails oversized json responses from content-length before reading", async () => {
    const chunk = new TextEncoder().encode("A".repeat(2048));
    const probe = createProbeStream([chunk, chunk, chunk]);
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(probe.stream, {
          status: 200,
          headers: {
            "content-type": "application/json",
            "content-length": String(70 * 1024),
          },
        }),
      ),
    );

    await expect(fetchHealth()).rejects.toMatchObject({ status: 200, code: "oversized_response" });
    expect(probe.emittedChunks()).toBeLessThan(2);
    expect(probe.cancelCalls()).toBeGreaterThan(0);
  });

  it("ignores malformed content-length and still enforces streaming response size caps", async () => {
    const chunk = new TextEncoder().encode("A".repeat(4096));
    const chunks = Array.from({ length: 40 }, () => chunk);
    const probe = createProbeStream(chunks);
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(probe.stream, {
          status: 200,
          headers: {
            "content-type": "application/json",
            "content-length": "not-a-number",
          },
        }),
      ),
    );

    await expect(fetchHealth()).rejects.toMatchObject({ status: 200, code: "oversized_response" });
    expect(probe.cancelCalls()).toBeGreaterThan(0);
    expect(probe.emittedChunks()).toBeLessThan(chunks.length);
  });

  it("rejects oversized binary responses before consuming full payload", async () => {
    const chunk = new Uint8Array(256 * 1024).fill(0x5a);
    const chunks = Array.from({ length: 64 }, () => chunk);
    const probe = createProbeStream(chunks);
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(probe.stream, {
          status: 200,
          headers: { "content-type": "application/octet-stream" },
        }),
      ),
    );

    await expect(
      downloadChannelAttachmentPreview(session, guildId, channelId, attachmentId),
    ).rejects.toMatchObject({
      status: 200,
      code: "oversized_response",
    });
    expect(probe.cancelCalls()).toBeGreaterThan(0);
    expect(probe.emittedChunks()).toBeLessThan(chunks.length);
  });

  it("keeps malformed json mapping parity", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response("{not_json", {
          status: 200,
          headers: { "content-type": "application/json" },
        }),
      ),
    );

    await expect(fetchHealth()).rejects.toMatchObject({ status: 200, code: "invalid_json" });
  });

  it("maps non-ok responses with error code deterministically", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ error: "rate_limited" }), {
          status: 429,
          headers: { "content-type": "application/json" },
        }),
      ),
    );

    await expect(fetchHealth()).rejects.toMatchObject({ status: 429, code: "rate_limited" });
  });

  it("maps non-json rate-limit responses by status fallback", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response("Too Many Requests! Wait for 1s", {
          status: 429,
          headers: { "content-type": "text/plain" },
        }),
      ),
    );

    await expect(fetchHealth()).rejects.toMatchObject({ status: 429, code: "rate_limited" });
  });

  it("maps non-json timeout responses by status fallback", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response("Request timed out", {
          status: 408,
          headers: { "content-type": "text/plain" },
        }),
      ),
    );

    await expect(fetchHealth()).rejects.toMatchObject({ status: 408, code: "request_timeout" });
  });

  it("maps non-ok responses without string error to unexpected_error", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ error: { code: "nested" } }), {
          status: 403,
          headers: { "content-type": "application/json" },
        }),
      ),
    );

    await expect(fetchHealth()).rejects.toMatchObject({ status: 403, code: "unexpected_error" });
  });

  it("preserves directory-join policy error codes for deterministic UI mapping", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(JSON.stringify({ error: "directory_join_ip_banned" }), {
          status: 403,
          headers: { "content-type": "application/json" },
        }),
      ),
    );

    await expect(joinPublicGuild(session, guildId)).rejects.toMatchObject({
      status: 403,
      code: "directory_join_ip_banned",
    });
  });

  it("maps successful directory join responses through strict DTO parsing", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            guild_id: guildId,
            outcome: "already_member",
          }),
          {
            status: 200,
            headers: { "content-type": "application/json" },
          },
        ),
      ),
    );

    await expect(joinPublicGuild(session, guildId)).resolves.toMatchObject({
      guildId,
      outcome: "already_member",
      joined: true,
    });
  });

  it("maps guild role list responses through strict DTO parsing", async () => {
    vi.stubGlobal(
      "fetch",
      vi.fn().mockResolvedValue(
        new Response(
          JSON.stringify({
            roles: [
              {
                role_id: "01ARZ3NDEKTSV4RRFFQ69G5FB2",
                name: "Responder",
                position: 3,
                is_system: false,
                permissions: ["create_message", "subscribe_streams"],
              },
            ],
          }),
          {
            status: 200,
            headers: { "content-type": "application/json" },
          },
        ),
      ),
    );

    await expect(fetchGuildRoles(session, guildId)).resolves.toMatchObject({
      roles: [
        {
          roleId: "01ARZ3NDEKTSV4RRFFQ69G5FB2",
          name: "Responder",
        },
      ],
    });
  });

  it("maps timeout aborts to network_error", async () => {
    vi.useFakeTimers();
    vi.stubGlobal(
      "fetch",
      vi.fn((_input: RequestInfo | URL, init?: RequestInit) => {
        const signal = init?.signal as AbortSignal | undefined;
        return new Promise<Response>((_resolve, reject) => {
          if (!signal || typeof signal.addEventListener !== "function") {
            reject(new Error("missing_abort_signal"));
            return;
          }
          signal.addEventListener(
            "abort",
            () => {
              reject(new DOMException("aborted", "AbortError"));
            },
            { once: true },
          );
        });
      }),
    );

    const request = fetchHealth();
    const assertion = expect(request).rejects.toMatchObject({ status: 0, code: "network_error" });
    await vi.advanceTimersByTimeAsync(7_100);
    await assertion;
  });
});
