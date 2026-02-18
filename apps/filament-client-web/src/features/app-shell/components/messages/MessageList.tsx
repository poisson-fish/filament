import { For, Show, createEffect, createSignal } from "solid-js";
import type { MessageId, MessageRecord } from "../../../../domain/chat";
import { MessageRow, type MessageRowProps } from "./MessageRow";
import {
  isMessageListPinnedToLatest,
  resolveMessageListRenderWindow,
  type ResolveMessageListRenderWindowInput,
} from "./message-list-window";

export interface MessageListProps extends Omit<MessageRowProps, "message"> {
  messages: MessageRecord[];
  nextBefore: MessageId | null;
  showLoadOlderButton: boolean;
  isLoadingOlder: boolean;
  isLoadingMessages: boolean;
  messageError: string;
  onLoadOlderMessages: () => Promise<void> | void;
  onListScroll: () => void;
  onListRef: (element: HTMLElement) => void;
  maxRenderedMessages?: ResolveMessageListRenderWindowInput["maxRenderedMessages"];
  maxHistoricalRenderedMessages?: ResolveMessageListRenderWindowInput["maxHistoricalRenderedMessages"];
}

export function MessageList(props: MessageListProps) {
  const [isPinnedToLatest, setIsPinnedToLatest] = createSignal(true);
  let messageListElement: HTMLElement | undefined;

  const handleListRef = (element: HTMLElement) => {
    messageListElement = element;
    props.onListRef(element);
    setIsPinnedToLatest(isMessageListPinnedToLatest(element));
  };

  const handleListScroll = (event: Event) => {
    props.onListScroll();
    const element = event.currentTarget;
    if (!(element instanceof HTMLElement)) {
      return;
    }
    setIsPinnedToLatest(isMessageListPinnedToLatest(element));
  };

  createEffect(() => {
    void props.messages.length;
    const element = messageListElement;
    if (!element) {
      return;
    }
    setIsPinnedToLatest(isMessageListPinnedToLatest(element));
  });

  const renderWindow = () =>
    resolveMessageListRenderWindow({
      messageCount: props.messages.length,
      maxRenderedMessages: props.maxRenderedMessages,
      maxHistoricalRenderedMessages: props.maxHistoricalRenderedMessages,
      mode: isPinnedToLatest() ? "bounded" : "full",
    });

  const visibleMessages = () => {
    const window = renderWindow();
    return props.messages.slice(window.startIndex, window.endIndex);
  };

  return (
    <section
      ref={handleListRef}
      class="message-list flex min-h-0 flex-1 flex-col gap-0 overflow-x-hidden overflow-y-auto overscroll-contain bg-bg-1 px-[0.85rem] pt-[0.55rem] pb-[0.72rem] max-[900px]:px-[0.52rem] max-[900px]:pt-[0.5rem] max-[900px]:pb-[0.64rem]"
      aria-live="polite"
      onScroll={handleListScroll}
    >
      <Show when={!props.isLoadingMessages && props.messages.length === 0 && !props.messageError}>
        <p class="muted m-0 px-[0.46rem] py-[0.38rem] text-[0.82rem]">
          No messages yet in this channel.
        </p>
      </Show>

      <Show when={props.nextBefore && props.showLoadOlderButton}>
        <button
          type="button"
          class="my-[0.15rem] inline-flex w-fit items-center rounded-control border border-line-soft bg-bg-3 px-[0.66rem] py-[0.34rem] text-[0.78rem] font-semibold leading-[1.2] text-ink-1 transition-colors duration-[140ms] ease-out hover:bg-bg-4 disabled:cursor-default disabled:opacity-62"
          onClick={() => void props.onLoadOlderMessages()}
          disabled={props.isLoadingOlder}
        >
          {props.isLoadingOlder ? "Loading older..." : "Load older messages"}
        </button>
      </Show>

      <For each={visibleMessages()}>
        {(message) => (
          <MessageRow
            message={message}
            currentUserId={props.currentUserId}
            canDeleteMessages={props.canDeleteMessages}
            displayUserLabel={props.displayUserLabel}
            resolveAvatarUrl={props.resolveAvatarUrl}
            onOpenAuthorProfile={props.onOpenAuthorProfile}
            editingMessageId={props.editingMessageId}
            editingDraft={props.editingDraft}
            isSavingEdit={props.isSavingEdit}
            deletingMessageId={props.deletingMessageId}
            openReactionPickerMessageId={props.openReactionPickerMessageId}
            reactionState={props.reactionState}
            pendingReactionByKey={props.pendingReactionByKey}
            messageMediaByAttachmentId={props.messageMediaByAttachmentId}
            loadingMediaPreviewIds={props.loadingMediaPreviewIds}
            failedMediaPreviewIds={props.failedMediaPreviewIds}
            downloadingAttachmentId={props.downloadingAttachmentId}
            addReactionIconUrl={props.addReactionIconUrl}
            editMessageIconUrl={props.editMessageIconUrl}
            deleteMessageIconUrl={props.deleteMessageIconUrl}
            onEditingDraftInput={props.onEditingDraftInput}
            onSaveEditMessage={props.onSaveEditMessage}
            onCancelEditMessage={props.onCancelEditMessage}
            onDownloadAttachment={props.onDownloadAttachment}
            onRetryMediaPreview={props.onRetryMediaPreview}
            onToggleMessageReaction={props.onToggleMessageReaction}
            onToggleReactionPicker={props.onToggleReactionPicker}
            onBeginEditMessage={props.onBeginEditMessage}
            onRemoveMessage={props.onRemoveMessage}
          />
        )}
      </For>
    </section>
  );
}
