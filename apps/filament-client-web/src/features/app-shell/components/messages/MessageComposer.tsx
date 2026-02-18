import { For, Show } from "solid-js";
import type { JSX } from "solid-js";
import type { ChannelRecord } from "../../../../domain/chat";
import { channelRailLabel, formatBytes } from "../../helpers";

export interface MessageComposerProps {
  activeChannel: ChannelRecord | null;
  canAccessActiveChannel: boolean;
  isSendingMessage: boolean;
  composerValue: string;
  composerAttachments: File[];
  onSubmit: (event: SubmitEvent) => Promise<void> | void;
  onComposerInput: (value: string) => void;
  onOpenAttachmentPicker: () => void;
  onAttachmentInput: JSX.EventHandler<HTMLInputElement, InputEvent>;
  onRemoveAttachment: (file: File) => void;
  attachmentInputRef: (element: HTMLInputElement) => void;
  composerInputRef: (element: HTMLInputElement) => void;
}

export function MessageComposer(props: MessageComposerProps) {
  const isDisabled = () =>
    !props.activeChannel || props.isSendingMessage || !props.canAccessActiveChannel;

  return (
    <form class="composer" onSubmit={props.onSubmit}>
      <input
        ref={props.attachmentInputRef}
        type="file"
        multiple
        class="composer-file-input"
        onInput={props.onAttachmentInput}
      />
      <div class="composer-input-shell">
        <button
          type="button"
          class="composer-attach-button"
          onClick={props.onOpenAttachmentPicker}
          disabled={isDisabled()}
          aria-label="Attach files"
          title="Attach files"
        >
          +
        </button>
        <input
          ref={props.composerInputRef}
          class="composer-text-input"
          value={props.composerValue}
          onInput={(event) => props.onComposerInput(event.currentTarget.value)}
          maxlength="2000"
          placeholder={
            props.activeChannel
              ? `Message ${channelRailLabel({ kind: props.activeChannel.kind, name: props.activeChannel.name })}`
              : "Select channel"
          }
          disabled={isDisabled()}
        />
        <button
          type="submit"
          class="composer-send-button"
          disabled={isDisabled()}
        >
          {props.isSendingMessage ? "Sending..." : "Send"}
        </button>
      </div>
      <Show when={props.composerAttachments.length > 0}>
        <div class="composer-attachments">
          <For each={props.composerAttachments}>
            {(file) => (
              <button
                type="button"
                class="composer-attachment-pill"
                onClick={() => props.onRemoveAttachment(file)}
                title={`Remove ${file.name}`}
              >
                {file.name} ({formatBytes(file.size)}) x
              </button>
            )}
          </For>
        </div>
      </Show>
    </form>
  );
}
