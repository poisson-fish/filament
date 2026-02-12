import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";
import { attachmentIdFromInput, channelIdFromInput, guildIdFromInput } from "../src/domain/chat";
import { downloadChannelAttachmentPreview, fetchHealth } from "../src/lib/api";

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
