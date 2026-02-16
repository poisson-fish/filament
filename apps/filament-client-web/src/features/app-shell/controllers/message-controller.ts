import {
  createEffect,
  createSignal,
  onCleanup,
  untrack,
  type Accessor,
  type Setter,
} from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import type {
  AttachmentId,
  AttachmentRecord,
  ChannelRecord,
  ChannelId,
  GuildId,
  MessageId,
  MessageRecord,
  ReactionEmoji,
} from "../../../domain/chat";
import {
  attachmentFilenameFromInput,
  messageContentFromInput,
} from "../../../domain/chat";
import {
  ApiError,
  addMessageReaction,
  createChannelMessage,
  deleteChannelAttachment,
  deleteChannelMessage,
  downloadChannelAttachmentPreview,
  editChannelMessage,
  refreshAuthSession,
  removeMessageReaction,
  uploadChannelAttachment,
} from "../../../lib/api";
import {
  channelKey,
  clearKeysByPrefix,
  createObjectUrl,
  mapError,
  mergeMessage,
  reactionKey,
  resolveAttachmentPreviewType,
  revokeObjectUrl,
  upsertReactionEntry,
  type MessageMediaPreview,
  type ReactionView,
} from "../helpers";
import {
  reduceAsyncOperationState,
  type AsyncOperationState,
} from "../state/async-operation-state";

const DEFAULT_MAX_PREVIEW_BYTES = 25 * 1024 * 1024;
const DEFAULT_MAX_MEDIA_PREVIEW_RETRIES = 20;
const DEFAULT_INITIAL_MEDIA_PREVIEW_DELAY_MS = 250;
const DEFAULT_MAX_COMPOSER_ATTACHMENTS = 5;

export interface MessageMediaPreviewControllerOptions {
  session: Accessor<AuthSession | null>;
  setAuthenticatedSession: (session: AuthSession) => void;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  messages: Accessor<MessageRecord[]>;
  maxPreviewBytes?: number;
  maxRetries?: number;
  initialDelayMs?: number;
}

export interface MessageMediaPreviewController {
  messageMediaByAttachmentId: Accessor<Record<string, MessageMediaPreview>>;
  loadingMediaPreviewIds: Accessor<Record<string, true>>;
  failedMediaPreviewIds: Accessor<Record<string, true>>;
  retryMediaPreview: (attachmentId: AttachmentId) => void;
}

export interface ComposerAttachmentMergeResult {
  files: File[];
  reachedCap: boolean;
}

export interface MessageActionsControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  activeChannel: Accessor<ChannelRecord | null>;
  canAccessActiveChannel: Accessor<boolean>;
  composer: Accessor<string>;
  setComposer: Setter<string>;
  composerAttachments: Accessor<File[]>;
  setComposerAttachments: Setter<File[]>;
  composerAttachmentInputElement: Accessor<HTMLInputElement | undefined>;
  isSendingMessage: Accessor<boolean>;
  setSendMessageState: Setter<AsyncOperationState>;
  setMessageStatus: Setter<string>;
  setMessageError: Setter<string>;
  setMessages: Setter<MessageRecord[]>;
  setAttachmentByChannel: Setter<Record<string, AttachmentRecord[]>>;
  isMessageListNearBottom: () => boolean;
  scrollMessageListToBottom: () => void;
  editingMessageId: Accessor<MessageId | null>;
  setEditingMessageId: Setter<MessageId | null>;
  editingDraft: Accessor<string>;
  setEditingDraft: Setter<string>;
  isSavingEdit: Accessor<boolean>;
  setSavingEdit: Setter<boolean>;
  deletingMessageId: Accessor<MessageId | null>;
  setDeletingMessageId: Setter<MessageId | null>;
  reactionState: Accessor<Record<string, ReactionView>>;
  setReactionState: Setter<Record<string, ReactionView>>;
  pendingReactionByKey: Accessor<Record<string, true>>;
  setPendingReactionByKey: Setter<Record<string, true>>;
  openReactionPickerMessageId: Accessor<MessageId | null>;
  setOpenReactionPickerMessageId: Setter<MessageId | null>;
  maxComposerAttachments?: number;
}

