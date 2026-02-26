import { describe, expect, it } from "vitest";
import { DomainValidationError } from "../src/domain/auth";
import {
  channelIdFromInput,
  guildIdFromInput,
  messageFromResponse,
  messageIdFromInput,
  reactionEmojiFromInput,
  userIdFromInput,
} from "../src/domain/chat";
import {
  channelKey,
  channelRailLabel,
  mapError,
  mapVoiceJoinError,
  mergeMessage,
  mergeMessageHistory,
  mergeReactionStateFromMessages,
  parseChannelKey,
  parsePermissionCsv,
  reactionKey,
  reactionViewsForMessage,
  replaceReactionStateFromMessages,
  resolveAttachmentPreviewType,
  tokenizeToDisplayText,
  userIdFromVoiceIdentity,
} from "../src/features/app-shell/helpers";
import { ApiError } from "../src/lib/api";
import { RtcClientError } from "../src/lib/rtc";

const GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const MESSAGE_ID_1 = "01ARZ3NDEKTSV4RRFFQ69G5FAY";
const MESSAGE_ID_2 = "01ARZ3NDEKTSV4RRFFQ69G5FAZ";
const MESSAGE_ID_3 = "01ARZ3NDEKTSV4RRFFQ69G5FB0";

function buildMessage(input: {
  messageId: string;
  content: string;
  createdAtUnix: number;
  reactions?: Array<{ emoji: string; count: number }>;
}) {
  return messageFromResponse({
    message_id: input.messageId,
    guild_id: GUILD_ID,
    channel_id: CHANNEL_ID,
    author_id: USER_ID,
    content: input.content,
    markdown_tokens: [{ type: "text", text: input.content }],
    attachments: [],
    reactions: input.reactions,
    created_at_unix: input.createdAtUnix,
  });
}

