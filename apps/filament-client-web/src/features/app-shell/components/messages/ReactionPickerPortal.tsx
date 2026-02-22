import { Show, createEffect, onCleanup } from "solid-js";
import { Portal } from "solid-js/web";
import data from "@emoji-mart/data";
import { Picker } from "emoji-mart";
import { computePosition, offset, shift, flip, autoUpdate } from "@floating-ui/dom";
import type { MessageId, ReactionEmoji } from "../../../../domain/chat";
import { reactionEmojiFromInput } from "../../../../domain/chat";

export interface ReactionPickerPortalProps {
  openMessageId: MessageId | null;
  onClose: () => void;
  onAddReaction: (messageId: MessageId, emoji: ReactionEmoji) => Promise<void> | void;
}

export function ReactionPickerPortal(props: ReactionPickerPortalProps) {
  let floatingRef: HTMLDivElement | undefined;

  createEffect(() => {
    const messageId = props.openMessageId;
    if (!messageId || !floatingRef) {
      return;
    }

    // Clear previous picker contents just in case
    floatingRef.innerHTML = "";

    const picker = new Picker({
      data,
      set: "twitter",
      theme: "auto",
      onEmojiSelect: (emoji: any) => {
        void props.onAddReaction(messageId, reactionEmojiFromInput(emoji.native));
      },
      onClickOutside: (e: any) => {
        // handle click outside if we want, but controller already does it
      }
    });
    floatingRef.appendChild(picker as unknown as HTMLElement);

    const anchor = document.querySelector(`[data-reaction-anchor-for="${messageId}"]`);
    if (anchor) {
      const cleanup = autoUpdate(anchor, floatingRef, () => {
        computePosition(anchor, floatingRef!, {
          placement: "bottom-end",
          middleware: [offset(4), flip(), shift({ padding: 8 })],
        }).then(({ x, y }) => {
          if (floatingRef) {
            Object.assign(floatingRef.style, {
              left: `${Math.max(0, x)}px`,
              top: `${Math.max(0, y)}px`,
            });
          }
        });
      });
      onCleanup(() => {
        cleanup();
      });
    }
  });

  return (
    <Show when={props.openMessageId}>
      <Portal>
        <div
          ref={floatingRef}
          class="reaction-picker-floating fixed z-[1400] w-auto h-auto rounded-[0.62rem]"
          role="dialog"
          aria-label="Choose reaction"
          style="top: 0; left: 0;"
        />
      </Portal>
    </Show>
  );
}