export interface MessageActionsController {
  sendMessage: (event: SubmitEvent) => Promise<void>;
  openComposerAttachmentPicker: () => void;
  onComposerAttachmentInput: (
    event: InputEvent & { currentTarget: HTMLInputElement },
  ) => void;
  removeComposerAttachment: (target: File) => void;
  beginEditMessage: (message: MessageRecord) => void;
  cancelEditMessage: () => void;
  saveEditMessage: (messageId: MessageId) => Promise<void>;
  removeMessage: (messageId: MessageId) => Promise<void>;
  toggleReactionPicker: (messageId: MessageId) => void;
  toggleMessageReaction: (
    messageId: MessageId,
    emoji: ReactionEmoji,
  ) => Promise<void>;
  addReactionFromPicker: (
    messageId: MessageId,
    emoji: ReactionEmoji,
  ) => Promise<void>;
}

export function mergeComposerAttachmentSelection(
  existingFiles: File[],
  incomingFiles: File[],
  maxAttachments: number,
): ComposerAttachmentMergeResult {
  const existingKeys = new Set(
    existingFiles.map(
      (file) => `${file.name}:${file.size}:${file.lastModified}:${file.type}`,
    ),
  );
  const next = [...existingFiles];
  let reachedCap = existingFiles.length >= maxAttachments;
  for (const file of incomingFiles) {
    const dedupeKey = `${file.name}:${file.size}:${file.lastModified}:${file.type}`;
    if (existingKeys.has(dedupeKey)) {
      continue;
    }
    if (next.length >= maxAttachments) {
      reachedCap = true;
      break;
    }
    next.push(file);
    existingKeys.add(dedupeKey);
  }
  return {
    files: next,
    reachedCap,
  };
}

export function clearReactionRecordsForMessage<T>(
  existing: Record<string, T>,
  messageId: MessageId,
): Record<string, T> {
  return clearKeysByPrefix(existing, `${messageId}|`);
}

export function collectMediaPreviewTargets(
  messages: MessageRecord[],
  maxPreviewBytes: number,
): Map<AttachmentId, AttachmentRecord> {
  const previewTargets = new Map<AttachmentId, AttachmentRecord>();
  for (const message of messages) {
    for (const attachment of message.attachments) {
      const { kind } = resolveAttachmentPreviewType(
        null,
        attachment.mimeType,
        attachment.filename,
      );
      if (kind === "file" || attachment.sizeBytes > maxPreviewBytes) {
        continue;
      }
      previewTargets.set(attachment.attachmentId, attachment);
    }
  }
  return previewTargets;
}

export function retainRecordByAllowedIds<T>(
  existing: Record<string, T>,
  allowedIds: Set<string>,
): Record<string, T> {
  return Object.fromEntries(
    Object.entries(existing).filter(([id]) => allowedIds.has(id)),
  ) as Record<string, T>;
}

export function nextMediaPreviewAttempt(
  currentAttempts: ReadonlyMap<string, number>,
  attachmentId: string,
): number {
  return (currentAttempts.get(attachmentId) ?? 0) + 1;
}

export function shouldRetryMediaPreview(
  attempt: number,
  maxRetries: number,
): boolean {
  return attempt <= maxRetries;
}

export function mediaPreviewRetryDelayMs(
  attempt: number,
  baseDelayMs = 250,
): number {
  const delay = baseDelayMs * Math.pow(1.5, Math.max(attempt - 1, 0));
  return Math.min(delay, 10_000);
}

