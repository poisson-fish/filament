import { render } from "@solidjs/testing-library";
import { describe, expect, it } from "vitest";
import {
  replaceEmojiShortcodes,
  replaceEmojiShortcodesWithSelection,
  renderEmojiMixedText,
} from "../src/features/app-shell/components/messages/emoji-utils";

describe("emoji utils", () => {
  it("replaces supported shortcodes with native emoji", () => {
    expect(replaceEmojiShortcodes(":joy: :+1:")).toBe("😂 👍");
    expect(replaceEmojiShortcodes(":JOY:")).toBe("😂");
    expect(replaceEmojiShortcodes(":not_real_emoji:")).toBe(":not_real_emoji:");
  });

  it("remaps selection without shifting the cursor for replacements after the caret", () => {
    const replacement = replaceEmojiShortcodesWithSelection("a :joy: z", 1, 1);
    expect(replacement.text).toBe("a 😂 z");
    expect(replacement.selectionStart).toBe(1);
    expect(replacement.selectionEnd).toBe(1);
  });

  it("renders mixed text with twemoji sprite spans", () => {
    const view = render(() => <p>{renderEmojiMixedText("ok 😂")}</p>);
    expect(view.container.textContent).toContain("ok ");
    const emojiSprite = view.getByRole("img", { name: "😂" });
    expect(emojiSprite.getAttribute("style")).toContain("background-image");
  });
});
