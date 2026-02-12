import { createRoot, createSignal, type Accessor, type Setter } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { messageIdFromInput, type MessageId } from "../src/domain/chat";
import {
  createReactionPickerController,
  resolveReactionPickerOverlayPosition,
} from "../src/features/app-shell/controllers/reaction-picker-controller";
import type { ReactionPickerOverlayPosition } from "../src/features/app-shell/types";

const MESSAGE_ID = messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAB");

function anchorRectFixture(): DOMRectReadOnly {
  return {
    x: 620,
    y: 120,
    width: 40,
    height: 40,
    top: 120,
    right: 660,
    bottom: 160,
    left: 620,
    toJSON: () => ({}),
  } as DOMRectReadOnly;
}

describe("app shell reaction picker controller", () => {
  it("resolves overlay placement inside viewport bounds", () => {
    const position = resolveReactionPickerOverlayPosition({
      anchorRect: anchorRectFixture(),
      viewportWidth: 800,
      viewportHeight: 600,
      overlayMaxWidthPx: 360,
      overlayMarginPx: 16,
      overlayEstimatedHeightPx: 220,
      overlayGapPx: 8,
    });

    expect(position).toEqual({
      top: 168,
      left: 300,
    });
  });

  it("tracks picker position and closes on escape/outside pointer events", async () => {
    let dispose = () => undefined;
    let openReactionPickerMessageId: Accessor<MessageId | null>;
    let setOpenReactionPickerMessageId: Setter<MessageId | null>;
    let position: Accessor<ReactionPickerOverlayPosition | null>;
    let setReactionPickerOverlayPosition: Setter<ReactionPickerOverlayPosition | null>;
    let anchor: HTMLElement;

    createRoot((rootDispose) => {
      dispose = rootDispose;
      [openReactionPickerMessageId, setOpenReactionPickerMessageId] =
        createSignal<MessageId | null>(MESSAGE_ID);
      [position, setReactionPickerOverlayPosition] =
        createSignal<ReactionPickerOverlayPosition | null>(null);

      anchor = document.createElement("button");
      anchor.setAttribute("data-reaction-anchor-for", MESSAGE_ID);
      anchor.getBoundingClientRect = () => anchorRectFixture();
      document.body.append(anchor);

      Object.defineProperty(window, "innerWidth", {
        configurable: true,
        writable: true,
        value: 900,
      });
      Object.defineProperty(window, "innerHeight", {
        configurable: true,
        writable: true,
        value: 700,
      });

      const controller = createReactionPickerController({
        openReactionPickerMessageId,
        setOpenReactionPickerMessageId,
        setReactionPickerOverlayPosition,
        trackPositionDependencies: vi.fn(),
        scheduleAfterPaint: (callback) => callback(),
      });
      controller.updateReactionPickerOverlayPosition(MESSAGE_ID);
    });

    await Promise.resolve();
    expect(position!()).toEqual({
      top: 168,
      left: 292,
    });

    window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
    expect(openReactionPickerMessageId!()).toBeNull();
    expect(position!()).toBeNull();

    setOpenReactionPickerMessageId!(MESSAGE_ID);
    await Promise.resolve();
    expect(openReactionPickerMessageId!()).toBe(MESSAGE_ID);

    const picker = document.createElement("div");
    picker.className = "reaction-picker-floating";
    const pickerInner = document.createElement("button");
    picker.append(pickerInner);
    document.body.append(picker);

    pickerInner.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    expect(openReactionPickerMessageId!()).toBe(MESSAGE_ID);

    anchor!.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    expect(openReactionPickerMessageId!()).toBe(MESSAGE_ID);

    document.body.dispatchEvent(new Event("pointerdown", { bubbles: true }));
    expect(openReactionPickerMessageId!()).toBeNull();

    dispose();
    anchor!.remove();
    picker.remove();
  });
});