export function createMessageActionsController(
  options: MessageActionsControllerOptions,
): MessageActionsController {
  const maxComposerAttachments =
    options.maxComposerAttachments ?? DEFAULT_MAX_COMPOSER_ATTACHMENTS;

  const setReactionPending = (key: string, pending: boolean) => {
    options.setPendingReactionByKey((existing) => {
      if (pending) {
        if (existing[key]) {
          return existing;
        }
        return { ...existing, [key]: true };
      }
      if (!existing[key]) {
        return existing;
      }
      const next = { ...existing };
      delete next[key];
      return next;
    });
  };

  const clearReactionStateForMessage = (messageId: MessageId) => {
    options.setReactionState((existing) =>
      clearReactionRecordsForMessage(existing, messageId),
    );
    options.setPendingReactionByKey((existing) =>
      clearReactionRecordsForMessage(existing, messageId),
    );
    if (options.openReactionPickerMessageId() === messageId) {
      options.setOpenReactionPickerMessageId(null);
    }
  };

  const sendMessage = async (event: SubmitEvent): Promise<void> => {
    event.preventDefault();
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    if (!session || !guildId || !channelId) {
      options.setMessageError("Select a channel first.");
      options.setSendMessageState((existing) =>
        reduceAsyncOperationState(existing, {
          type: "fail",
          errorMessage: "Select a channel first.",
        }),
      );
      return;
    }
    if (options.isSendingMessage()) {
      return;
    }

    options.setMessageError("");
    options.setMessageStatus("");
    options.setSendMessageState((existing) =>
      reduceAsyncOperationState(existing, {
        type: "start",
      }),
    );
    let uploadedForMessage: AttachmentRecord[] = [];
    try {
      const draft = options.composer().trim();
      const selectedFiles = options.composerAttachments();
      if (draft.length === 0 && selectedFiles.length === 0) {
        options.setMessageError(
          "Message must include text or at least one attachment.",
        );
        options.setSendMessageState((existing) =>
          reduceAsyncOperationState(existing, {
            type: "fail",
            errorMessage: "Message must include text or at least one attachment.",
          }),
        );
        return;
      }
      for (const file of selectedFiles) {
        const filename = attachmentFilenameFromInput(file.name);
        const uploaded = await uploadChannelAttachment(
          session,
          guildId,
          channelId,
          file,
          filename,
        );
        uploadedForMessage.push(uploaded);
      }

      const created = await createChannelMessage(session, guildId, channelId, {
        content: messageContentFromInput(draft),
        attachmentIds:
          uploadedForMessage.length > 0
            ? uploadedForMessage.map((record) => record.attachmentId)
            : undefined,
      });
      const shouldStickToBottom = options.isMessageListNearBottom();
      options.setMessages((existing) => mergeMessage(existing, created));
      if (shouldStickToBottom) {
        options.scrollMessageListToBottom();
      }
      options.setComposer("");
      options.setComposerAttachments([]);
      const attachmentInput = options.composerAttachmentInputElement();
      if (attachmentInput) {
        attachmentInput.value = "";
      }
      if (uploadedForMessage.length > 0) {
        const key = channelKey(guildId, channelId);
        options.setAttachmentByChannel((existing) => {
          const current = existing[key] ?? [];
          const uploadedIds = new Set(
            uploadedForMessage.map((record) => record.attachmentId),
          );
          const deduped = current.filter(
            (entry) => !uploadedIds.has(entry.attachmentId),
          );
          return {
            ...existing,
            [key]: [...uploadedForMessage, ...deduped],
          };
        });
        options.setMessageStatus(
          `Sent with ${uploadedForMessage.length} attachment${uploadedForMessage.length === 1 ? "" : "s"}.`,
        );
        options.setSendMessageState((existing) =>
          reduceAsyncOperationState(existing, {
            type: "succeed",
            statusMessage: `Sent with ${uploadedForMessage.length} attachment${uploadedForMessage.length === 1 ? "" : "s"}.`,
          }),
        );
      } else {
        options.setSendMessageState((existing) =>
          reduceAsyncOperationState(existing, {
            type: "succeed",
          }),
        );
      }
    } catch (error) {
      if (uploadedForMessage.length > 0) {
        await Promise.allSettled(
          uploadedForMessage.map((record) =>
            deleteChannelAttachment(session, guildId, channelId, record.attachmentId),
          ),
        );
      }
      const errorMessage = mapError(error, "Unable to send message.");
      options.setMessageError(errorMessage);
      options.setSendMessageState((existing) =>
        reduceAsyncOperationState(existing, {
          type: "fail",
          errorMessage,
        }),
      );
    }
  };

  const openComposerAttachmentPicker = () => {
    if (!options.activeChannel() || !options.canAccessActiveChannel()) {
      options.setMessageError("Select a channel first.");
      return;
    }
    options.composerAttachmentInputElement()?.click();
  };

  const onComposerAttachmentInput = (
    event: InputEvent & { currentTarget: HTMLInputElement },
  ) => {
    const incomingFiles = [...(event.currentTarget.files ?? [])];
    if (incomingFiles.length === 0) {
      return;
    }

    options.setMessageError("");
    const merged = mergeComposerAttachmentSelection(
      options.composerAttachments(),
      incomingFiles,
      maxComposerAttachments,
    );
    options.setComposerAttachments(merged.files);

    const attachmentInput = options.composerAttachmentInputElement();
    if (attachmentInput) {
      attachmentInput.value = "";
    }
    if (merged.reachedCap) {
      options.setMessageError(
        `Maximum ${maxComposerAttachments} attachments per message.`,
      );
    }
  };

  const removeComposerAttachment = (target: File) => {
    options.setComposerAttachments((existing) =>
      existing.filter(
        (file) =>
          !(
            file.name === target.name &&
            file.size === target.size &&
            file.lastModified === target.lastModified &&
            file.type === target.type
          ),
      ),
    );
  };

  const beginEditMessage = (message: MessageRecord) => {
    options.setEditingMessageId(message.messageId);
    options.setEditingDraft(message.content);
  };

  const cancelEditMessage = () => {
    options.setEditingMessageId(null);
    options.setEditingDraft("");
  };

  const saveEditMessage = async (messageId: MessageId): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    if (!session || !guildId || !channelId || options.isSavingEdit()) {
      return;
    }

    options.setSavingEdit(true);
    options.setMessageError("");
    try {
      const updated = await editChannelMessage(session, guildId, channelId, messageId, {
        content: messageContentFromInput(options.editingDraft()),
      });
      options.setMessages((existing) => mergeMessage(existing, updated));
      options.setEditingMessageId(null);
      options.setEditingDraft("");
      options.setMessageStatus("Message updated.");
    } catch (error) {
      options.setMessageError(mapError(error, "Unable to edit message."));
    } finally {
      options.setSavingEdit(false);
    }
  };

  const removeMessage = async (messageId: MessageId): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    if (!session || !guildId || !channelId || options.deletingMessageId()) {
      return;
    }

    options.setDeletingMessageId(messageId);
    options.setMessageError("");
    try {
      await deleteChannelMessage(session, guildId, channelId, messageId);
      options.setMessages((existing) =>
        existing.filter((entry) => entry.messageId !== messageId),
      );
      if (options.editingMessageId() === messageId) {
        cancelEditMessage();
      }
      clearReactionStateForMessage(messageId);
      options.setMessageStatus("Message deleted.");
    } catch (error) {
      options.setMessageError(mapError(error, "Unable to delete message."));
    } finally {
      options.setDeletingMessageId(null);
    }
  };

  const toggleReactionPicker = (messageId: MessageId) => {
    options.setOpenReactionPickerMessageId((existing) =>
      existing === messageId ? null : messageId,
    );
  };

  const toggleMessageReaction = async (
    messageId: MessageId,
    emoji: ReactionEmoji,
  ): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    if (!session || !guildId || !channelId) {
      return;
    }

    const key = reactionKey(messageId, emoji);
    if (options.pendingReactionByKey()[key]) {
      return;
    }
    setReactionPending(key, true);
    const state = options.reactionState()[key] ?? { count: 0, reacted: false };

    try {
      if (state.reacted) {
        const response = await removeMessageReaction(
          session,
          guildId,
          channelId,
          messageId,
          emoji,
        );
        options.setReactionState((existing) =>
          upsertReactionEntry(existing, key, {
            count: response.count,
            reacted: false,
          }),
        );
      } else {
        const response = await addMessageReaction(
          session,
          guildId,
          channelId,
          messageId,
          emoji,
        );
        options.setReactionState((existing) =>
          upsertReactionEntry(existing, key, {
            count: response.count,
            reacted: true,
          }),
        );
      }
    } catch (error) {
      options.setMessageError(mapError(error, "Unable to update reaction."));
    } finally {
      setReactionPending(key, false);
    }
  };

  const addReactionFromPicker = async (
    messageId: MessageId,
    emoji: ReactionEmoji,
  ): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    if (!session || !guildId || !channelId) {
      return;
    }

    const key = reactionKey(messageId, emoji);
    if (options.pendingReactionByKey()[key]) {
      return;
    }
    setReactionPending(key, true);
    options.setOpenReactionPickerMessageId(null);
    try {
      const response = await addMessageReaction(
        session,
        guildId,
        channelId,
        messageId,
        emoji,
      );
      options.setReactionState((existing) =>
        upsertReactionEntry(existing, key, {
          count: response.count,
          reacted: true,
        }),
      );
    } catch (error) {
      options.setMessageError(mapError(error, "Unable to update reaction."));
    } finally {
      setReactionPending(key, false);
    }
  };

  return {
    sendMessage,
    openComposerAttachmentPicker,
    onComposerAttachmentInput,
    removeComposerAttachment,
    beginEditMessage,
    cancelEditMessage,
    saveEditMessage,
    removeMessage,
    toggleReactionPicker,
    toggleMessageReaction,
    addReactionFromPicker,
  };
}

