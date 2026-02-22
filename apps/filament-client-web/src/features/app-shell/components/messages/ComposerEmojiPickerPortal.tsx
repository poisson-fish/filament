import { Show, createEffect, onCleanup } from "solid-js";
import { Portal } from "solid-js/web";
import data from "@emoji-mart/data/sets/14/twitter.json";
import { Picker } from "emoji-mart";
import { computePosition, offset, shift, flip, autoUpdate } from "@floating-ui/dom";
import { emojiNativeFromSelection } from "./emoji-utils";

export interface ComposerEmojiPickerPortalProps {
  isOpen: boolean;
  onClose: () => void;
  onAddEmoji: (emojiNative: string) => void;
  anchorSelector: string;
}

export function ComposerEmojiPickerPortal(props: ComposerEmojiPickerPortalProps) {
  let floatingRef: HTMLDivElement | undefined;

  createEffect(() => {
    if (!props.isOpen || !floatingRef) {
      return;
    }

    floatingRef.innerHTML = "";

    const picker = new Picker({
      data,
      set: "twitter",
      theme: "auto",
      onEmojiSelect: (selection: unknown) => {
        const native = emojiNativeFromSelection(selection);
        if (!native) {
          return;
        }
        props.onAddEmoji(native);
      },
      onClickOutside: (event: unknown) => {
        const target = (event as { target?: unknown })?.target;
        if (!(target instanceof Node)) {
          props.onClose();
          return;
        }
        const anchor = document.querySelector(props.anchorSelector);
        if (anchor && anchor.contains(target)) {
          return;
        }
        props.onClose();
      },
    });

    floatingRef.appendChild(picker as unknown as HTMLElement);

    const anchor = document.querySelector(props.anchorSelector);
    const cleanupAutoUpdate =
      anchor instanceof HTMLElement
        ? autoUpdate(anchor, floatingRef, () => {
            computePosition(anchor, floatingRef!, {
              placement: "top-end",
              middleware: [offset(8), flip(), shift({ padding: 8 })],
            }).then(({ x, y }) => {
              if (floatingRef) {
                Object.assign(floatingRef.style, {
                  left: `${Math.max(0, x)}px`,
                  top: `${Math.max(0, y)}px`,
                });
              }
            });
          })
        : null;
    const onWindowKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        props.onClose();
      }
    };
    window.addEventListener("keydown", onWindowKeydown);

    onCleanup(() => {
      cleanupAutoUpdate?.();
      window.removeEventListener("keydown", onWindowKeydown);
      if (floatingRef) {
        floatingRef.innerHTML = "";
      }
    });
  });

  return (
    <Show when={props.isOpen}>
      <Portal>
        <div
          ref={floatingRef}
          class="composer-emoji-picker-floating fixed z-[1400] h-auto w-auto rounded-[0.62rem]"
          role="dialog"
          aria-label="Choose emoji"
          style="top: 0; left: 0;"
        />
      </Portal>
    </Show>
  );
}
