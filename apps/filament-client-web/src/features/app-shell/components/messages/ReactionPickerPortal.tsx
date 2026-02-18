import { For, Show } from "solid-js";
import { Portal } from "solid-js/web";
import type { MessageId, ReactionEmoji } from "../../../../domain/chat";
import type { ReactionPickerOption, ReactionPickerOverlayPosition } from "../../types";

export interface ReactionPickerPortalProps {
  openMessageId: MessageId | null;
  position: ReactionPickerOverlayPosition | null;
  options: ReactionPickerOption[];
  onClose: () => void;
  onAddReaction: (messageId: MessageId, emoji: ReactionEmoji) => Promise<void> | void;
}

export function ReactionPickerPortal(props: ReactionPickerPortalProps) {
  return (
    <Show when={props.openMessageId}>
      {(messageIdAccessor) => (
        <Show when={props.position}>
          {(positionAccessor) => (
            <Portal>
              <div
                class="reaction-picker-floating fx-panel fixed z-[1400] w-[min(23rem,calc(100vw-1rem))] rounded-[0.62rem] p-[0.5rem]"
                role="dialog"
                aria-label="Choose reaction"
                style={`top: ${positionAccessor().top}px; left: ${positionAccessor().left}px;`}
              >
                <div class="flex items-center justify-between gap-[0.5rem]">
                  <p class="m-0 text-[0.78rem] tracking-[0.08em] text-ink-1 uppercase">React</p>
                  <button
                    type="button"
                    class="inline-flex items-center rounded-[0.5rem] border border-line-soft bg-bg-3 px-[0.45rem] py-[0.16rem] text-[0.74rem] text-ink-1 transition-colors duration-[140ms] ease-out hover:bg-bg-4"
                    onClick={props.onClose}
                  >
                    Close
                  </button>
                </div>
                <div class="mt-[0.45rem] grid max-h-[11.5rem] grid-cols-[repeat(auto-fill,minmax(2rem,1fr))] gap-[0.3rem] overflow-auto">
                  <For each={props.options}>
                    {(option) => (
                      <button
                        type="button"
                        class="inline-flex h-[2rem] w-[2rem] items-center justify-center rounded-[0.42rem] border border-line-soft bg-bg-3 p-0 transition-colors duration-[140ms] ease-out hover:border-line hover:bg-bg-4"
                        onClick={() => void props.onAddReaction(messageIdAccessor(), option.emoji)}
                        aria-label={`Add ${option.label} reaction`}
                        title={option.label}
                      >
                        <img
                          class="block h-[1.3rem] w-[1.3rem]"
                          src={option.iconUrl}
                          alt=""
                          loading="lazy"
                          decoding="async"
                          referrerPolicy="no-referrer"
                        />
                      </button>
                    )}
                  </For>
                </div>
              </div>
            </Portal>
          )}
        </Show>
      )}
    </Show>
  );
}
