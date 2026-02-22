import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import { messageIdFromInput, reactionEmojiFromInput } from "../src/domain/chat";
import { ReactionPickerPortal } from "../src/features/app-shell/components/messages/ReactionPickerPortal";

const MESSAGE_ID = messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAC");

describe("app shell reaction picker portal", () => {
  it("does not render when required picker state is missing", () => {
    render(() => (
      <ReactionPickerPortal
        openMessageId={null}
        onClose={() => undefined}
        onAddReaction={() => undefined}
      />
    ));

    expect(screen.queryByRole("dialog", { name: "Choose reaction" })).toBeNull();
  });
});
