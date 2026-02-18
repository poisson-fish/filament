import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import { messageIdFromInput, reactionEmojiFromInput } from "../src/domain/chat";
import { ReactionPickerPortal } from "../src/features/app-shell/components/messages/ReactionPickerPortal";

const MESSAGE_ID = messageIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAC");

describe("app shell reaction picker portal", () => {
  it("renders with Uno utility classes while preserving the floating hook", async () => {
    const onClose = vi.fn();
    const onAddReaction = vi.fn();

    render(() => (
      <ReactionPickerPortal
        openMessageId={MESSAGE_ID}
        position={{ top: 120, left: 240 }}
        options={[
          {
            emoji: reactionEmojiFromInput("👍"),
            label: "Thumbs up",
            iconUrl: "/emoji/thumbs-up.svg",
          },
        ]}
        onClose={onClose}
        onAddReaction={onAddReaction}
      />
    ));

    const dialog = await screen.findByRole("dialog", { name: "Choose reaction" });
    expect(dialog).toHaveClass("reaction-picker-floating");
    expect(dialog).toHaveClass("fx-panel");
    expect(dialog).toHaveClass("fixed");

    expect(document.querySelector(".reaction-picker-header")).toBeNull();
    expect(document.querySelector(".reaction-picker-title")).toBeNull();
    expect(document.querySelector(".reaction-picker-close")).toBeNull();
    expect(document.querySelector(".reaction-picker-grid")).toBeNull();
    expect(document.querySelector(".reaction-picker-option")).toBeNull();

    await fireEvent.click(screen.getByRole("button", { name: "Add Thumbs up reaction" }));
    expect(onAddReaction).toHaveBeenCalledOnce();
    expect(onAddReaction).toHaveBeenCalledWith(MESSAGE_ID, reactionEmojiFromInput("👍"));

    await fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(onClose).toHaveBeenCalledOnce();
  });

  it("does not render when required picker state is missing", () => {
    render(() => (
      <ReactionPickerPortal
        openMessageId={null}
        position={{ top: 120, left: 240 }}
        options={[]}
        onClose={() => undefined}
        onAddReaction={() => undefined}
      />
    ));

    expect(screen.queryByRole("dialog", { name: "Choose reaction" })).toBeNull();
  });
});
