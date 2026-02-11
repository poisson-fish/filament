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
                class="reaction-picker reaction-picker-floating"
                role="dialog"
                aria-label="Choose reaction"
                style={`top: ${positionAccessor().top}px; left: ${positionAccessor().left}px;`}
              >
                <div class="reaction-picker-header">
                  <p class="reaction-picker-title">React</p>
                  <button
                    type="button"
                    class="reaction-picker-close"
                    onClick={props.onClose}
                  >
                    Close
                  </button>
                </div>
                <div class="reaction-picker-grid">
                  <For each={props.options}>
                    {(option) => (
                      <button
                        type="button"
                        class="reaction-picker-option"
                        onClick={() => void props.onAddReaction(messageIdAccessor(), option.emoji)}
                        aria-label={`Add ${option.label} reaction`}
                        title={option.label}
                      >
                        <img
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
