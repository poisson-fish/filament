import { For, Show, createSignal } from "solid-js";
import type { JSX } from "solid-js";
import type { ChannelRecord } from "../../../../domain/chat";
import { channelRailLabel, formatBytes } from "../../helpers";
import {
  initEmojiMart,
  replaceEmojiShortcodesWithSelection,
  renderEmojiMixedText,
} from "./emoji-utils";
import { ComposerEmojiPickerPortal } from "./ComposerEmojiPickerPortal";

const ATTACH_ICON_URL = new URL(
  "../../../../../resource/coolicons.v4.1/cooliocns SVG/Edit/Add_Plus.svg",
  import.meta.url,
).href;
const TEXT_CHANNEL_ICON_URL = new URL(
  "../../../../../resource/coolicons.v4.1/cooliocns SVG/Communication/Chat.svg",
  import.meta.url,
).href;
const VOICE_CHANNEL_ICON_URL = new URL(
  "../../../../../resource/coolicons.v4.1/cooliocns SVG/User/User_Voice.svg",
  import.meta.url,
).href;
const GIFT_ICON_URL = new URL(
  "../../../../../resource/coolicons.v4.1/cooliocns SVG/Interface/Gift.svg",
  import.meta.url,
).href;
const EMOJI_ICON_URL = new URL(
  "../../../../../resource/coolicons.v4.1/cooliocns SVG/Interface/Star.svg",
  import.meta.url,
).href;

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
  initEmojiMart();

  const isDisabled = () =>
    !props.activeChannel || props.isSendingMessage || !props.canAccessActiveChannel;
  const activeChannelIconUrl = () =>
    props.activeChannel?.kind === "voice" ? VOICE_CHANNEL_ICON_URL : TEXT_CHANNEL_ICON_URL;
  const controlButtonClass =
    "inline-flex min-h-[2.7rem] items-center justify-center border-0 bg-transparent px-[0.78rem] text-ink-2 transition-colors duration-[140ms] ease-out enabled:hover:bg-bg-3 enabled:hover:text-ink-0 disabled:cursor-not-allowed disabled:opacity-68";
  const utilityButtonClass =
    "inline-flex h-[2.12rem] items-center justify-center rounded-[0.48rem] border-0 bg-transparent px-[0.5rem] text-ink-2 transition-colors duration-[140ms] ease-out enabled:hover:bg-bg-3 enabled:hover:text-ink-0 disabled:cursor-not-allowed disabled:opacity-68";

  const [isEmojiPickerOpen, setEmojiPickerOpen] = createSignal(false);
  let inputEl: HTMLInputElement | undefined;
  let ghostEl: HTMLDivElement | undefined;

  const handleEmojiAdd = (emojiNative: string) => {
    setEmojiPickerOpen(false);

    if (!inputEl) {
      props.onComposerInput(props.composerValue + emojiNative);
      return;
    }
    const start = inputEl.selectionStart ?? props.composerValue.length;
    const end = inputEl.selectionEnd ?? props.composerValue.length;
    const val = props.composerValue;
    props.onComposerInput(val.slice(0, start) + emojiNative + val.slice(end));

    // Attempt cursor restore
    inputEl.focus();
    setTimeout(() => {
      if (inputEl) {
        inputEl.selectionStart = inputEl.selectionEnd = start + emojiNative.length;
      }
    }, 0);
  };

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
      <div class="grid w-full min-w-0 grid-cols-[auto_auto_minmax(0,1fr)_auto] items-center overflow-hidden rounded-[0.82rem] border border-line-soft bg-bg-1 focus-within:border-line focus-within:shadow-[0_0_0_1px_var(--line)]">
        <button
          type="button"
          class={`${controlButtonClass} border-r border-line-soft`}
          onClick={props.onOpenAttachmentPicker}
          disabled={isDisabled()}
          aria-label="Attach files"
          title="Attach files"
        >
          <span
            class="icon-mask h-[1.06rem] w-[1.06rem]"
            style={`--icon-url: url("${ATTACH_ICON_URL}")`}
            aria-hidden="true"
          />
        </button>
        <span
          class="icon-mask mx-[0.62rem] h-[0.98rem] w-[0.98rem] shrink-0 text-ink-2"
          style={`--icon-url: url("${activeChannelIconUrl()}")`}
          aria-hidden="true"
        />
        <div
          class="relative grid w-full min-w-0 items-center"
          style="grid-template-columns: minmax(0,1fr);"
        >
          <div
            ref={ghostEl}
            class="pointer-events-none col-start-1 row-start-1 flex h-full w-full min-w-0 items-center overflow-hidden whitespace-pre border-0 bg-transparent pl-0 pr-[0.64rem] text-[0.94rem] text-ink-0 disabled:opacity-68"
            aria-hidden="true"
          >
            {props.composerValue ? (
              renderEmojiMixedText(props.composerValue)
            ) : (
              <span class="text-ink-2">
                {props.activeChannel
                  ? `Message ${channelRailLabel({ kind: props.activeChannel.kind, name: props.activeChannel.name })}`
                  : "Select channel"}
              </span>
            )}
          </div>
          <input
            ref={(el) => {
              inputEl = el;
              props.composerInputRef(el);
            }}
            class="col-start-1 row-start-1 w-full min-w-0 border-0 bg-transparent pl-0 pr-[0.64rem] text-[0.94rem] text-transparent caret-ink-0 outline-none placeholder:text-transparent disabled:cursor-not-allowed disabled:opacity-68"
            value={props.composerValue}
            onInput={(event) => {
              const rawValue = event.currentTarget.value;
              const replacement = replaceEmojiShortcodesWithSelection(
                rawValue,
                event.currentTarget.selectionStart,
                event.currentTarget.selectionEnd,
              );
              if (replacement.text !== rawValue) {
                event.currentTarget.value = replacement.text;
                if (
                  replacement.selectionStart !== null &&
                  replacement.selectionEnd !== null
                ) {
                  event.currentTarget.selectionStart = replacement.selectionStart;
                  event.currentTarget.selectionEnd = replacement.selectionEnd;
                }
              }
              props.onComposerInput(replacement.text);
            }}
            onScroll={(event) => {
              if (ghostEl) {
                ghostEl.scrollLeft = event.currentTarget.scrollLeft;
              }
            }}
            maxlength="2000"
            placeholder={
              props.activeChannel
                ? `Message ${channelRailLabel({ kind: props.activeChannel.kind, name: props.activeChannel.name })}`
                : "Select channel"
            }
            disabled={isDisabled()}
          />
        </div>
        <div class="inline-flex items-center gap-[0.1rem] border-l border-line-soft px-[0.34rem]">
          <button
            type="button"
            class={utilityButtonClass}
            aria-label="Open gift picker"
            title="Gift"
            disabled={isDisabled()}
            onClick={(event) => event.preventDefault()}
          >
            <span
              class="icon-mask h-[1rem] w-[1rem]"
              style={`--icon-url: url("${GIFT_ICON_URL}")`}
              aria-hidden="true"
            />
          </button>
          <button
            type="button"
            class={`${utilityButtonClass} min-w-[2.2rem] px-[0.48rem] text-[0.66rem] font-[760] leading-none tracking-[0.04em]`}
            aria-label="Open GIF picker"
            title="GIF"
            disabled={isDisabled()}
            onClick={(event) => event.preventDefault()}
          >
            GIF
          </button>
          <button
            id="composer-emoji-button"
            type="button"
            class={utilityButtonClass}
            aria-label="Open emoji picker"
            title="Emoji"
            disabled={isDisabled()}
            onClick={(event) => {
              event.preventDefault();
              setEmojiPickerOpen((prev) => !prev);
            }}
          >
            <span
              class="icon-mask h-[0.96rem] w-[0.96rem]"
              style={`--icon-url: url("${EMOJI_ICON_URL}")`}
              aria-hidden="true"
            />
          </button>
          <button
            type="submit"
            class="h-0 w-0 overflow-hidden border-0 p-0 opacity-0"
            tabIndex={-1}
            aria-label={props.isSendingMessage ? "Sending..." : "Send message"}
            disabled={isDisabled()}
          >
            {props.isSendingMessage ? "Sending..." : "Send"}
          </button>
        </div>
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
      <ComposerEmojiPickerPortal
        isOpen={isEmojiPickerOpen()}
        onClose={() => setEmojiPickerOpen(false)}
        onAddEmoji={handleEmojiAdd}
        anchorSelector="#composer-emoji-button"
      />
    </form>
  );
}
