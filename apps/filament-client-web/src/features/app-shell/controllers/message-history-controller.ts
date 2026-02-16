import { createEffect, type Accessor, type Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import type {
  ChannelId,
  GuildId,
  MessageId,
  MessageRecord,
  SearchResults,
} from "../../../domain/chat";
import { fetchChannelMessages } from "../../../lib/api";
import {
  mapError,
  mergeMessageHistory,
  normalizeMessageOrder,
  type ReactionView,
} from "../helpers";
import {
  reduceAsyncOperationState,
  type AsyncOperationState,
} from "../state/async-operation-state";
import type { MessageHistoryLoadTarget } from "../state/message-state";
import type { MessageListScrollMetrics } from "./message-list-controller";

export interface MessageHistoryControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  canAccessActiveChannel: Accessor<boolean>;
  nextBefore: Accessor<MessageId | null>;
  isLoadingOlder: Accessor<boolean>;
  setMessages: Setter<MessageRecord[]>;
  setNextBefore: Setter<MessageId | null>;
  setShowLoadOlderButton: Setter<boolean>;
  setMessageError: Setter<string>;
  setRefreshMessagesState: Setter<AsyncOperationState>;
  setMessageHistoryLoadTarget: Setter<MessageHistoryLoadTarget | null>;
  setEditingMessageId: Setter<MessageId | null>;
  setEditingDraft: Setter<string>;
  setReactionState: Setter<Record<string, ReactionView>>;
  setPendingReactionByKey: Setter<Record<string, true>>;
  setOpenReactionPickerMessageId: Setter<MessageId | null>;
  setSearchResults: Setter<SearchResults | null>;
  setSearchError: Setter<string>;
  setSearchOpsStatus: Setter<string>;
  setAttachmentStatus: Setter<string>;
  setAttachmentError: Setter<string>;
  setVoiceStatus: Setter<string>;
  setVoiceError: Setter<string>;
  captureScrollMetrics: () => MessageListScrollMetrics | null;
  restoreScrollAfterPrepend: (metrics: MessageListScrollMetrics | null) => void;
  scrollMessageListToBottom: () => void;
}

export interface MessageHistoryControllerDependencies {
  fetchChannelMessages: typeof fetchChannelMessages;
}

export interface MessageHistoryController {
  refreshMessages: () => Promise<void>;
  loadOlderMessages: () => Promise<void>;
}

const DEFAULT_MESSAGE_HISTORY_CONTROLLER_DEPENDENCIES: MessageHistoryControllerDependencies = {
  fetchChannelMessages,
};

export function createMessageHistoryController(
  options: MessageHistoryControllerOptions,
  dependencies: Partial<MessageHistoryControllerDependencies> = {},
): MessageHistoryController {
  const deps = {
    ...DEFAULT_MESSAGE_HISTORY_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };
  let historyRequestVersion = 0;

  const refreshMessages = async (): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    if (!session || !guildId || !channelId) {
      options.setMessages([]);
      options.setNextBefore(null);
      options.setShowLoadOlderButton(false);
      options.setMessageHistoryLoadTarget(null);
      options.setRefreshMessagesState((existing) =>
        reduceAsyncOperationState(existing, {
          type: "reset",
        }),
      );
      return;
    }

    const requestVersion = ++historyRequestVersion;
    options.setMessageError("");
    options.setRefreshMessagesState((existing) =>
      reduceAsyncOperationState(existing, {
        type: "start",
      }),
    );
    options.setMessageHistoryLoadTarget("refresh");
    try {
      const history = await deps.fetchChannelMessages(session, guildId, channelId, {
        limit: 50,
      });
      if (requestVersion !== historyRequestVersion) {
        return;
      }
      options.setMessages(normalizeMessageOrder(history.messages));
      options.setNextBefore(history.nextBefore);
      options.setEditingMessageId(null);
      options.setEditingDraft("");
      options.scrollMessageListToBottom();
      options.setRefreshMessagesState((existing) =>
        reduceAsyncOperationState(existing, {
          type: "succeed",
        }),
      );
    } catch (error) {
      if (requestVersion !== historyRequestVersion) {
        return;
      }
      const errorMessage = mapError(error, "Unable to load messages.");
      options.setMessageError(errorMessage);
      options.setRefreshMessagesState((existing) =>
        reduceAsyncOperationState(existing, {
          type: "fail",
          errorMessage,
        }),
      );
      options.setMessages([]);
      options.setNextBefore(null);
      options.setShowLoadOlderButton(false);
    } finally {
      if (requestVersion === historyRequestVersion) {
        options.setMessageHistoryLoadTarget(null);
      }
    }
  };

  const loadOlderMessages = async (): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    const before = options.nextBefore();
    if (!session || !guildId || !channelId || !before || options.isLoadingOlder()) {
      return;
    }

    const requestVersion = historyRequestVersion;
    const previousScrollMetrics = options.captureScrollMetrics();
    options.setMessageError("");
    options.setRefreshMessagesState((existing) =>
      reduceAsyncOperationState(existing, {
        type: "start",
      }),
    );
    options.setMessageHistoryLoadTarget("load-older");
    try {
      const history = await deps.fetchChannelMessages(session, guildId, channelId, {
        limit: 50,
        before,
      });
      if (requestVersion !== historyRequestVersion) {
        return;
      }
      options.setMessages((existing) =>
        mergeMessageHistory(existing, history.messages),
      );
      options.setNextBefore(history.nextBefore);
      options.restoreScrollAfterPrepend(previousScrollMetrics);
      options.setRefreshMessagesState((existing) =>
        reduceAsyncOperationState(existing, {
          type: "succeed",
        }),
      );
    } catch (error) {
      if (requestVersion !== historyRequestVersion) {
        return;
      }
      const errorMessage = mapError(error, "Unable to load older messages.");
      options.setMessageError(errorMessage);
      options.setRefreshMessagesState((existing) =>
        reduceAsyncOperationState(existing, {
          type: "fail",
          errorMessage,
        }),
      );
    } finally {
      if (requestVersion === historyRequestVersion) {
        options.setMessageHistoryLoadTarget(null);
      }
    }
  };

  createEffect(() => {
    void options.activeGuildId();
    void options.activeChannelId();
    const canRead = options.canAccessActiveChannel();
    historyRequestVersion += 1;
    options.setReactionState({});
    options.setPendingReactionByKey({});
    options.setOpenReactionPickerMessageId(null);
    options.setSearchResults(null);
    options.setSearchError("");
    options.setSearchOpsStatus("");
    options.setAttachmentStatus("");
    options.setAttachmentError("");
    options.setVoiceStatus("");
    options.setVoiceError("");
    if (canRead) {
      void refreshMessages();
      return;
    }
    options.setMessages([]);
    options.setNextBefore(null);
    options.setShowLoadOlderButton(false);
    options.setMessageHistoryLoadTarget(null);
    options.setRefreshMessagesState((existing) =>
      reduceAsyncOperationState(existing, {
        type: "reset",
      }),
    );
  });

  return {
    refreshMessages,
    loadOlderMessages,
  };
}
