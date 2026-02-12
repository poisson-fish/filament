import { For, Show, createMemo } from "solid-js";
import type {
  AttachmentId,
  AttachmentRecord,
  MessageId,
  MessageRecord,
  ReactionEmoji,
  UserId,
} from "../../../../domain/chat";
import {
  actorAvatarGlyph,
  formatBytes,
  formatMessageTime,
  reactionViewsForMessage,
  tokenizeToDisplayText,
  type MessageMediaPreview,
  type ReactionView,
} from "../../helpers";

export interface MessageRowProps {
  message: MessageRecord;
  currentUserId: UserId | null;
  canDeleteMessages: boolean;
  displayUserLabel: (userId: string) => string;
  resolveAvatarUrl: (userId: string) => string | null;
  onOpenAuthorProfile: (userId: UserId) => void;
  editingMessageId: MessageId | null;
  editingDraft: string;
  isSavingEdit: boolean;
  deletingMessageId: MessageId | null;
  openReactionPickerMessageId: MessageId | null;
  reactionState: Record<string, ReactionView>;
  pendingReactionByKey: Record<string, true>;
  messageMediaByAttachmentId: Record<string, MessageMediaPreview>;
  loadingMediaPreviewIds: Record<string, true>;
  failedMediaPreviewIds: Record<string, true>;
  downloadingAttachmentId: AttachmentId | null;
  addReactionIconUrl: string;
  editMessageIconUrl: string;
  deleteMessageIconUrl: string;
  onEditingDraftInput: (value: string) => void;
  onSaveEditMessage: (messageId: MessageId) => Promise<void> | void;
  onCancelEditMessage: () => void;
  onDownloadAttachment: (record: AttachmentRecord) => Promise<void> | void;
  onRetryMediaPreview: (attachmentId: AttachmentId) => void;
  onToggleMessageReaction: (messageId: MessageId, emoji: ReactionEmoji) => Promise<void> | void;
  onToggleReactionPicker: (messageId: MessageId) => void;
  onBeginEditMessage: (message: MessageRecord) => void;
  onRemoveMessage: (messageId: MessageId) => Promise<void> | void;
}

