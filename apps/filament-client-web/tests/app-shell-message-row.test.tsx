import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  guildIdFromInput,
  messageFromResponse,
  reactionEmojiFromInput,
  type AttachmentRecord,
  type MessageRecord,
  userIdFromInput,
} from "../src/domain/chat";
import {
  MessageRow,
  type MessageRowProps,
} from "../src/features/app-shell/components/messages/MessageRow";

const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const AUTHOR_ID = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");
const REACTION = reactionEmojiFromInput("👍");

function messageFixture(): MessageRecord {
  return messageFromResponse({
    message_id: "01ARZ3NDEKTSV4RRFFQ69G5FAZ",
    guild_id: GUILD_ID,
    channel_id: CHANNEL_ID,
    author_id: AUTHOR_ID,
    content: "primary alert",
    markdown_tokens: [{ type: "text", text: "primary alert" }],
    attachments: [
      {
        attachment_id: "01ARZ3NDEKTSV4RRFFQ69G5FB0",
        guild_id: GUILD_ID,
        channel_id: CHANNEL_ID,
        owner_id: AUTHOR_ID,
        filename: "evidence.png",
        mime_type: "image/png",
        size_bytes: 1024,
        sha256_hex: "a".repeat(64),
      },
    ],
    created_at_unix: 1,
  });
}

function rowPropsFixture(overrides: Partial<MessageRowProps> = {}): MessageRowProps {
  const message = messageFixture();
  return {
    message,
    currentUserId: AUTHOR_ID,
    canDeleteMessages: false,
    displayUserLabel: () => "alice",
    resolveAvatarUrl: () => null,
    onOpenAuthorProfile: () => undefined,
    editingMessageId: null,
    editingDraft: "",
    isSavingEdit: false,
    deletingMessageId: null,
    openReactionPickerMessageId: null,
    reactionState: {
      [`${message.messageId}|${REACTION}`]: { count: 2, reacted: false },
    },
    pendingReactionByKey: {},
    messageMediaByAttachmentId: {},
    loadingMediaPreviewIds: {},
    failedMediaPreviewIds: {},
    downloadingAttachmentId: null,
    addReactionIconUrl: "/icons/reaction.svg",
    editMessageIconUrl: "/icons/edit.svg",
    deleteMessageIconUrl: "/icons/delete.svg",
    onEditingDraftInput: () => undefined,
    onSaveEditMessage: () => undefined,
    onCancelEditMessage: () => undefined,
    onDownloadAttachment: (_record: AttachmentRecord) => undefined,
    onRetryMediaPreview: () => undefined,
    onToggleMessageReaction: () => undefined,
    onToggleReactionPicker: () => undefined,
    onBeginEditMessage: () => undefined,
    onRemoveMessage: () => undefined,
    ...overrides,
  };
}

describe("app shell message row", () => {
  it("renders with Uno utility classes and without legacy MessageRow internals", () => {
    render(() => <MessageRow {...rowPropsFixture()} />);

    const row = document.querySelector("article.message-row");
    expect(row).not.toBeNull();
    expect(row).toHaveClass("group");
    expect(row).toHaveClass("grid");
    expect(row).toHaveClass("[&:first-of-type]:mt-auto");

    const tokenized = screen.getByText("primary alert");
    expect(tokenized).toHaveClass("message-tokenized");
    expect(tokenized).toHaveClass("whitespace-pre-wrap");
    expect(tokenized).toHaveClass("text-ink-1");

    const downloadButton = screen.getByRole("button", {
      name: "Download evidence.png",
    });
    expect(downloadButton).toHaveClass("border-brand");
    expect(downloadButton).toHaveClass("rounded-[0.56rem]");

    const reactionChip = screen.getByRole("button", {
      name: "👍 reaction (2)",
    });
    expect(reactionChip).toHaveClass("reaction-chip");
    expect(reactionChip).toHaveClass("border-line-soft");
    expect(screen.getByRole("img", { name: "👍" })).toBeInTheDocument();

    const hoverActions = document.querySelector(".message-hover-actions");
    expect(hoverActions).not.toBeNull();
    expect(hoverActions).toHaveClass("opacity-0");
    expect(hoverActions).toHaveClass("shadow-panel");

    const removedLegacyHooks = [
      ".message-avatar-button",
      ".message-avatar",
      ".message-avatar-fallback",
      ".message-avatar-image",
      ".message-main",
      ".message-meta",
      ".reaction-row",
      ".reaction-controls",
      ".reaction-list",
      ".message-actions",
      ".message-attachments",
      ".message-attachment-card",
      ".message-attachment-download",
      ".message-attachment-meta",
      ".message-attachment-loading",
      ".message-attachment-failed",
      ".message-attachment-retry",
      ".message-edit",
      ".inline-form",
    ];

    for (const selector of removedLegacyHooks) {
      expect(document.querySelector(selector)).toBeNull();
    }
  });

  it("routes interactions and reflects reacted and deleting utility states", async () => {
    const onOpenAuthorProfile = vi.fn();
    const onToggleMessageReaction = vi.fn();
    const onRemoveMessage = vi.fn();
    const message = messageFixture();

    render(() => (
      <MessageRow
        {...rowPropsFixture({
          message,
          reactionState: {
            [`${message.messageId}|${REACTION}`]: { count: 3, reacted: true },
          },
          deletingMessageId: message.messageId,
          onOpenAuthorProfile,
          onToggleMessageReaction,
          onRemoveMessage,
        })}
      />
    ));

    await fireEvent.click(screen.getByRole("button", { name: "Open alice profile" }));
    expect(onOpenAuthorProfile).toHaveBeenCalledOnce();
    expect(onOpenAuthorProfile).toHaveBeenCalledWith(AUTHOR_ID);

    const reactionChip = screen.getByRole("button", {
      name: "👍 reaction (3)",
    });
    expect(reactionChip).toHaveClass("border-line");
    await fireEvent.click(reactionChip);
    expect(onToggleMessageReaction).toHaveBeenCalledOnce();
    expect(onToggleMessageReaction).toHaveBeenCalledWith(message.messageId, REACTION);

    const deleteButton = screen.getByRole("button", { name: "Delete message" });
    expect(deleteButton).toBeDisabled();
    expect(deleteButton).toHaveClass("text-danger");
    expect(deleteButton.querySelector(".icon-mask")).toHaveClass("animate-pulse");
    expect(onRemoveMessage).not.toHaveBeenCalled();
  });
});
