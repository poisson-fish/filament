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
    expect(composerForm).toHaveClass("bg-bg-1");

    const fileInput = document.querySelector(".composer-file-input");
    expect(fileInput).not.toBeNull();
    expect(fileInput).toHaveClass("hidden");

    const attachButton = screen.getByRole("button", { name: "Attach files" });
    const sendButton = screen.getByRole("button", { name: "Send" });
    expect(attachButton).toHaveClass("border-r");
    expect(sendButton).toHaveClass("border-l");

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
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
    expect(screen.getByPlaceholderText("Select channel")).toBeDisabled();
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