export function MessageRow(props: MessageRowProps) {
  const isEditing = () => props.editingMessageId === props.message.messageId;
  const isDeleting = () => props.deletingMessageId === props.message.messageId;
  const isReactionPickerOpen = () => props.openReactionPickerMessageId === props.message.messageId;
  const canEditOrDelete =
    () => props.currentUserId === props.message.authorId || props.canDeleteMessages;
  const reactions = createMemo(() =>
    reactionViewsForMessage(
      props.message.messageId,
      props.reactionState,
      props.pendingReactionByKey,
    ),
  );
  const authorAvatarUrl = createMemo(() => props.resolveAvatarUrl(props.message.authorId));

  return (
    <article class="message-row">
      <button
        type="button"
        class="message-avatar-button"
        aria-label={`Open ${props.displayUserLabel(props.message.authorId)} profile`}
        onClick={() => props.onOpenAuthorProfile(props.message.authorId)}
      >
        <span class="message-avatar">
          <span class="message-avatar-fallback" aria-hidden="true">
            {actorAvatarGlyph(props.displayUserLabel(props.message.authorId))}
          </span>
          <Show when={authorAvatarUrl()}>
            <img
              class="message-avatar-image"
              src={authorAvatarUrl()!}
              alt={`${props.displayUserLabel(props.message.authorId)} avatar`}
              loading="lazy"
              decoding="async"
              referrerPolicy="no-referrer"
              onError={(event) => {
                event.currentTarget.style.display = "none";
              }}
            />
          </Show>
        </span>
      </button>
      <div class="message-main">
        <p class="message-meta">
          <strong>{props.displayUserLabel(props.message.authorId)}</strong>
          <span>{formatMessageTime(props.message.createdAtUnix)}</span>
        </p>
        <Show
          when={isEditing()}
          fallback={
            <Show when={tokenizeToDisplayText(props.message.markdownTokens) || props.message.content}>
              <p class="message-tokenized">
                {tokenizeToDisplayText(props.message.markdownTokens) || props.message.content}
              </p>
            </Show>
          }
        >
          <form
            class="inline-form message-edit"
            onSubmit={(event) => {
              event.preventDefault();
              void props.onSaveEditMessage(props.message.messageId);
            }}
          >
            <input
              value={props.editingDraft}
              onInput={(event) => props.onEditingDraftInput(event.currentTarget.value)}
              maxlength="2000"
            />
            <div class="message-actions">
              <button type="submit" disabled={props.isSavingEdit}>
                {props.isSavingEdit ? "Saving..." : "Save"}
              </button>
              <button type="button" onClick={props.onCancelEditMessage}>
                Cancel
              </button>
            </div>
          </form>
        </Show>
        <Show when={props.message.attachments.length > 0}>
          <div class="message-attachments">
            <For each={props.message.attachments}>
              {(record) => {
                const preview = () => props.messageMediaByAttachmentId[record.attachmentId];
                return (
                  <div class="message-attachment-card">
                    <Show
                      when={preview() && preview()!.kind === "image"}
                      fallback={
                        <Show
                          when={preview() && preview()!.kind === "video"}
                          fallback={
                            <Show
                              when={props.loadingMediaPreviewIds[record.attachmentId]}
                              fallback={
                                <Show
                                  when={props.failedMediaPreviewIds[record.attachmentId]}
                                  fallback={
                                    <button
                                      type="button"
                                      class="message-attachment-download"
                                      onClick={() => void props.onDownloadAttachment(record)}
                                      disabled={props.downloadingAttachmentId === record.attachmentId}
                                    >
                                      {props.downloadingAttachmentId === record.attachmentId
                                        ? "Fetching..."
                                        : `Download ${record.filename}`}
                                    </button>
                                  }
                                >
                                  <div class="message-attachment-failed">
                                    <span>Preview unavailable.</span>
                                    <button
                                      type="button"
                                      class="message-attachment-retry"
                                      onClick={() => props.onRetryMediaPreview(record.attachmentId)}
                                    >
                                      Retry preview
                                    </button>
                                  </div>
                                </Show>
                              }
                            >
                              <p class="message-attachment-loading">Loading preview...</p>
                            </Show>
                          }
                        >
                          <video
                            class="message-attachment-video"
                            src={preview()!.url}
                            controls
                            preload="metadata"
                            playsinline
                          />
                        </Show>
                      }
                    >
                      <img
                        class="message-attachment-image"
                        src={preview()!.url}
                        alt={record.filename}
                        loading="lazy"
                        decoding="async"
                        referrerPolicy="no-referrer"
                      />
                    </Show>
                    <p class="message-attachment-meta">
                      {record.filename} ({formatBytes(record.sizeBytes)})
                    </p>
                  </div>
                );
              }}
            </For>
          </div>
        </Show>
        <Show when={reactions().length > 0 || isReactionPickerOpen()}>
          <div class="reaction-row">
            <div class="reaction-controls">
              <div class="reaction-list">
                <For each={reactions()}>
                  {(reaction) => (
                    <button
                      type="button"
                      classList={{ "reaction-chip": true, reacted: reaction.reacted }}
                      onClick={() =>
                        void props.onToggleMessageReaction(props.message.messageId, reaction.emoji)}
                      disabled={reaction.pending}
                      aria-label={`${reaction.emoji} reaction (${reaction.count})`}
                    >
                      <span class="reaction-chip-emoji">{reaction.emoji}</span>
                      <span class="reaction-chip-count">{reaction.count}</span>
                    </button>
                  )}
                </For>
              </div>
            </div>
          </div>
        </Show>
      </div>
      <div class="message-hover-actions">
        <button
          type="button"
          class="icon-button"
          onClick={() => props.onToggleReactionPicker(props.message.messageId)}
          data-reaction-anchor-for={props.message.messageId}
          aria-label="Add reaction"
          title="Add reaction"
        >
          <span
            class="icon-mask"
            style={`--icon-url: url("${props.addReactionIconUrl}")`}
            aria-hidden="true"
          />
        </button>
        <Show when={canEditOrDelete()}>
          <>
            <button
              type="button"
              class="icon-button"
              onClick={() => props.onBeginEditMessage(props.message)}
              aria-label="Edit message"
              title="Edit message"
            >
              <span
                class="icon-mask"
                style={`--icon-url: url("${props.editMessageIconUrl}")`}
                aria-hidden="true"
              />
            </button>
            <button
              type="button"
              classList={{ "icon-button": true, "is-busy": isDeleting(), danger: true }}
              onClick={() => void props.onRemoveMessage(props.message.messageId)}
              disabled={isDeleting()}
              aria-label="Delete message"
              title={isDeleting() ? "Deleting message..." : "Delete message"}
            >
              <span
                class="icon-mask"
                style={`--icon-url: url("${props.deleteMessageIconUrl}")`}
                aria-hidden="true"
              />
            </button>
          </>
        </Show>
      </div>
    </article>
  );
}