describe("app shell helpers", () => {
  it("returns sorted reaction views scoped to one message", () => {
    const messageId = messageIdFromInput(MESSAGE_ID_1);
    const otherMessageId = messageIdFromInput(MESSAGE_ID_2);
    const thumbsUp = reactionEmojiFromInput("👍");
    const fire = reactionEmojiFromInput("🔥");

    const views = reactionViewsForMessage(
      messageId,
      {
        [reactionKey(messageId, thumbsUp)]: { count: 1, reacted: true },
        [reactionKey(messageId, fire)]: { count: 3, reacted: false },
        [reactionKey(otherMessageId, fire)]: { count: 9, reacted: true },
      },
      {
        [reactionKey(messageId, fire)]: true,
      },
    );

    expect(views).toHaveLength(2);
    expect(views[0]?.emoji).toBe(fire);
    expect(views[0]?.count).toBe(3);
    expect(views[0]?.pending).toBe(true);
    expect(views[1]?.emoji).toBe(thumbsUp);
  });

  it("parses and validates channel keys", () => {
    const encoded = channelKey(guildIdFromInput(GUILD_ID), channelIdFromInput(CHANNEL_ID));

    const parsed = parseChannelKey(encoded);
    expect(parsed?.guildId).toBe(GUILD_ID);
    expect(parsed?.channelId).toBe(CHANNEL_ID);
    expect(parseChannelKey("missing-separator")).toBeNull();
  });

  it("uses filename inference for media previews when payload mime is generic", () => {
    expect(resolveAttachmentPreviewType(null, "application/octet-stream", "photo.png")).toEqual({
      kind: "image",
      mimeType: "image/png",
    });
    expect(resolveAttachmentPreviewType("text/plain", "application/octet-stream", "clip.mp4")).toEqual({
      kind: "video",
      mimeType: "video/mp4",
    });
    expect(resolveAttachmentPreviewType("image/svg+xml", "image/svg+xml", "vector.svg")).toEqual({
      kind: "file",
      mimeType: "image/svg+xml",
    });
  });

  it("maps domain and API errors to stable user messages", () => {
    expect(mapError(new DomainValidationError("bad input"), "fallback")).toBe("bad input");
    expect(mapError(new ApiError(429, "rate_limited", "rate_limited"), "fallback")).toBe(
      "Rate limited. Please wait and retry.",
    );
    expect(mapError(new ApiError(408, "request_timeout", "request_timeout"), "fallback")).toBe(
      "Server timed out while processing the request.",
    );
    expect(mapError(new ApiError(500, "internal_error", "internal_error"), "fallback")).toBe(
      "Server reported an internal error. Retry in a moment.",
    );
    expect(mapError(new ApiError(500, "unexpected", "unexpected"), "fallback")).toBe(
      "Request failed (unexpected).",
    );
  });

  it("maps voice-join failures for token expiry and rtc failures", () => {
    expect(
      mapVoiceJoinError(new ApiError(401, "invalid_credentials", "invalid_credentials")),
    ).toContain("Refresh session or login again");

    expect(mapVoiceJoinError(new RtcClientError("join_failed", "token expired during join"))).toBe(
      "Voice token expired before signaling completed. Select Join Voice to request a fresh token.",
    );

    expect(mapVoiceJoinError(new RtcClientError("join_failed", "failed transport"))).toBe(
      "Voice connection failed. Verify LiveKit signaling reachability and retry.",
    );
  });

  it("orders merged messages chronologically and avoids duplicate history IDs", () => {
    const first = buildMessage({ messageId: MESSAGE_ID_1, content: "first", createdAtUnix: 10 });
    const second = buildMessage({ messageId: MESSAGE_ID_2, content: "second", createdAtUnix: 20 });
    const secondReplacement = buildMessage({
      messageId: MESSAGE_ID_2,
      content: "second-new",
      createdAtUnix: 20,
    });
    const third = buildMessage({ messageId: MESSAGE_ID_3, content: "third", createdAtUnix: 30 });

    const withUpdate = mergeMessage([second, first], secondReplacement);
    expect(withUpdate.map((message) => message.content)).toEqual(["first", "second-new"]);

    const historyMerged = mergeMessageHistory([second, third], [first, secondReplacement]);
    expect(historyMerged.map((message) => message.content)).toEqual(["first", "second", "third"]);
  });

  it("merges reaction snapshots for targeted messages and preserves local reacted flags", () => {
    const messageId = messageIdFromInput(MESSAGE_ID_1);
    const thumbsUp = reactionEmojiFromInput("👍");
    const fire = reactionEmojiFromInput("🔥");
    const stale = reactionEmojiFromInput("✅");

    const existing = {
      [reactionKey(messageId, thumbsUp)]: { count: 1, reacted: true },
      [reactionKey(messageIdFromInput(MESSAGE_ID_2), stale)]: { count: 4, reacted: false },
    };
    const messages = [
      buildMessage({
        messageId: MESSAGE_ID_1,
        content: "updated",
        createdAtUnix: 10,
        reactions: [
          { emoji: thumbsUp, count: 3 },
          { emoji: fire, count: 2 },
        ],
      }),
    ];

    expect(mergeReactionStateFromMessages(existing, messages)).toEqual({
      [reactionKey(messageId, thumbsUp)]: { count: 3, reacted: true },
      [reactionKey(messageId, fire)]: { count: 2, reacted: false },
      [reactionKey(messageIdFromInput(MESSAGE_ID_2), stale)]: { count: 4, reacted: false },
    });
  });

  it("replaces reaction state from snapshots and drops stale message keys", () => {
    const messageId = messageIdFromInput(MESSAGE_ID_1);
    const thumbsUp = reactionEmojiFromInput("👍");
    const stale = reactionEmojiFromInput("✅");

    const existing = {
      [reactionKey(messageId, thumbsUp)]: { count: 1, reacted: true },
      [reactionKey(messageIdFromInput(MESSAGE_ID_2), stale)]: { count: 4, reacted: false },
    };
    const messages = [
      buildMessage({
        messageId: MESSAGE_ID_1,
        content: "latest",
        createdAtUnix: 20,
        reactions: [{ emoji: thumbsUp, count: 5 }],
      }),
    ];

    expect(replaceReactionStateFromMessages(existing, messages)).toEqual({
      [reactionKey(messageId, thumbsUp)]: { count: 5, reacted: true },
    });
  });

  it("renders markdown tokens into safe display text", () => {
    const output = tokenizeToDisplayText([
      { type: "text", text: "hello" },
      { type: "soft_break" },
      { type: "list_item_start" },
      { type: "text", text: "item" },
      { type: "list_item_end" },
      { type: "link_start", href: "https://filament.local" },
      { type: "text", text: "docs" },
      { type: "link_end" },
      { type: "fenced_code", language: "rust", code: "fn main() {}" },
    ]);

    expect(output).toContain("hello");
    expect(output).toContain("item");
    expect(output).toContain("(https://filament.local)");
    expect(output).toContain("```rust");
    expect(output).toContain("fn main() {}");
  });

  it("parses permission CSV with dedupe and validation", () => {
    const parsed = parsePermissionCsv("create_message, subscribe_streams, create_message");
    expect(parsed).toEqual(["create_message", "subscribe_streams"]);
    expect(() => parsePermissionCsv("invalid_permission")).toThrow(DomainValidationError);
  });

  it("parses user identity labels safely", () => {
    const parsed = userIdFromVoiceIdentity(`u.${USER_ID}.desktop`);
    expect(parsed).toBe(userIdFromInput(USER_ID));
    expect(userIdFromVoiceIdentity(`u.not-a-ulid.desktop`)).toBeNull();
    expect(userIdFromVoiceIdentity("something-else")).toBeNull();
  });

  it("keeps channel label rules for text and voice", () => {
    expect(channelRailLabel({ kind: "text", name: "general" })).toBe("#general");
    expect(channelRailLabel({ kind: "voice", name: "briefing" })).toBe("briefing");
  });
});
