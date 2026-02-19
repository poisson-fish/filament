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
  const controlButtonClass =
    "inline-flex min-h-[2.58rem] items-center justify-center border-0 bg-transparent px-[0.82rem] text-ink-1 transition-colors duration-[140ms] ease-out hover:bg-bg-3 disabled:cursor-not-allowed disabled:opacity-68";

  return (
    <form
      class="composer grid grid-cols-[minmax(0,1fr)] items-stretch justify-items-stretch gap-[0.52rem] border-t border-line-soft bg-bg-2 px-[0.9rem] pt-[0.68rem] pb-[0.86rem] max-[900px]:p-[0.52rem]"
      onSubmit={props.onSubmit}
    >
      <input
        ref={props.attachmentInputRef}
        type="file"
        multiple
        class="composer-file-input hidden"
        onInput={props.onAttachmentInput}
      />
      <div class="grid w-full min-w-0 grid-cols-[auto_minmax(0,1fr)_auto] items-stretch overflow-hidden rounded-[0.62rem] border border-line-soft bg-bg-2 focus-within:border-line focus-within:shadow-[0_0_0_1px_var(--line)]">
        <button
          type="button"
          class={`${controlButtonClass} border-r border-line-soft text-[1.14rem] font-[700]`}
          onClick={props.onOpenAttachmentPicker}
          disabled={isDisabled()}
          aria-label="Attach files"
          title="Attach files"
        >
          +
        </button>
        <input
          ref={props.composerInputRef}
          class="w-full min-w-0 border-0 bg-transparent px-[0.78rem] text-ink-0 outline-none placeholder:text-ink-2 disabled:cursor-not-allowed disabled:opacity-68"
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
          class={`${controlButtonClass} min-w-[4.2rem] border-l border-line-soft text-[0.9rem] font-[760] leading-none tracking-[0.01em]`}
          disabled={isDisabled()}
        >
          {props.isSendingMessage ? "Sending..." : "Send"}
        </button>
      </div>
      <Show when={props.composerAttachments.length > 0}>
        <div class="flex flex-wrap gap-[0.4rem]">
          <For each={props.composerAttachments}>
            {(file) => (
              <button
                type="button"
                class="inline-flex items-center rounded-[999px] border border-line-soft bg-bg-2 px-[0.62rem] py-[0.25rem] text-[0.78rem] text-ink-1 transition-colors duration-[140ms] ease-out hover:bg-bg-3"
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
