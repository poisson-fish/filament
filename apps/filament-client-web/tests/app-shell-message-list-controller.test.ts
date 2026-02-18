import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { messageIdFromInput, type MessageId } from "../src/domain/chat";
import {
  createMessageListController,
  type MessageListScrollMetrics,
} from "../src/features/app-shell/controllers/message-list-controller";

const MESSAGE_ID = messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAA");

interface ScrollElementHandle {
  element: HTMLElement;
  setScrollTop: (next: number) => void;
  setScrollHeight: (next: number) => void;
  setClientHeight: (next: number) => void;
}

function createScrollElement(
  initial: MessageListScrollMetrics & { clientHeight: number },
): ScrollElementHandle {
  const element = document.createElement("section");
  let scrollTop = initial.scrollTop;
  let scrollHeight = initial.scrollHeight;
  let clientHeight = initial.clientHeight;

  Object.defineProperty(element, "scrollTop", {
    configurable: true,
    get: () => scrollTop,
    set: (next: number) => {
      scrollTop = next;
    },
  });
  Object.defineProperty(element, "scrollHeight", {
    configurable: true,
    get: () => scrollHeight,
  });
  Object.defineProperty(element, "clientHeight", {
    configurable: true,
    get: () => clientHeight,
  });

  return {
    element,
    setScrollTop: (next) => {
      scrollTop = next;
    },
    setScrollHeight: (next) => {
      scrollHeight = next;
    },
    setClientHeight: (next) => {
      clientHeight = next;
    },
  };
}

describe("app shell message list controller", () => {
  it("updates load-older visibility and reaction picker reflow on scroll", () => {
    createRoot((dispose) => {
      const [nextBefore, setNextBefore] = createSignal<MessageId | null>(null);
      const [isLoadingOlder] = createSignal(false);
      const [openReactionPickerMessageId, setOpenReactionPickerMessageId] =
        createSignal<MessageId | null>(null);
      const [showLoadOlderButton, setShowLoadOlderButton] = createSignal(false);
      const updateReactionPickerOverlayPosition = vi.fn();

      const controller = createMessageListController({
        nextBefore,
        isLoadingOlder,
        openReactionPickerMessageId,
        setShowLoadOlderButton,
        updateReactionPickerOverlayPosition,
        scheduleAfterPaint: (callback) => callback(),
      });

      const metrics = createScrollElement({
        scrollTop: 40,
        scrollHeight: 1_600,
        clientHeight: 600,
      });

      controller.onListRef(metrics.element);
      expect(showLoadOlderButton()).toBe(false);

      setNextBefore(MESSAGE_ID);
      controller.updateLoadOlderButtonVisibility();
      expect(showLoadOlderButton()).toBe(true);

      metrics.setScrollTop(400);
      controller.onMessageListScroll(() => undefined);
      expect(showLoadOlderButton()).toBe(false);

      setOpenReactionPickerMessageId(MESSAGE_ID);
      controller.onMessageListScroll(() => undefined);
      expect(updateReactionPickerOverlayPosition).toHaveBeenCalledWith(MESSAGE_ID);

      dispose();
    });
  });

  it("auto-loads older pages only near top with a cursor and idle loader", () => {
    createRoot((dispose) => {
      const [nextBefore, setNextBefore] = createSignal<MessageId | null>(MESSAGE_ID);
      const [isLoadingOlder, setLoadingOlder] = createSignal(false);
      const [openReactionPickerMessageId] = createSignal<MessageId | null>(null);
      const [, setShowLoadOlderButton] = createSignal(false);

      const controller = createMessageListController({
        nextBefore,
        isLoadingOlder,
        openReactionPickerMessageId,
        setShowLoadOlderButton,
        updateReactionPickerOverlayPosition: () => undefined,
        autoLoadTopThresholdPx: 120,
        scheduleAfterPaint: (callback) => callback(),
      });

      const metrics = createScrollElement({
        scrollTop: 90,
        scrollHeight: 1_400,
        clientHeight: 500,
      });
      controller.onListRef(metrics.element);

      const loadOlderMessages = vi.fn();
      controller.onMessageListScroll(loadOlderMessages);
      expect(loadOlderMessages).toHaveBeenCalledTimes(1);

      setLoadingOlder(true);
      controller.onMessageListScroll(loadOlderMessages);
      expect(loadOlderMessages).toHaveBeenCalledTimes(1);

      setLoadingOlder(false);
      setNextBefore(null);
      controller.onMessageListScroll(loadOlderMessages);
      expect(loadOlderMessages).toHaveBeenCalledTimes(1);

      dispose();
    });
  });

  it("captures/restores scroll metrics and sticky-bottom behavior", () => {
    createRoot((dispose) => {
      const [nextBefore] = createSignal<MessageId | null>(MESSAGE_ID);
      const [isLoadingOlder] = createSignal(false);
      const [openReactionPickerMessageId] = createSignal<MessageId | null>(null);
      const [showLoadOlderButton, setShowLoadOlderButton] = createSignal(false);

      const controller = createMessageListController({
        nextBefore,
        isLoadingOlder,
        openReactionPickerMessageId,
        setShowLoadOlderButton,
        updateReactionPickerOverlayPosition: () => undefined,
        stickyBottomThresholdPx: 100,
        scheduleAfterPaint: (callback) => callback(),
      });

      const metrics = createScrollElement({
        scrollTop: 1_040,
        scrollHeight: 1_600,
        clientHeight: 500,
      });
      controller.onListRef(metrics.element);
      expect(controller.isMessageListNearBottom()).toBe(true);

      metrics.setScrollTop(850);
      expect(controller.isMessageListNearBottom()).toBe(false);

      const snapshot = controller.captureScrollMetrics();
      expect(snapshot).toEqual({ scrollHeight: 1_600, scrollTop: 850 });

      metrics.setScrollHeight(2_000);
      controller.restoreScrollAfterPrepend(snapshot);
      expect(metrics.element.scrollTop).toBe(1_250);

      controller.scrollMessageListToBottom();
      expect(metrics.element.scrollTop).toBe(2_000);
      expect(showLoadOlderButton()).toBe(false);

      dispose();
    });
  });

  it("keeps load-older hidden when history does not overflow the list viewport", () => {
    createRoot((dispose) => {
      const [nextBefore] = createSignal<MessageId | null>(MESSAGE_ID);
      const [isLoadingOlder] = createSignal(false);
      const [openReactionPickerMessageId] = createSignal<MessageId | null>(null);
      const [showLoadOlderButton, setShowLoadOlderButton] = createSignal(false);

      const controller = createMessageListController({
        nextBefore,
        isLoadingOlder,
        openReactionPickerMessageId,
        setShowLoadOlderButton,
        updateReactionPickerOverlayPosition: () => undefined,
        scheduleAfterPaint: (callback) => callback(),
      });

      const metrics = createScrollElement({
        scrollTop: 0,
        scrollHeight: 400,
        clientHeight: 800,
      });
      controller.onListRef(metrics.element);

      controller.updateLoadOlderButtonVisibility();
      expect(showLoadOlderButton()).toBe(false);

      dispose();
    });
  });
});
