import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  channelIdFromInput,
  guildIdFromInput,
  messageFromResponse,
  messageIdFromInput,
  type MessageRecord,
} from "../src/domain/chat";
import { createMessageHistoryController } from "../src/features/app-shell/controllers/message-history-controller";
import type { AsyncOperationState } from "../src/features/app-shell/state/async-operation-state";
import {
  isMessageHistoryLoadingForTarget,
  type MessageHistoryLoadTarget,
} from "../src/features/app-shell/state/message-state";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");

function messageFixture(input: {
  messageId: string;
  authorId: string;
  content: string;
  createdAtUnix: number;
}): MessageRecord {
  return messageFromResponse({
    message_id: input.messageId,
    guild_id: GUILD_ID,
    channel_id: CHANNEL_ID,
    author_id: input.authorId,
    content: input.content,
    markdown_tokens: [{ type: "text", text: input.content }],
    attachments: [],
    created_at_unix: input.createdAtUnix,
  });
}

function deferred<T>() {
  let resolve: ((value: T) => void) | undefined;
  let reject: ((reason?: unknown) => void) | undefined;
  const promise = new Promise<T>((innerResolve, innerReject) => {
    resolve = innerResolve;
    reject = innerReject;
  });
  return {
    promise,
    resolve: (value: T) => resolve?.(value),
    reject: (reason?: unknown) => reject?.(reason),
  };
}

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe("app shell message history controller", () => {
  it("refreshes and loads older messages while preserving scroll restoration", async () => {
    const [session] = createSignal(SESSION);
    const [activeGuildId] = createSignal(GUILD_ID);
    const [activeChannelId] = createSignal(CHANNEL_ID);
    const [canAccessActiveChannel] = createSignal(true);
    const [nextBefore, setNextBefore] = createSignal<
      ReturnType<typeof messageIdFromInput> | null
    >(messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAZ"));
    const [messageHistoryLoadTarget, setMessageHistoryLoadTarget] = createSignal<MessageHistoryLoadTarget | null>(null);
    const isLoadingMessages = () =>
      isMessageHistoryLoadingForTarget(
        refreshMessagesState(),
        messageHistoryLoadTarget(),
        "refresh",
      );
    const isLoadingOlder = () =>
      isMessageHistoryLoadingForTarget(
        refreshMessagesState(),
        messageHistoryLoadTarget(),
        "load-older",
      );
    const [messages, setMessages] = createSignal<MessageRecord[]>([]);
    const [showLoadOlderButton, setShowLoadOlderButton] = createSignal(false);
    const [messageError, setMessageError] = createSignal("");
    const [refreshMessagesState, setRefreshMessagesState] =
      createSignal<AsyncOperationState>({
        phase: "idle",
        statusMessage: "",
        errorMessage: "",
      });

    const scrollToBottomMock = vi.fn();
    const captureScrollMetricsMock = vi.fn(() => ({
      scrollHeight: 1400,
      scrollTop: 300,
    }));
    const restoreScrollAfterPrependMock = vi.fn();

    const fetchChannelMessagesMock = vi.fn(async (_session, _guildId, _channelId, input) => {
      if (input?.before) {
        return {
          messages: [
            messageFixture({
              messageId: "01ARZ3NDEKTSV4RRFFQ69G5FAX",
              authorId: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
              content: "older",
              createdAtUnix: 1,
            }),
          ],
          nextBefore: null,
        };
      }
      return {
        messages: [
          messageFixture({
            messageId: "01ARZ3NDEKTSV4RRFFQ69G5FAY",
            authorId: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
            content: "latest",
            createdAtUnix: 2,
          }),
        ],
        nextBefore: messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAZ"),
      };
    });

    const controller = createRoot(() =>
      createMessageHistoryController(
        {
          session,
          activeGuildId,
          activeChannelId,
          canAccessActiveChannel,
          nextBefore,
          refreshMessagesState,
          messageHistoryLoadTarget,
          setMessages,
          setNextBefore,
          setShowLoadOlderButton,
          setMessageError,
          setRefreshMessagesState,
          setMessageHistoryLoadTarget,
          setEditingMessageId: vi.fn(),
          setEditingDraft: vi.fn(),
          setReactionState: vi.fn(),
          setPendingReactionByKey: vi.fn(),
          setOpenReactionPickerMessageId: vi.fn(),
          setSearchResults: vi.fn(),
          setSearchError: vi.fn(),
          setSearchOpsStatus: vi.fn(),
          setAttachmentStatus: vi.fn(),
          setAttachmentError: vi.fn(),
          setVoiceStatus: vi.fn(),
          setVoiceError: vi.fn(),
          captureScrollMetrics: captureScrollMetricsMock,
          restoreScrollAfterPrepend: restoreScrollAfterPrependMock,
          scrollMessageListToBottom: scrollToBottomMock,
        },
        {
          fetchChannelMessages: fetchChannelMessagesMock,
        },
      ),
    );

    await flush();
    expect(fetchChannelMessagesMock).toHaveBeenCalledWith(SESSION, GUILD_ID, CHANNEL_ID, {
      limit: 50,
    });
    expect(messages().map((entry) => entry.content)).toEqual(["latest"]);
    expect(messageHistoryLoadTarget()).toBeNull();
    expect(messageError()).toBe("");
    expect(refreshMessagesState()).toEqual({
      phase: "succeeded",
      statusMessage: "",
      errorMessage: "",
    });
    expect(scrollToBottomMock).toHaveBeenCalledTimes(1);

    await controller.loadOlderMessages();
    expect(fetchChannelMessagesMock).toHaveBeenCalledWith(SESSION, GUILD_ID, CHANNEL_ID, {
      limit: 50,
      before: messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAZ"),
    });
    expect(messages().map((entry) => entry.content)).toEqual(["older", "latest"]);
    expect(restoreScrollAfterPrependMock).toHaveBeenCalledTimes(1);
    expect(showLoadOlderButton()).toBe(false);
    expect(refreshMessagesState().phase).toBe("succeeded");
  });

  it("ignores stale refresh responses after channel access reset", async () => {
    const [session] = createSignal(SESSION);
    const [activeGuildId] = createSignal(GUILD_ID);
    const [activeChannelId] = createSignal(CHANNEL_ID);
    const [canAccessActiveChannel, setCanAccessActiveChannel] = createSignal(true);
    const [nextBefore, setNextBefore] = createSignal<
      ReturnType<typeof messageIdFromInput> | null
    >(null);
    const [refreshMessagesState, setRefreshMessagesState] =
      createSignal<AsyncOperationState>({
        phase: "idle",
        statusMessage: "",
        errorMessage: "",
      });
    const [messageHistoryLoadTarget, setMessageHistoryLoadTarget] = createSignal<MessageHistoryLoadTarget | null>(null);
    const isLoadingMessages = () =>
      isMessageHistoryLoadingForTarget(
        refreshMessagesState(),
        messageHistoryLoadTarget(),
        "refresh",
      );
    const isLoadingOlder = () =>
      isMessageHistoryLoadingForTarget(
        refreshMessagesState(),
        messageHistoryLoadTarget(),
        "load-older",
      );
    const [messages, setMessages] = createSignal<MessageRecord[]>([]);
    const [messageError, setMessageError] = createSignal("");

    const pendingRefresh = deferred<{
      messages: MessageRecord[];
      nextBefore: ReturnType<typeof messageIdFromInput> | null;
    }>();
    const fetchChannelMessagesMock = vi.fn(() => pendingRefresh.promise);

    createRoot(() =>
      createMessageHistoryController(
        {
          session,
          activeGuildId,
          activeChannelId,
          canAccessActiveChannel,
          nextBefore,
          refreshMessagesState,
          messageHistoryLoadTarget,
          setMessages,
          setNextBefore,
          setShowLoadOlderButton: vi.fn(),
          setMessageError,
          setRefreshMessagesState,
          setMessageHistoryLoadTarget,
          setEditingMessageId: vi.fn(),
          setEditingDraft: vi.fn(),
          setReactionState: vi.fn(),
          setPendingReactionByKey: vi.fn(),
          setOpenReactionPickerMessageId: vi.fn(),
          setSearchResults: vi.fn(),
          setSearchError: vi.fn(),
          setSearchOpsStatus: vi.fn(),
          setAttachmentStatus: vi.fn(),
          setAttachmentError: vi.fn(),
          setVoiceStatus: vi.fn(),
          setVoiceError: vi.fn(),
          captureScrollMetrics: vi.fn(() => null),
          restoreScrollAfterPrepend: vi.fn(),
          scrollMessageListToBottom: vi.fn(),
        },
        {
          fetchChannelMessages: fetchChannelMessagesMock,
        },
      ),
    );

    setCanAccessActiveChannel(false);
    pendingRefresh.resolve({
      messages: [
        messageFixture({
          messageId: "01ARZ3NDEKTSV4RRFFQ69G5FAZ",
          authorId: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
          content: "stale",
          createdAtUnix: 1,
        }),
      ],
      nextBefore: null,
    });
    await flush();

    expect(messages()).toEqual([]);
    expect(nextBefore()).toBeNull();
    expect(messageError()).toBe("");
    expect(messageHistoryLoadTarget()).toBeNull();
    expect(isLoadingOlder()).toBe(false);
    expect(refreshMessagesState()).toEqual({
      phase: "idle",
      statusMessage: "",
      errorMessage: "",
    });
  });

  it("blocks load-older while refresh is already running", async () => {
    const [session] = createSignal(SESSION);
    const [activeGuildId] = createSignal(GUILD_ID);
    const [activeChannelId] = createSignal(CHANNEL_ID);
    const [canAccessActiveChannel] = createSignal(true);
    const [nextBefore, setNextBefore] = createSignal<
      ReturnType<typeof messageIdFromInput> | null
    >(messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAZ"));
    const [refreshMessagesState, setRefreshMessagesState] =
      createSignal<AsyncOperationState>({
        phase: "idle",
        statusMessage: "",
        errorMessage: "",
      });
    const [messageHistoryLoadTarget, setMessageHistoryLoadTarget] = createSignal<MessageHistoryLoadTarget | null>(null);
    const isLoadingMessages = () =>
      isMessageHistoryLoadingForTarget(
        refreshMessagesState(),
        messageHistoryLoadTarget(),
        "refresh",
      );
    const isLoadingOlder = () =>
      isMessageHistoryLoadingForTarget(
        refreshMessagesState(),
        messageHistoryLoadTarget(),
        "load-older",
      );

    const pendingRefresh = deferred<{
      messages: MessageRecord[];
      nextBefore: ReturnType<typeof messageIdFromInput> | null;
    }>();
    const fetchChannelMessagesMock = vi.fn(() => pendingRefresh.promise);

    const controller = createRoot(() =>
      createMessageHistoryController(
        {
          session,
          activeGuildId,
          activeChannelId,
          canAccessActiveChannel,
          nextBefore,
          refreshMessagesState,
          messageHistoryLoadTarget,
          setMessages: vi.fn(),
          setNextBefore,
          setShowLoadOlderButton: vi.fn(),
          setMessageError: vi.fn(),
          setRefreshMessagesState,
          setMessageHistoryLoadTarget,
          setEditingMessageId: vi.fn(),
          setEditingDraft: vi.fn(),
          setReactionState: vi.fn(),
          setPendingReactionByKey: vi.fn(),
          setOpenReactionPickerMessageId: vi.fn(),
          setSearchResults: vi.fn(),
          setSearchError: vi.fn(),
          setSearchOpsStatus: vi.fn(),
          setAttachmentStatus: vi.fn(),
          setAttachmentError: vi.fn(),
          setVoiceStatus: vi.fn(),
          setVoiceError: vi.fn(),
          captureScrollMetrics: vi.fn(() => null),
          restoreScrollAfterPrepend: vi.fn(),
          scrollMessageListToBottom: vi.fn(),
        },
        {
          fetchChannelMessages: fetchChannelMessagesMock,
        },
      ),
    );

    await flush();
    expect(fetchChannelMessagesMock).toHaveBeenCalledTimes(1);
    expect(isLoadingMessages()).toBe(true);

    await controller.loadOlderMessages();
    expect(fetchChannelMessagesMock).toHaveBeenCalledTimes(1);
    expect(isLoadingOlder()).toBe(false);

    pendingRefresh.resolve({ messages: [], nextBefore: null });
    await flush();
  });
});
