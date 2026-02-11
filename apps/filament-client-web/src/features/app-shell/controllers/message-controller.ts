import {
  createEffect,
  createSignal,
  onCleanup,
  untrack,
  type Accessor,
} from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import type {
  AttachmentId,
  AttachmentRecord,
  ChannelId,
  GuildId,
  MessageRecord,
} from "../../../domain/chat";
import {
  ApiError,
  downloadChannelAttachmentPreview,
  refreshAuthSession,
} from "../../../lib/api";
import { resolveAttachmentPreviewType } from "../helpers";
import {
  createObjectUrl,
  revokeObjectUrl,
  type MessageMediaPreview,
} from "../helpers";

const DEFAULT_MAX_PREVIEW_BYTES = 25 * 1024 * 1024;
const DEFAULT_MAX_MEDIA_PREVIEW_RETRIES = 2;
const DEFAULT_INITIAL_MEDIA_PREVIEW_DELAY_MS = 75;

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
  baseDelayMs = 600,
): number {
  return baseDelayMs * Math.max(attempt, 1);
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
