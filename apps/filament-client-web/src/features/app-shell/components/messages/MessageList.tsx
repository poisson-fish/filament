import { For, Show } from "solid-js";
import type { MessageId, MessageRecord } from "../../../../domain/chat";
import { MessageRow, type MessageRowProps } from "./MessageRow";

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
}

export function MessageList(props: MessageListProps) {
  return (
    <section
      ref={props.onListRef}
      class="message-list"
      aria-live="polite"
      onScroll={props.onListScroll}
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

      <For each={props.messages}>
        {(message) => (
          <MessageRow
            message={message}
            currentUserId={props.currentUserId}
            canDeleteMessages={props.canDeleteMessages}
            displayUserLabel={props.displayUserLabel}
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
