import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  channelFromResponse,
  channelIdFromInput,
  guildIdFromInput,
  messageFromResponse,
  messageIdFromInput,
} from "../src/domain/chat";
import {
  clearReactionRecordsForMessage,
  collectMediaPreviewTargets,
  createMessageActionsController,
  mediaPreviewRetryDelayMs,
  mergeComposerAttachmentSelection,
  nextMediaPreviewAttempt,
  retainRecordByAllowedIds,
  shouldRetryMediaPreview,
} from "../src/features/app-shell/controllers/message-controller";
import type { AsyncOperationState } from "../src/features/app-shell/state/async-operation-state";

const GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

function createMessageActionsHarness(input?: {
  activeGuildId?: ReturnType<typeof guildIdFromInput> | null;
  activeChannelId?: ReturnType<typeof channelIdFromInput> | null;
  composer?: string;
  attachments?: File[];
}) {
  const [session] = createSignal(SESSION);
  const [activeGuildId] = createSignal(input?.activeGuildId ?? null);
  const [activeChannelId] = createSignal(input?.activeChannelId ?? null);
  const [activeChannel] = createSignal(
    channelFromResponse({
      channel_id: CHANNEL_ID,
      name: "incident-room",
      kind: "text",
    }),
  );
  const [composer] = createSignal(input?.composer ?? "");
  const [, setComposer] = createSignal("");
  const [composerAttachments] = createSignal(input?.attachments ?? []);
  const [, setComposerAttachments] = createSignal<File[]>([]);
  const [messageStatus, setMessageStatus] = createSignal("");
  const [messageError, setMessageError] = createSignal("");
  const [isSendingMessage, setSendingMessage] = createSignal(false);
  const [sendMessageState, setSendMessageState] = createSignal<AsyncOperationState>({
    phase: "idle",
    statusMessage: "",
    errorMessage: "",
  });

  const controller = createMessageActionsController({
    session,
    activeGuildId,
    activeChannelId,
    activeChannel,
    canAccessActiveChannel: () => true,
    composer,
    setComposer,
    composerAttachments,
    setComposerAttachments,
    composerAttachmentInputElement: () => undefined,
    isSendingMessage,
    setSendingMessage,
    setSendMessageState,
    setMessageStatus,
    setMessageError,
    setMessages: vi.fn(),
    setAttachmentByChannel: vi.fn(),
    isMessageListNearBottom: () => true,
    scrollMessageListToBottom: vi.fn(),
    editingMessageId: () => null,
    setEditingMessageId: vi.fn(),
    editingDraft: () => "",
    setEditingDraft: vi.fn(),
    isSavingEdit: () => false,
    setSavingEdit: vi.fn(),
    deletingMessageId: () => null,
    setDeletingMessageId: vi.fn(),
    reactionState: () => ({}),
    setReactionState: vi.fn(),
    pendingReactionByKey: () => ({}),
    setPendingReactionByKey: vi.fn(),
    openReactionPickerMessageId: () => null,
    setOpenReactionPickerMessageId: vi.fn(),
  });

  return {
    controller,
    messageStatus,
    messageError,
    sendMessageState,
  };
}

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
    // Default base is 250ms
    expect(mediaPreviewRetryDelayMs(1)).toBe(250);
    // 2nd attempt: 250 * 1.5 = 375
    expect(mediaPreviewRetryDelayMs(2)).toBe(375);
  });

  it("calculates retry delays with exponential backoff and cap", () => {
    // 1st attempt: 250 * 1 = 250
    expect(mediaPreviewRetryDelayMs(1, 250)).toBe(250);
    // 2nd attempt: 250 * 1.5 = 375
    expect(mediaPreviewRetryDelayMs(2, 250)).toBe(375);
    // 5th attempt: 250 * 1.5^4 = 1265.625
    expect(mediaPreviewRetryDelayMs(5, 250)).toBeCloseTo(1265.625);
    // High attempt cap: 10000
    expect(mediaPreviewRetryDelayMs(100, 250)).toBe(10000);
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

  it("sets failed send state when no channel is selected", async () => {
    const harness = createRoot(() => createMessageActionsHarness());

    await harness.controller.sendMessage({
      preventDefault() {},
    } as SubmitEvent);

    expect(harness.messageError()).toBe("Select a channel first.");
    expect(harness.sendMessageState()).toEqual({
      phase: "failed",
      statusMessage: "",
      errorMessage: "Select a channel first.",
    });
  });

  it("sets failed send state when composer has no content or attachments", async () => {
    const harness = createRoot(() =>
      createMessageActionsHarness({
        activeGuildId: guildIdFromInput(GUILD_ID),
        activeChannelId: channelIdFromInput(CHANNEL_ID),
      }),
    );

    await harness.controller.sendMessage({
      preventDefault() {},
    } as SubmitEvent);

    expect(harness.messageStatus()).toBe("");
    expect(harness.messageError()).toBe(
      "Message must include text or at least one attachment.",
    );
    expect(harness.sendMessageState()).toEqual({
      phase: "failed",
      statusMessage: "",
      errorMessage: "Message must include text or at least one attachment.",
    });
  });
});
