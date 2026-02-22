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
import { initEmojiMart, renderEmojiMixedText } from "./emoji-utils";

initEmojiMart();

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
  const editActionButtonClass =
    "inline-flex items-center rounded-[0.55rem] border border-line-soft bg-bg-3 px-[0.55rem] py-[0.25rem] text-[0.82rem] text-ink-1 transition-colors duration-[140ms] ease-out hover:bg-bg-4 disabled:cursor-default disabled:opacity-66";
  const hoverActionButtonClass =
    "icon-button inline-flex min-h-[1.9rem] w-[1.9rem] items-center justify-center rounded-[0.4rem] border-0 bg-transparent p-0 text-ink-2 transition-colors duration-[120ms] ease-out hover:bg-bg-4 hover:text-ink-0 disabled:cursor-default disabled:opacity-58";
  const reactionChipClass = (reacted: boolean) =>
    reacted
      ? "reaction-chip inline-flex items-center gap-[0.3rem] rounded-[999px] border border-line bg-bg-4 px-[0.5rem] py-[0.18rem] text-ink-0 transition-colors duration-[140ms] ease-out disabled:cursor-default disabled:opacity-66"
      : "reaction-chip inline-flex items-center gap-[0.3rem] rounded-[999px] border border-line-soft bg-bg-3 px-[0.5rem] py-[0.18rem] text-ink-1 transition-colors duration-[140ms] ease-out hover:bg-bg-4 disabled:cursor-default disabled:opacity-66";
  const reactions = createMemo(() =>
    reactionViewsForMessage(
      props.message.messageId,
      props.reactionState,
      props.pendingReactionByKey,
    ),
  );
  const authorAvatarUrl = createMemo(() => props.resolveAvatarUrl(props.message.authorId));

  return (
    <article class="message-row group relative mt-[0.02rem] grid grid-cols-[2.35rem_minmax(0,1fr)] gap-[0.65rem] rounded-[0.45rem] border-0 bg-transparent px-[0.46rem] py-[0.34rem] transition-colors duration-[120ms] ease-out hover:bg-bg-3 focus-within:bg-bg-3 [&:first-of-type]:mt-auto">
      <button
        type="button"
        class="mt-[0.08rem] w-[2.1rem] cursor-pointer rounded-[999px] border-0 bg-transparent p-0"
        aria-label={`Open ${props.displayUserLabel(props.message.authorId)} profile`}
        onClick={() => props.onOpenAuthorProfile(props.message.authorId)}
      >
        <span class="relative mt-[0.08rem] grid h-[2.1rem] w-[2.1rem] place-items-center overflow-hidden rounded-[999px] border border-line-soft bg-bg-3 text-[0.74rem] font-[800] tracking-[0.03em] text-ink-0">
          <span class="z-[1]" aria-hidden="true">
            {actorAvatarGlyph(props.displayUserLabel(props.message.authorId))}
          </span>
          <Show when={authorAvatarUrl()}>
            <img
              class="absolute inset-0 z-[2] h-full w-full rounded-[inherit] object-cover"
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
      <div class="min-w-0 pr-[7rem] [@media(hover:none)]:pr-[4.5rem]">
        <p class="m-0 flex flex-wrap items-baseline gap-[0.52rem]">
          <strong class="m-0 text-[0.96rem] font-[790] text-ink-0">
            {props.displayUserLabel(props.message.authorId)}
          </strong>
          <span class="text-[0.74rem] text-ink-2">
            {formatMessageTime(props.message.createdAtUnix)}
          </span>
        </p>
        <Show
          when={isEditing()}
          fallback={
            <Show when={tokenizeToDisplayText(props.message.markdownTokens) || props.message.content}>
              <p class="message-tokenized mt-[0.08rem] whitespace-pre-wrap break-words leading-[1.38] text-ink-1">
                {renderEmojiMixedText(
                  tokenizeToDisplayText(props.message.markdownTokens) || props.message.content
                )}
              </p>
            </Show>
          }
        >
          <form
            class="mt-[0.26rem] grid gap-[0.42rem]"
            onSubmit={(event) => {
              event.preventDefault();
              void props.onSaveEditMessage(props.message.messageId);
            }}
          >
            <input
              class="w-full min-w-0 rounded-[0.56rem] border border-line-soft bg-bg-3 px-[0.68rem] py-[0.4rem] text-ink-0 outline-none transition-colors duration-[140ms] ease-out placeholder:text-ink-2 focus:border-line"
              value={props.editingDraft}
              onInput={(event) => props.onEditingDraftInput(event.currentTarget.value)}
              maxlength="2000"
            />
            <div class="flex flex-wrap gap-[0.42rem]">
              <button type="submit" class={editActionButtonClass} disabled={props.isSavingEdit}>
                {props.isSavingEdit ? "Saving..." : "Save"}
              </button>
              <button type="button" class={editActionButtonClass} onClick={props.onCancelEditMessage}>
                Cancel
              </button>
            </div>
          </form>
        </Show>
        <Show when={props.message.attachments.length > 0}>
          <div class="mt-[0.52rem] grid gap-[0.55rem]">
            <For each={props.message.attachments}>
              {(record) => {
                const preview = () => props.messageMediaByAttachmentId[record.attachmentId];
                return (
                  <div class="grid gap-[0.42rem] rounded-[0.62rem] border border-line-soft bg-bg-2 p-[0.45rem]">
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
                                      class="inline-flex w-fit items-center rounded-[0.56rem] border border-brand bg-bg-3 px-[0.6rem] py-[0.36rem] text-[0.82rem] text-ink-0 transition-colors duration-[140ms] ease-out hover:bg-bg-4 disabled:cursor-default disabled:opacity-66"
                                      onClick={() => void props.onDownloadAttachment(record)}
                                      disabled={props.downloadingAttachmentId === record.attachmentId}
                                    >
                                      {props.downloadingAttachmentId === record.attachmentId
                                        ? "Fetching..."
                                        : `Download ${record.filename}`}
                                    </button>
                                  }
                                >
                                  <div class="flex items-center gap-[0.5rem] text-[0.8rem] text-danger">
                                    <span>Preview unavailable.</span>
                                    <button
                                      type="button"
                                      class="inline-flex items-center rounded-[0.5rem] border border-brand bg-bg-3 px-[0.5rem] py-[0.2rem] text-[0.78rem] text-ink-0 transition-colors duration-[140ms] ease-out hover:bg-bg-4"
                                      onClick={() => props.onRetryMediaPreview(record.attachmentId)}
                                    >
                                      Retry preview
                                    </button>
                                  </div>
                                </Show>
                              }
                            >
                              <p class="m-0 text-[0.82rem] text-ink-1">Loading preview...</p>
                            </Show>
                          }
                        >
                          <video
                            class="block max-h-[22rem] w-full max-w-[min(100%,33rem)] rounded-[0.5rem] border border-line-soft bg-bg-0 object-contain"
                            src={preview()!.url}
                            controls
                            preload="metadata"
                            playsinline
                          />
                        </Show>
                      }
                    >
                      <img
                        class="block max-h-[22rem] w-full max-w-[min(100%,33rem)] rounded-[0.5rem] border border-line-soft bg-bg-0 object-contain"
                        src={preview()!.url}
                        alt={record.filename}
                        loading="lazy"
                        decoding="async"
                        referrerPolicy="no-referrer"
                      />
                    </Show>
                    <p class="m-0 text-[0.78rem] text-ink-2">
                      {record.filename} ({formatBytes(record.sizeBytes)})
                    </p>
                  </div>
                );
              }}
            </For>
          </div>
        </Show>
        <Show when={reactions().length > 0 || isReactionPickerOpen()}>
          <div class="mt-[0.4rem] grid gap-[0.4rem]">
            <div class="flex flex-wrap items-center gap-[0.35rem]">
              <div class="flex flex-wrap gap-[0.35rem]">
                <For each={reactions()}>
                  {(reaction) => (
                    <button
                      type="button"
                      class={reactionChipClass(reaction.reacted)}
                      onClick={() =>
                        void props.onToggleMessageReaction(props.message.messageId, reaction.emoji)}
                      disabled={reaction.pending}
                      aria-label={`${reaction.emoji} reaction (${reaction.count})`}
                    >
                      <span class="inline-flex items-center justify-center text-[1.05rem] text-inherit leading-none">
                        {reaction.emoji}
                      </span>
                      <span class="text-[0.78rem] font-[700] text-inherit">{reaction.count}</span>
                    </button>
                  )}
                </For>
              </div>
            </div>
          </div>
        </Show>
      </div>
      <div class="message-hover-actions pointer-events-none absolute top-[-0.54rem] right-[0.58rem] z-[2] inline-flex translate-y-[0.2rem] items-center gap-[0.1rem] rounded-[0.52rem] border border-line-soft bg-bg-2 p-[0.14rem] opacity-0 shadow-panel transition duration-[120ms] ease-out group-hover:pointer-events-auto group-hover:translate-y-0 group-hover:opacity-100 group-focus-within:pointer-events-auto group-focus-within:translate-y-0 group-focus-within:opacity-100 focus-within:pointer-events-auto focus-within:translate-y-0 focus-within:opacity-100">
        <button
          type="button"
          class={hoverActionButtonClass}
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
              class={hoverActionButtonClass}
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
              class={`${hoverActionButtonClass} text-danger hover:bg-danger-panel hover:text-danger-ink`}
              onClick={() => void props.onRemoveMessage(props.message.messageId)}
              disabled={isDeleting()}
              aria-label="Delete message"
              title={isDeleting() ? "Deleting message..." : "Delete message"}
            >
              <span
                classList={{ "icon-mask": true, "animate-pulse": isDeleting() }}
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
