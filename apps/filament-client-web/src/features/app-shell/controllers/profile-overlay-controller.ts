import { createEffect, onCleanup, type Accessor, type Setter } from "solid-js";
import type { UserId } from "../../../domain/chat";

export interface ProfileOverlayControllerOptions {
  selectedProfileUserId: Accessor<UserId | null>;
  setSelectedProfileUserId: Setter<UserId | null>;
}

export function createProfileOverlayController(
  options: ProfileOverlayControllerOptions,
): void {
  createEffect(() => {
    if (!options.selectedProfileUserId()) {
      return;
    }
    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        options.setSelectedProfileUserId(null);
      }
    };
    window.addEventListener("keydown", onKeydown);
    onCleanup(() => window.removeEventListener("keydown", onKeydown));
  });
}
