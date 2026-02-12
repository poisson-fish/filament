import { createEffect, onCleanup, type Accessor, type Setter } from "solid-js";
import type { MessageId } from "../../../domain/chat";
import type { ReactionPickerOverlayPosition } from "../types";
import {
  REACTION_PICKER_OVERLAY_ESTIMATED_HEIGHT_PX,
  REACTION_PICKER_OVERLAY_GAP_PX,
  REACTION_PICKER_OVERLAY_MARGIN_PX,
  REACTION_PICKER_OVERLAY_MAX_WIDTH_PX,
} from "../config/ui-constants";

export interface ReactionPickerOverlayPositionOptions {
  anchorRect: DOMRectReadOnly;
  viewportWidth: number;
  viewportHeight: number;
  overlayMaxWidthPx?: number;
  overlayMarginPx?: number;
  overlayEstimatedHeightPx?: number;
  overlayGapPx?: number;
}

export interface ReactionPickerControllerOptions {
  openReactionPickerMessageId: Accessor<MessageId | null>;
  setOpenReactionPickerMessageId: Setter<MessageId | null>;
  setReactionPickerOverlayPosition: Setter<ReactionPickerOverlayPosition | null>;
  trackPositionDependencies?: () => void;
  scheduleAfterPaint?: (callback: () => void) => void;
}

export interface ReactionPickerController {
  reactionPickerAnchorSelector: (messageId: MessageId) => string;
  updateReactionPickerOverlayPosition: (messageId: MessageId) => void;
}

function defaultScheduleAfterPaint(callback: () => void): void {
  if (typeof window.requestAnimationFrame === "function") {
    window.requestAnimationFrame(() => callback());
    return;
  }
  window.setTimeout(callback, 0);
}

export function reactionPickerAnchorSelector(messageId: MessageId): string {
  return `[data-reaction-anchor-for="${messageId}"]`;
}

export function resolveReactionPickerOverlayPosition(
  options: ReactionPickerOverlayPositionOptions,
): ReactionPickerOverlayPosition {
  const overlayMaxWidthPx =
    options.overlayMaxWidthPx ?? REACTION_PICKER_OVERLAY_MAX_WIDTH_PX;
  const overlayMarginPx = options.overlayMarginPx ?? REACTION_PICKER_OVERLAY_MARGIN_PX;
  const overlayEstimatedHeightPx =
    options.overlayEstimatedHeightPx ?? REACTION_PICKER_OVERLAY_ESTIMATED_HEIGHT_PX;
  const overlayGapPx = options.overlayGapPx ?? REACTION_PICKER_OVERLAY_GAP_PX;

  const overlayWidth = Math.min(
    overlayMaxWidthPx,
    Math.max(240, options.viewportWidth - overlayMarginPx * 2),
  );
  const maxLeft = Math.max(
    overlayMarginPx,
    options.viewportWidth - overlayWidth - overlayMarginPx,
  );
  const left = Math.min(
    maxLeft,
    Math.max(overlayMarginPx, options.anchorRect.right - overlayWidth),
  );

  const preferredTop = options.anchorRect.bottom + overlayGapPx;
  const canPlaceBelow =
    preferredTop + overlayEstimatedHeightPx <= options.viewportHeight - overlayMarginPx;
  const top = canPlaceBelow
    ? preferredTop
    : Math.max(
        overlayMarginPx,
        options.anchorRect.top - overlayEstimatedHeightPx - overlayGapPx,
      );

  return { top, left };
}

export function createReactionPickerController(
  options: ReactionPickerControllerOptions,
): ReactionPickerController {
  const scheduleAfterPaint = options.scheduleAfterPaint ?? defaultScheduleAfterPaint;

  const updateReactionPickerOverlayPosition = (messageId: MessageId): void => {
    const anchor = document.querySelector(
      reactionPickerAnchorSelector(messageId),
    ) as HTMLElement | null;
    if (!anchor) {
      options.setReactionPickerOverlayPosition(null);
      return;
    }

    options.setReactionPickerOverlayPosition(
      resolveReactionPickerOverlayPosition({
        anchorRect: anchor.getBoundingClientRect(),
        viewportWidth: window.innerWidth,
        viewportHeight: window.innerHeight,
      }),
    );
  };

  createEffect(() => {
    const openPickerMessageId = options.openReactionPickerMessageId();
    if (!openPickerMessageId) {
      options.setReactionPickerOverlayPosition(null);
      return;
    }

    const updatePosition = () =>
      updateReactionPickerOverlayPosition(openPickerMessageId);
    scheduleAfterPaint(updatePosition);

    const onWindowResize = () => updatePosition();
    const onWindowScroll = () => updatePosition();
    const onWindowKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        options.setOpenReactionPickerMessageId(null);
      }
    };
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (!(target instanceof Node)) {
        return;
      }
      const picker = document.querySelector(
        ".reaction-picker-floating",
      ) as HTMLElement | null;
      if (picker?.contains(target)) {
        return;
      }
      if (
        target instanceof Element &&
        target.closest(reactionPickerAnchorSelector(openPickerMessageId))
      ) {
        return;
      }
      options.setOpenReactionPickerMessageId(null);
    };

    window.addEventListener("resize", onWindowResize);
    window.addEventListener("scroll", onWindowScroll, true);
    window.addEventListener("keydown", onWindowKeydown);
    window.addEventListener("pointerdown", onPointerDown);

    onCleanup(() => {
      window.removeEventListener("resize", onWindowResize);
      window.removeEventListener("scroll", onWindowScroll, true);
      window.removeEventListener("keydown", onWindowKeydown);
      window.removeEventListener("pointerdown", onPointerDown);
    });
  });

  createEffect(() => {
    const openPickerMessageId = options.openReactionPickerMessageId();
    if (!openPickerMessageId) {
      return;
    }
    options.trackPositionDependencies?.();
    scheduleAfterPaint(() =>
      updateReactionPickerOverlayPosition(openPickerMessageId),
    );
  });

  return {
    reactionPickerAnchorSelector,
    updateReactionPickerOverlayPosition,
  };
}
