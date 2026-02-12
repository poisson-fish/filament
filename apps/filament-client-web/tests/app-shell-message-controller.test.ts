import { describe, expect, it } from "vitest";
import { messageFromResponse, messageIdFromInput } from "../src/domain/chat";
import {
  clearReactionRecordsForMessage,
  collectMediaPreviewTargets,
  mediaPreviewRetryDelayMs,
  mergeComposerAttachmentSelection,
  nextMediaPreviewAttempt,
  retainRecordByAllowedIds,
  shouldRetryMediaPreview,
} from "../src/features/app-shell/controllers/message-controller";

const GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";

function messageWithAttachments(attachments: Array<{
  attachment_id: string;
  filename: string;
  mime_type: string;
  size_bytes: number;
}>): ReturnType<typeof messageFromResponse> {
  return messageFromResponse({
    message_id: "01ARZ3NDEKTSV4RRFFQ69G5FAY",
    guild_id: GUILD_ID,
    channel_id: CHANNEL_ID,
    author_id: USER_ID,
    content: "attachment test",
    markdown_tokens: [{ type: "text", text: "attachment test" }],
    attachments: attachments.map((attachment) => ({
      ...attachment,
      guild_id: GUILD_ID,
      channel_id: CHANNEL_ID,
      owner_id: USER_ID,
      sha256_hex: "a".repeat(64),
    })),
    created_at_unix: 1,
  });
}

describe("app shell message controller", () => {
  it("collects only previewable attachments under byte caps", () => {
    const message = messageWithAttachments([
      {
        attachment_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
        filename: "screen.png",
        mime_type: "image/png",
        size_bytes: 120,
      },
      {
        attachment_id: "01ARZ3NDEKTSV4RRFFQ69G5FB1",
        filename: "clip.mp4",
        mime_type: "video/mp4",
        size_bytes: 900,
      },
      {
        attachment_id: "01ARZ3NDEKTSV4RRFFQ69G5FB2",
        filename: "manual.pdf",
        mime_type: "application/pdf",
        size_bytes: 80,
      },
      {
        attachment_id: "01ARZ3NDEKTSV4RRFFQ69G5FB3",
        filename: "oversized.jpg",
        mime_type: "image/jpeg",
        size_bytes: 5000,
      },
    ]);

    const targets = collectMediaPreviewTargets([message], 1000);

    expect([...targets.keys()]).toEqual([
      "01ARZ3NDEKTSV4RRFFQ69G5FB0",
      "01ARZ3NDEKTSV4RRFFQ69G5FB1",
    ]);
  });

  it("tracks retry attempts and stops after max retries", () => {
    const attempts = new Map<string, number>();

    const firstAttempt = nextMediaPreviewAttempt(attempts, "att");
    attempts.set("att", firstAttempt);
    const secondAttempt = nextMediaPreviewAttempt(attempts, "att");
    attempts.set("att", secondAttempt);
    const thirdAttempt = nextMediaPreviewAttempt(attempts, "att");

    expect(firstAttempt).toBe(1);
    expect(secondAttempt).toBe(2);
    expect(thirdAttempt).toBe(3);
    expect(shouldRetryMediaPreview(secondAttempt, 2)).toBe(true);
    expect(shouldRetryMediaPreview(thirdAttempt, 2)).toBe(false);
    expect(mediaPreviewRetryDelayMs(1)).toBe(600);
    expect(mediaPreviewRetryDelayMs(2)).toBe(1200);
  });

  it("retains record entries only for allowed IDs", () => {
    const retained = retainRecordByAllowedIds(
      {
        alpha: true,
        beta: true,
      },
      new Set(["beta"]),
    );

    expect(retained).toEqual({ beta: true });
  });

  it("deduplicates and caps composer attachment selection", () => {
    const fileA = new File(["a"], "one.txt", {
      type: "text/plain",
      lastModified: 1,
    });
    const fileADupe = new File(["a"], "one.txt", {
      type: "text/plain",
      lastModified: 1,
    });
    const fileB = new File(["b"], "two.txt", {
      type: "text/plain",
      lastModified: 2,
    });
    const fileC = new File(["c"], "three.txt", {
      type: "text/plain",
      lastModified: 3,
    });

    const merged = mergeComposerAttachmentSelection([fileA], [fileADupe, fileB, fileC], 2);

    expect(merged.files).toEqual([fileA, fileB]);
    expect(merged.reachedCap).toBe(true);
  });

  it("clears reaction records for one message prefix only", () => {
    const firstMessageId = messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB4");
    const secondMessageId = messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB5");
    const cleared = clearReactionRecordsForMessage(
      {
        [`${firstMessageId}|👍`]: { count: 1, reacted: true },
        [`${firstMessageId}|🔥`]: { count: 2, reacted: false },
        [`${secondMessageId}|👍`]: { count: 3, reacted: true },
      },
      firstMessageId,
    );

    expect(cleared).toEqual({
      [`${secondMessageId}|👍`]: { count: 3, reacted: true },
    });
  });
});
