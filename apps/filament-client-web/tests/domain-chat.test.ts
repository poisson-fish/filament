import {
  channelIdFromInput,
  guildIdFromInput,
  messageContentFromInput,
  messageFromResponse,
  searchQueryFromInput,
  workspaceFromStorage,
} from "../src/domain/chat";

describe("chat domain invariants", () => {
  it("accepts ULID ids", () => {
    const ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    expect(guildIdFromInput(ulid)).toBe(ulid);
    expect(channelIdFromInput(ulid)).toBe(ulid);
  });

  it("rejects invalid ids", () => {
    expect(() => guildIdFromInput("not-ulid")).toThrow();
  });

  it("rejects oversized message content", () => {
    expect(() => messageContentFromInput("A".repeat(2001))).toThrow();
  });

  it("maps message payloads into validated records", () => {
    const message = messageFromResponse({
      message_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      author_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      content: "hello",
      created_at_unix: 1,
    });

    expect(message.content).toBe("hello");
  });

  it("enforces search query policy", () => {
    expect(searchQueryFromInput("needle")).toBe("needle");
    expect(() => searchQueryFromInput("content:hello")).toThrow();
  });

  it("validates workspace cache payloads", () => {
    const workspace = workspaceFromStorage({
      guildId: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      guildName: "Security",
      channels: [{ channelId: "01ARZ3NDEKTSV4RRFFQ69G5FAV", name: "incident-room" }],
    });

    expect(workspace.channels[0]?.name).toBe("incident-room");
  });
});
