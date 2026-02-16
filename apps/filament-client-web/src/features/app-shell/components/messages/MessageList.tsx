import { For, Show, createSignal } from "solid-js";
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
}

export function MessageList(props: MessageListProps) {
  const [isPinnedToLatest, setIsPinnedToLatest] = createSignal(true);

  const handleListRef = (element: HTMLElement) => {
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

  const renderWindow = () =>
    resolveMessageListRenderWindow({
      messageCount: props.messages.length,
      maxRenderedMessages: props.maxRenderedMessages,
      mode: isPinnedToLatest() ? "bounded" : "full",
    });

  const visibleMessages = () => {
    const window = renderWindow();
    return props.messages.slice(window.startIndex, window.endIndex);
  };

  return (
    <section
      ref={handleListRef}
      class="message-list"
      aria-live="polite"
      onScroll={handleListScroll}
    >
      <Show when={props.nextBefore && props.showLoadOlderButton}>
        <button
          type="button"
          class="load-older"
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

      <Show when={!props.isLoadingMessages && props.messages.length === 0 && !props.messageError}>
        <p class="muted">No messages yet in this channel.</p>
      </Show>
    </section>
  );
}
