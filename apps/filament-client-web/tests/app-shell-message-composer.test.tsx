import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  channelIdFromInput,
  channelNameFromInput,
  type ChannelRecord,
} from "../src/domain/chat";
import {
  MessageComposer,
  type MessageComposerProps,
} from "../src/features/app-shell/components/messages/MessageComposer";

const CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");

function channelFixture(): ChannelRecord {
  return {
    channelId: CHANNEL_ID,
    name: channelNameFromInput("incident-room"),
    kind: "text",
  };
}

function composerPropsFixture(
  overrides: Partial<MessageComposerProps> = {},
): MessageComposerProps {
  return {
    activeChannel: channelFixture(),
    canAccessActiveChannel: true,
    isSendingMessage: false,
    composerValue: "",
    composerAttachments: [],
    onSubmit: () => undefined,
    onComposerInput: () => undefined,
    onOpenAttachmentPicker: () => undefined,
    onAttachmentInput: () => undefined,
    onRemoveAttachment: () => undefined,
    attachmentInputRef: () => undefined,
    composerInputRef: () => undefined,
    ...overrides,
  };
}

describe("app shell message composer", () => {
  it("renders composer with Uno utility classes and without legacy internal hooks", () => {
    render(() => <MessageComposer {...composerPropsFixture()} />);

    const composerForm = document.querySelector("form.composer");
    expect(composerForm).not.toBeNull();
    expect(composerForm).toHaveClass("grid");
    expect(composerForm).toHaveClass("border-t");
    expect(composerForm).toHaveClass("bg-bg-2");

    const fileInput = document.querySelector(".composer-file-input");
    expect(fileInput).not.toBeNull();
    expect(fileInput).toHaveClass("hidden");

    const attachButton = screen.getByRole("button", { name: "Attach files" });
    const giftButton = screen.getByRole("button", { name: "Open gift picker" });
    const gifButton = screen.getByRole("button", { name: "Open GIF picker" });
    const emojiButton = screen.getByRole("button", { name: "Open emoji picker" });
    expect(attachButton).toHaveClass("border-r");
    expect(giftButton).toHaveClass("h-[2.12rem]");
    expect(gifButton).toHaveTextContent("GIF");
    expect(emojiButton).toHaveClass("h-[2.12rem]");

    expect(document.querySelector(".composer-input-shell")).toBeNull();
    expect(document.querySelector(".composer-send-button")).toBeNull();
    expect(document.querySelector(".composer-attach-button")).toBeNull();
    expect(document.querySelector(".composer-text-input")).toBeNull();
    expect(document.querySelector(".composer-attachment-pill")).toBeNull();
  });

  it("disables composer controls when no accessible channel is active", () => {
    render(() =>
      <MessageComposer
        {...composerPropsFixture({
          activeChannel: null,
          canAccessActiveChannel: false,
        })}
      />
    );

    expect(screen.getByRole("button", { name: "Attach files" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Open gift picker" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Open GIF picker" })).toBeDisabled();
    expect(screen.getByRole("button", { name: "Open emoji picker" })).toBeDisabled();
    expect(screen.getByPlaceholderText("Select channel")).toBeDisabled();
  });

  it("opens and closes the emoji picker from the composer emoji button", async () => {
    render(() => <MessageComposer {...composerPropsFixture()} />);

    const emojiButton = screen.getByRole("button", { name: "Open emoji picker" });
    expect(screen.queryByRole("dialog", { name: "Choose emoji" })).toBeNull();

    await fireEvent.click(emojiButton);
    expect(screen.getByRole("dialog", { name: "Choose emoji" })).toBeInTheDocument();

    await fireEvent.click(emojiButton);
    expect(screen.queryByRole("dialog", { name: "Choose emoji" })).toBeNull();
  });

  it("converts :shortcode: input into native emoji before forwarding composer input", async () => {
    const onComposerInput = vi.fn();

    render(() => <MessageComposer {...composerPropsFixture({ onComposerInput })} />);

    const input = screen.getByPlaceholderText("Message #incident-room") as HTMLInputElement;
    await fireEvent.input(input, { target: { value: ":joy:" } });

    expect(onComposerInput).toHaveBeenCalledWith("😂");
  });

  it("renders attachment pills and routes remove actions", async () => {
    const onRemoveAttachment = vi.fn();
    const attachment = new File(["x"], "sample.txt", { type: "text/plain" });

    render(() =>
      <MessageComposer
        {...composerPropsFixture({
          composerAttachments: [attachment],
          onRemoveAttachment,
        })}
      />
    );

    await fireEvent.click(screen.getByRole("button", { name: /sample\.txt/i }));
    expect(onRemoveAttachment).toHaveBeenCalledOnce();
    expect(onRemoveAttachment).toHaveBeenCalledWith(attachment);
  });
});
