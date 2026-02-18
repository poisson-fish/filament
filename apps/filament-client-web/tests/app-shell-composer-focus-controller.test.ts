import { createRoot, createSignal } from "solid-js";
import { describe, expect, it } from "vitest";
import {
  channelIdFromInput,
  channelNameFromInput,
  type ChannelRecord,
} from "../src/domain/chat";
import { createComposerFocusController } from "../src/features/app-shell/controllers/composer-focus-controller";

const ACTIVE_CHANNEL: ChannelRecord = {
  channelId: channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAB"),
  name: channelNameFromInput("general"),
  kind: "text",
};

describe("app shell composer focus controller", () => {
  it("focuses the composer and inserts first typed character", async () => {
    let dispose: (() => void) | undefined;
    const [composerValue, setComposerValue] = createSignal("hello");
    const composerInput = document.createElement("input");
    composerInput.value = composerValue();
    composerInput.addEventListener("input", () => {
      setComposerValue(composerInput.value);
    });
    document.body.append(composerInput);

    createRoot((rootDispose) => {
      dispose = rootDispose;
      const [activeChannel] = createSignal<ChannelRecord | null>(ACTIVE_CHANNEL);
      const [canAccessActiveChannel] = createSignal(true);
      createComposerFocusController({
        activeChannel,
        canAccessActiveChannel,
        composerInputElement: () => composerInput,
        composerValue,
      });
    });

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "a", bubbles: true }));
    await Promise.resolve();

    expect(composerInput.value).toBe("helloa");

    dispose?.();
    composerInput.remove();
  });

  it("does not redirect typing when already in an editable element", async () => {
    let dispose: (() => void) | undefined;
    const [composerValue, setComposerValue] = createSignal("root");
    const composerInput = document.createElement("input");
    composerInput.value = composerValue();
    composerInput.addEventListener("input", () => {
      setComposerValue(composerInput.value);
    });
    document.body.append(composerInput);

    const panelInput = document.createElement("input");
    panelInput.value = "panel";
    document.body.append(panelInput);
    panelInput.focus();

    createRoot((rootDispose) => {
      dispose = rootDispose;
      const [activeChannel] = createSignal<ChannelRecord | null>(ACTIVE_CHANNEL);
      const [canAccessActiveChannel] = createSignal(true);
      createComposerFocusController({
        activeChannel,
        canAccessActiveChannel,
        composerInputElement: () => composerInput,
        composerValue,
      });
    });

    panelInput.dispatchEvent(new KeyboardEvent("keydown", { key: "z", bubbles: true }));
    await Promise.resolve();

    expect(document.activeElement).toBe(panelInput);
    expect(composerInput.value).toBe("root");

    dispose?.();
    composerInput.remove();
    panelInput.remove();
  });

  it("does not redirect typing when channel is inaccessible or composer disabled", async () => {
    let dispose: (() => void) | undefined;
    const [composerValue] = createSignal("msg");
    const composerInput = document.createElement("input");
    composerInput.value = composerValue();
    composerInput.disabled = true;
    document.body.append(composerInput);

    createRoot((rootDispose) => {
      dispose = rootDispose;
      const [activeChannel] = createSignal<ChannelRecord | null>(ACTIVE_CHANNEL);
      const [canAccessActiveChannel] = createSignal(false);
      createComposerFocusController({
        activeChannel,
        canAccessActiveChannel,
        composerInputElement: () => composerInput,
        composerValue,
      });
    });

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "x", bubbles: true }));
    await Promise.resolve();

    expect(composerInput.value).toBe("msg");
    expect(document.activeElement).not.toBe(composerInput);

    dispose?.();
    composerInput.remove();
  });

  it("does not capture modifier shortcuts", async () => {
    let dispose: (() => void) | undefined;
    const [composerValue] = createSignal("test");
    const composerInput = document.createElement("input");
    composerInput.value = composerValue();
    document.body.append(composerInput);

    createRoot((rootDispose) => {
      dispose = rootDispose;
      const [activeChannel] = createSignal<ChannelRecord | null>(ACTIVE_CHANNEL);
      const [canAccessActiveChannel] = createSignal(true);
      createComposerFocusController({
        activeChannel,
        canAccessActiveChannel,
        composerInputElement: () => composerInput,
        composerValue,
      });
    });

    window.dispatchEvent(
      new KeyboardEvent("keydown", { key: "k", ctrlKey: true, bubbles: true }),
    );
    await Promise.resolve();

    expect(composerInput.value).toBe("test");
    expect(document.activeElement).not.toBe(composerInput);

    dispose?.();
    composerInput.remove();
  });

  it("submits composer form on global Enter when draft has text", async () => {
    let dispose: (() => void) | undefined;
    const [composerValue] = createSignal("hello world");
    const composerInput = document.createElement("input");
    composerInput.value = composerValue();
    const form = document.createElement("form");
    let submitCount = 0;
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      submitCount += 1;
    });
    form.append(composerInput);
    document.body.append(form);

    createRoot((rootDispose) => {
      dispose = rootDispose;
      const [activeChannel] = createSignal<ChannelRecord | null>(ACTIVE_CHANNEL);
      const [canAccessActiveChannel] = createSignal(true);
      createComposerFocusController({
        activeChannel,
        canAccessActiveChannel,
        composerInputElement: () => composerInput,
        composerValue,
      });
    });

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await Promise.resolve();

    expect(submitCount).toBe(1);

    dispose?.();
    form.remove();
  });

  it("does not submit on global Enter when draft is empty", async () => {
    let dispose: (() => void) | undefined;
    const [composerValue] = createSignal("   ");
    const composerInput = document.createElement("input");
    composerInput.value = composerValue();
    const form = document.createElement("form");
    let submitCount = 0;
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      submitCount += 1;
    });
    form.append(composerInput);
    document.body.append(form);

    createRoot((rootDispose) => {
      dispose = rootDispose;
      const [activeChannel] = createSignal<ChannelRecord | null>(ACTIVE_CHANNEL);
      const [canAccessActiveChannel] = createSignal(true);
      createComposerFocusController({
        activeChannel,
        canAccessActiveChannel,
        composerInputElement: () => composerInput,
        composerValue,
      });
    });

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Enter", bubbles: true }));
    await Promise.resolve();

    expect(submitCount).toBe(0);

    dispose?.();
    form.remove();
  });
});