export function createMessageMediaPreviewController(
  options: MessageMediaPreviewControllerOptions,
): MessageMediaPreviewController {
  const maxPreviewBytes = options.maxPreviewBytes ?? DEFAULT_MAX_PREVIEW_BYTES;
  const maxRetries = options.maxRetries ?? DEFAULT_MAX_MEDIA_PREVIEW_RETRIES;
  const initialDelayMs =
    options.initialDelayMs ?? DEFAULT_INITIAL_MEDIA_PREVIEW_DELAY_MS;

  const inflightMessageMediaLoads = new Set<string>();
  const previewRetryAttempts = new Map<string, number>();
  let previewSessionRefreshPromise: Promise<void> | null = null;

  const [messageMediaByAttachmentId, setMessageMediaByAttachmentId] = createSignal<
    Record<string, MessageMediaPreview>
  >({});
  const [loadingMediaPreviewIds, setLoadingMediaPreviewIds] = createSignal<
    Record<string, true>
  >({});
  const [failedMediaPreviewIds, setFailedMediaPreviewIds] = createSignal<
    Record<string, true>
  >({});
  const [mediaPreviewRetryTick, setMediaPreviewRetryTick] = createSignal(0);

  createEffect(() => {
    void mediaPreviewRetryTick();
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    const messageList = options.messages();
    if (!session || !guildId || !channelId) {
      setMessageMediaByAttachmentId((existing) => {
        for (const preview of Object.values(existing)) {
          revokeObjectUrl(preview.url);
        }
        return {};
      });
      setLoadingMediaPreviewIds({});
      setFailedMediaPreviewIds({});
      previewRetryAttempts.clear();
      return;
    }

    const previewTargets = collectMediaPreviewTargets(messageList, maxPreviewBytes);
    const existingPreviews = untrack(() => messageMediaByAttachmentId());
    const targetIds = new Set<string>([...previewTargets.keys()]);

    setMessageMediaByAttachmentId((existing) => {
      const next: Record<string, MessageMediaPreview> = {};
      for (const [attachmentId, preview] of Object.entries(existing)) {
        if (targetIds.has(attachmentId)) {
          next[attachmentId] = preview;
        } else {
          revokeObjectUrl(preview.url);
          previewRetryAttempts.delete(attachmentId);
        }
      }
      return next;
    });
    setLoadingMediaPreviewIds((existing) =>
      retainRecordByAllowedIds(existing, targetIds),
    );
    setFailedMediaPreviewIds((existing) =>
      retainRecordByAllowedIds(existing, targetIds),
    );

    let cancelled = false;
    const refreshSessionForPreview = async (): Promise<void> => {
      if (previewSessionRefreshPromise) {
        return previewSessionRefreshPromise;
      }
      const current = options.session();
      if (!current) {
        throw new Error("missing_session");
      }
      previewSessionRefreshPromise = (async () => {
        const next = await refreshAuthSession(current.refreshToken);
        options.setAuthenticatedSession(next);
      })();
      try {
        await previewSessionRefreshPromise;
      } finally {
        previewSessionRefreshPromise = null;
      }
    };

    for (const [attachmentId, attachment] of previewTargets) {
      if (
        existingPreviews[attachmentId] ||
        inflightMessageMediaLoads.has(attachmentId)
      ) {
        continue;
      }
      inflightMessageMediaLoads.add(attachmentId);
      setLoadingMediaPreviewIds((existing) => ({
        ...existing,
        [attachmentId]: true,
      }));
      setFailedMediaPreviewIds((existing) => {
        if (!existing[attachmentId]) {
          return existing;
        }
        const next = { ...existing };
        delete next[attachmentId];
        return next;
      });

      const attempt = previewRetryAttempts.get(attachmentId) ?? 0;
      const runFetch = async () => {
        let activeSession = options.session() ?? session;
        try {
          return await downloadChannelAttachmentPreview(
            activeSession,
            guildId,
            channelId,
            attachmentId,
          );
        } catch (error) {
          if (
            error instanceof ApiError &&
            error.code === "invalid_credentials" &&
            attempt === 0
          ) {
            await refreshSessionForPreview();
            activeSession = options.session() ?? activeSession;
            return downloadChannelAttachmentPreview(
              activeSession,
              guildId,
              channelId,
              attachmentId,
            );
          }
          throw error;
        }
      };

      const processFetch = () =>
        runFetch()
          .then((payload) => {
            if (cancelled) {
              return;
            }
            const { mimeType, kind } = resolveAttachmentPreviewType(
              payload.mimeType,
              attachment.mimeType,
              attachment.filename,
            );
            if (kind === "file") {
              setLoadingMediaPreviewIds((existing) => {
                const next = { ...existing };
                delete next[attachmentId];
                return next;
              });
              return;
            }

            const blob = new Blob([payload.bytes.buffer as ArrayBuffer], {
              type: mimeType,
            });
            const url = createObjectUrl(blob);
            if (!url) {
              setLoadingMediaPreviewIds((existing) => {
                const next = { ...existing };
                delete next[attachmentId];
                return next;
              });
              setFailedMediaPreviewIds((existing) => ({
                ...existing,
                [attachmentId]: true,
              }));
              return;
            }

            setMessageMediaByAttachmentId((existing) => {
              const previous = existing[attachmentId];
              if (previous) {
                revokeObjectUrl(previous.url);
              }
              return {
                ...existing,
                [attachmentId]: {
                  url,
                  kind,
                  mimeType,
                },
              };
            });
            previewRetryAttempts.delete(attachmentId);
            setLoadingMediaPreviewIds((existing) => {
              const next = { ...existing };
              delete next[attachmentId];
              return next;
            });
          })
          .catch(() => {
            if (cancelled) {
              return;
            }
            const nextAttempt = nextMediaPreviewAttempt(
              previewRetryAttempts,
              attachmentId,
            );
            previewRetryAttempts.set(attachmentId, nextAttempt);
            if (shouldRetryMediaPreview(nextAttempt, maxRetries)) {
              window.setTimeout(() => {
                setMediaPreviewRetryTick((value) => value + 1);
              }, mediaPreviewRetryDelayMs(nextAttempt));
              return;
            }
            setLoadingMediaPreviewIds((existing) => {
              const next = { ...existing };
              delete next[attachmentId];
              return next;
            });
            setFailedMediaPreviewIds((existing) => ({
              ...existing,
              [attachmentId]: true,
            }));
          })
          .finally(() => {
            inflightMessageMediaLoads.delete(attachmentId);
          });

      if (attempt === 0) {
        window.setTimeout(() => {
          if (cancelled) {
            inflightMessageMediaLoads.delete(attachmentId);
            return;
          }
          void processFetch();
        }, initialDelayMs);
      } else {
        void processFetch();
      }
    }

    onCleanup(() => {
      cancelled = true;
    });
  });

  onCleanup(() => {
    for (const preview of Object.values(messageMediaByAttachmentId())) {
      revokeObjectUrl(preview.url);
    }
    setMessageMediaByAttachmentId({});
    setLoadingMediaPreviewIds({});
    setFailedMediaPreviewIds({});
  });

  const retryMediaPreview = (attachmentId: AttachmentId) => {
    previewRetryAttempts.delete(attachmentId);
    setFailedMediaPreviewIds((existing) => {
      const next = { ...existing };
      delete next[attachmentId];
      return next;
    });
    setMediaPreviewRetryTick((value) => value + 1);
  };

  return {
    messageMediaByAttachmentId,
    loadingMediaPreviewIds,
    failedMediaPreviewIds,
    retryMediaPreview,
  };
}
