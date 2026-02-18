import { createEffect, type Accessor, type Setter } from "solid-js";
import type { MessageId } from "../../../domain/chat";
import {
  MESSAGE_AUTOLOAD_TOP_THRESHOLD_PX,
  MESSAGE_LOAD_OLDER_BUTTON_TOP_THRESHOLD_PX,
  MESSAGE_STICKY_BOTTOM_THRESHOLD_PX,
} from "../config/ui-constants";

export interface MessageListScrollMetrics {
  scrollHeight: number;
  scrollTop: number;
}

export interface MessageListControllerOptions {
  nextBefore: Accessor<MessageId | null>;
  isLoadingOlder: Accessor<boolean>;
  openReactionPickerMessageId: Accessor<MessageId | null>;
  setShowLoadOlderButton: Setter<boolean>;
  updateReactionPickerOverlayPosition: (messageId: MessageId) => void;
  stickyBottomThresholdPx?: number;
  loadOlderButtonTopThresholdPx?: number;
  autoLoadTopThresholdPx?: number;
  scheduleAfterPaint?: (callback: () => void) => void;
}

export interface MessageListController {
  onListRef: (element: HTMLElement) => void;
  runAfterMessageListPaint: (callback: (element: HTMLElement) => void) => void;
  isMessageListNearBottom: () => boolean;
  updateLoadOlderButtonVisibility: () => void;
  scrollMessageListToBottom: () => void;
  onMessageListScroll: (
    loadOlderMessages: () => Promise<void> | void,
  ) => void;
  captureScrollMetrics: () => MessageListScrollMetrics | null;
  restoreScrollAfterPrepend: (metrics: MessageListScrollMetrics | null) => void;
}

function defaultScheduleAfterPaint(callback: () => void): void {
  if (typeof window.requestAnimationFrame === "function") {
    window.requestAnimationFrame(() => callback());
    return;
  }
  window.setTimeout(callback, 0);
}

export function createMessageListController(
  options: MessageListControllerOptions,
): MessageListController {
  const stickyBottomThresholdPx =
    options.stickyBottomThresholdPx ?? MESSAGE_STICKY_BOTTOM_THRESHOLD_PX;
  const loadOlderButtonTopThresholdPx =
    options.loadOlderButtonTopThresholdPx ??
    MESSAGE_LOAD_OLDER_BUTTON_TOP_THRESHOLD_PX;
  const autoLoadTopThresholdPx =
    options.autoLoadTopThresholdPx ?? MESSAGE_AUTOLOAD_TOP_THRESHOLD_PX;
  const scheduleAfterPaint = options.scheduleAfterPaint ?? defaultScheduleAfterPaint;

  let messageListElement: HTMLElement | undefined;

  const runAfterMessageListPaint = (callback: (element: HTMLElement) => void): void => {
    scheduleAfterPaint(() => {
      const element = messageListElement;
      if (!element) {
        return;
      }
      callback(element);
    });
  };

  const isMessageListNearBottom = (): boolean => {
    const element = messageListElement;
    if (!element) {
      return true;
    }
    const distanceFromBottom =
      element.scrollHeight - element.clientHeight - element.scrollTop;
    return distanceFromBottom <= stickyBottomThresholdPx;
  };

  const updateLoadOlderButtonVisibility = (): void => {
    const before = options.nextBefore();
    const element = messageListElement;
    if (!before || !element) {
      options.setShowLoadOlderButton(false);
      return;
    }

    const maxScrollTop = Math.max(element.scrollHeight - element.clientHeight, 0);
    if (maxScrollTop <= 0) {
      options.setShowLoadOlderButton(false);
      return;
    }

    const distanceFromBottom =
      element.scrollHeight - element.clientHeight - element.scrollTop;
    const isNearTop = element.scrollTop <= loadOlderButtonTopThresholdPx;
    const isNearBottom = distanceFromBottom <= stickyBottomThresholdPx;
    options.setShowLoadOlderButton(isNearTop && !isNearBottom);
  };

  const scrollMessageListToBottom = (): void => {
    runAfterMessageListPaint((element) => {
      element.scrollTop = element.scrollHeight;
      updateLoadOlderButtonVisibility();
    });
  };

  const captureScrollMetrics = (): MessageListScrollMetrics | null => {
    const element = messageListElement;
    if (!element) {
      return null;
    }
    return {
      scrollHeight: element.scrollHeight,
      scrollTop: element.scrollTop,
    };
  };

  const restoreScrollAfterPrepend = (
    metrics: MessageListScrollMetrics | null,
  ): void => {
    if (!metrics) {
      return;
    }
    runAfterMessageListPaint((element) => {
      const delta = element.scrollHeight - metrics.scrollHeight;
      element.scrollTop = metrics.scrollTop + delta;
      updateLoadOlderButtonVisibility();
    });
  };

  const onMessageListScroll = (
    loadOlderMessages: () => Promise<void> | void,
  ): void => {
    updateLoadOlderButtonVisibility();
    const openPickerMessageId = options.openReactionPickerMessageId();
    if (openPickerMessageId) {
      options.updateReactionPickerOverlayPosition(openPickerMessageId);
    }
    const before = options.nextBefore();
    const element = messageListElement;
    if (!before || !element || options.isLoadingOlder()) {
      return;
    }
    if (element.scrollTop <= autoLoadTopThresholdPx) {
      void loadOlderMessages();
    }
  };

  const onListRef = (element: HTMLElement): void => {
    messageListElement = element;
    runAfterMessageListPaint(() => {
      updateLoadOlderButtonVisibility();
    });
  };

  createEffect(() => {
    void options.nextBefore();
    runAfterMessageListPaint(() => {
      updateLoadOlderButtonVisibility();
    });
  });

  return {
    onListRef,
    runAfterMessageListPaint,
    isMessageListNearBottom,
    updateLoadOlderButtonVisibility,
    scrollMessageListToBottom,
    onMessageListScroll,
    captureScrollMetrics,
    restoreScrollAfterPrepend,
  };
}
