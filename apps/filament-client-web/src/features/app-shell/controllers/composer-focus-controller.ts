import { createEffect, onCleanup, type Accessor } from "solid-js";
import type { ChannelRecord } from "../../../domain/chat";

export interface ComposerFocusControllerOptions {
  activeChannel: Accessor<ChannelRecord | null>;
  canAccessActiveChannel: Accessor<boolean>;
  composerInputElement: Accessor<HTMLInputElement | undefined>;
  composerValue: Accessor<string>;
}

function asElement(target: EventTarget | null): Element | null {
  return target instanceof Element ? target : null;
}

function isEditableElement(element: Element | null): boolean {
  if (!element) {
    return false;
  }
  if (element instanceof HTMLInputElement) {
    return true;
  }
  if (element instanceof HTMLTextAreaElement) {
    return true;
  }
  if (element instanceof HTMLSelectElement) {
    return true;
  }
  if (element instanceof HTMLElement && element.isContentEditable) {
    return true;
  }
  return false;
}

function isInteractiveControl(element: Element | null): boolean {
  if (!element) {
    return false;
  }
  if (element.closest("button, a, summary")) {
    return true;
  }
  if (element instanceof HTMLElement && element.tabIndex >= 0) {
    return true;
  }
  const role = element.getAttribute("role");
  return (
    role === "button" ||
    role === "link" ||
    role === "menuitem" ||
    role === "switch" ||
    role === "checkbox" ||
    role === "radio" ||
    role === "tab"
  );
}

function isPrintableTypingKey(event: KeyboardEvent): boolean {
  if (event.isComposing || event.defaultPrevented) {
    return false;
  }
  if (event.ctrlKey || event.metaKey || event.altKey) {
    return false;
  }
  return event.key.length === 1;
}

function shouldSubmitFromGlobalEnter(event: KeyboardEvent): boolean {
  if (event.defaultPrevented || event.isComposing) {
    return false;
  }
  if (event.key !== "Enter") {
    return false;
  }
  if (event.ctrlKey || event.metaKey || event.altKey || event.shiftKey) {
    return false;
  }
  return true;
}

function insertCharacterAndDispatchInput(
  input: HTMLInputElement,
  character: string,
): void {
  const selectionStart = input.selectionStart ?? input.value.length;
  const selectionEnd = input.selectionEnd ?? input.value.length;
  input.setRangeText(character, selectionStart, selectionEnd, "end");
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

export function createComposerFocusController(
  options: ComposerFocusControllerOptions,
): void {
  createEffect(() => {
    const onWindowKeydown = (event: KeyboardEvent) => {
      if (!options.activeChannel() || !options.canAccessActiveChannel()) {
        return;
      }
      const eventTarget = asElement(event.target);
      const activeElement = asElement(document.activeElement);
      if (isEditableElement(eventTarget) || isEditableElement(activeElement)) {
        return;
      }
      if (isInteractiveControl(eventTarget)) {
        return;
      }

      const composerInput = options.composerInputElement();
      if (!composerInput || composerInput.disabled || composerInput.readOnly) {
        return;
      }

      if (shouldSubmitFromGlobalEnter(event)) {
        if (options.composerValue().trim().length === 0) {
          return;
        }
        const composerForm = composerInput.form;
        if (!composerForm) {
          return;
        }
        event.preventDefault();
        composerForm.requestSubmit();
        return;
      }
      if (!isPrintableTypingKey(event)) {
        return;
      }

      event.preventDefault();
      composerInput.focus({ preventScroll: true });
      insertCharacterAndDispatchInput(composerInput, event.key);
    };

    window.addEventListener("keydown", onWindowKeydown);
    onCleanup(() => {
      window.removeEventListener("keydown", onWindowKeydown);
    });
  });
}
