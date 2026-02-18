import { render, screen } from "@solidjs/testing-library";
import { fireEvent } from "@solidjs/testing-library";
import { describe, expect, it } from "vitest";
import {
  channelIdFromInput,
  guildIdFromInput,
  messageFromResponse,
  type MessageRecord,
  userIdFromInput,
} from "../src/domain/chat";
import { MessageList } from "../src/features/app-shell/components/messages/MessageList";

const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const AUTHOR_ID = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");

function messageIdForIndex(index: number): string {
  return String(index + 1).padStart(26, "0");
}

function messageFixture(index: number): MessageRecord {
  const text = `message-${index}`;
  return messageFromResponse({
    message_id: messageIdForIndex(index),
    guild_id: GUILD_ID,
    channel_id: CHANNEL_ID,
    author_id: AUTHOR_ID,
    content: text,
    markdown_tokens: [{ type: "text", text }],
    attachments: [],
    created_at_unix: index + 1,
  });
}

function renderList(messages: MessageRecord[], maxRenderedMessages?: number): void {
  render(() => (
    <MessageList
      messages={messages}
      maxRenderedMessages={maxRenderedMessages}
      maxHistoricalRenderedMessages={900}
      nextBefore={null}
      showLoadOlderButton={false}
      isLoadingOlder={false}
      isLoadingMessages={false}
      messageError=""
      onLoadOlderMessages={() => undefined}
      onListScroll={() => undefined}
      onListRef={() => undefined}
      currentUserId={AUTHOR_ID}
      canDeleteMessages={false}
      displayUserLabel={(userId) => userId}
      resolveAvatarUrl={() => null}
      onOpenAuthorProfile={() => undefined}
      editingMessageId={null}
      editingDraft=""
      isSavingEdit={false}
      deletingMessageId={null}
      openReactionPickerMessageId={null}
      reactionState={{}}
      pendingReactionByKey={{}}
      messageMediaByAttachmentId={{}}
      loadingMediaPreviewIds={{}}
      failedMediaPreviewIds={{}}
      downloadingAttachmentId={null}
      addReactionIconUrl=""
      editMessageIconUrl=""
      deleteMessageIconUrl=""
      onEditingDraftInput={() => undefined}
      onSaveEditMessage={() => undefined}
      onCancelEditMessage={() => undefined}
      onDownloadAttachment={() => undefined}
      onRetryMediaPreview={() => undefined}
      onToggleMessageReaction={() => undefined}
      onToggleReactionPicker={() => undefined}
      onBeginEditMessage={() => undefined}
      onRemoveMessage={() => undefined}
    />
  ));
}

describe("app shell message list", () => {
  it("renders all messages when history is below the window size", () => {
    const messages = Array.from({ length: 6 }, (_, index) => messageFixture(index));
    renderList(messages, 10);

    const rows = document.querySelectorAll(".message-row");
    expect(rows).toHaveLength(6);
    expect(screen.getByText("message-0")).toBeInTheDocument();
    expect(screen.getByText("message-5")).toBeInTheDocument();
  });

  it("renders a trailing bounded window for dense histories", () => {
    const messages = Array.from({ length: 260 }, (_, index) => messageFixture(index));
    renderList(messages);

    const rows = document.querySelectorAll(".message-row");
    expect(rows).toHaveLength(240);
    expect(screen.queryByText("message-0")).not.toBeInTheDocument();
    expect(screen.getByText("message-20")).toBeInTheDocument();
    expect(screen.getByText("message-259")).toBeInTheDocument();
  });

  it("expands to full history when scrolled away from latest", async () => {
    const messages = Array.from({ length: 260 }, (_, index) => messageFixture(index));
    renderList(messages);

    const listElement = document.querySelector(".message-list") as HTMLElement;
    Object.defineProperty(listElement, "scrollHeight", {
      configurable: true,
      get: () => 2_000,
    });
    Object.defineProperty(listElement, "clientHeight", {
      configurable: true,
      get: () => 600,
    });

    listElement.scrollTop = 1_360;
    await fireEvent.scroll(listElement);

    expect(document.querySelectorAll(".message-row")).toHaveLength(240);

    listElement.scrollTop = 100;
    await fireEvent.scroll(listElement);

    const rows = document.querySelectorAll(".message-row");
    expect(rows).toHaveLength(260);
    expect(screen.getByText("message-0")).toBeInTheDocument();
    expect(screen.getByText("message-259")).toBeInTheDocument();
  });

  it("renders full dense history when scrolled away from latest", async () => {
    const messages = Array.from({ length: 1_500 }, (_, index) => messageFixture(index));
    renderList(messages);

    const listElement = document.querySelector(".message-list") as HTMLElement;
    Object.defineProperty(listElement, "scrollHeight", {
      configurable: true,
      get: () => 12_000,
    });
    Object.defineProperty(listElement, "clientHeight", {
      configurable: true,
      get: () => 800,
    });

    listElement.scrollTop = 100;
    await fireEvent.scroll(listElement);

    const rows = document.querySelectorAll(".message-row");
    expect(rows).toHaveLength(1_500);
    expect(screen.getByText("message-0")).toBeInTheDocument();
    expect(screen.getByText("message-1499")).toBeInTheDocument();
  });
});
