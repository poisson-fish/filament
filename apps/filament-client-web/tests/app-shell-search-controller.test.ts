import { createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  channelIdFromInput,
  guildIdFromInput,
  searchResultsFromResponse,
  type SearchResults,
} from "../src/domain/chat";
import { createSearchController } from "../src/features/app-shell/controllers/search-controller";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});
const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const MESSAGE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

function searchFixture(): SearchResults {
  return searchResultsFromResponse({
    message_ids: [MESSAGE_ID],
    messages: [
      {
        message_id: MESSAGE_ID,
        guild_id: GUILD_ID,
        channel_id: CHANNEL_ID,
        author_id: USER_ID,
        content: "incident logs",
        markdown_tokens: [{ type: "text", text: "incident logs" }],
        attachments: [],
        created_at_unix: 1,
      },
    ],
  });
}

describe("app shell search controller", () => {
  it("runs search and maintenance actions through controller state wiring", async () => {
    const [session] = createSignal(SESSION);
    const [activeGuildId] = createSignal(GUILD_ID);
    const [activeChannelId] = createSignal(CHANNEL_ID);
    const [searchQuery] = createSignal(" incident ");
    const [isSearching, setSearching] = createSignal(false);
    const [searchError, setSearchError] = createSignal("");
    const [searchResults, setSearchResults] = createSignal<SearchResults | null>(null);
    const [isRunningSearchOps, setRunningSearchOps] = createSignal(false);
    const [searchOpsStatus, setSearchOpsStatus] = createSignal("");

    const results = searchFixture();
    const searchGuildMessagesMock = vi.fn(async () => results);
    const rebuildGuildSearchIndexMock = vi.fn(async () => undefined);
    const reconcileGuildSearchIndexMock = vi.fn(async () => ({ upserted: 2, deleted: 1 }));

    const controller = createSearchController(
      {
        session,
        activeGuildId,
        activeChannelId,
        searchQuery,
        isSearching,
        setSearching,
        setSearchError,
        setSearchResults,
        isRunningSearchOps,
        setRunningSearchOps,
        setSearchOpsStatus,
      },
      {
        searchGuildMessages: searchGuildMessagesMock,
        rebuildGuildSearchIndex: rebuildGuildSearchIndexMock,
        reconcileGuildSearchIndex: reconcileGuildSearchIndexMock,
      },
    );

    const submitEvent = {
      preventDefault: vi.fn(),
    } as unknown as SubmitEvent;

    await controller.runSearch(submitEvent);

    expect(submitEvent.preventDefault).toHaveBeenCalledTimes(1);
    expect(searchGuildMessagesMock).toHaveBeenCalledTimes(1);
    expect(searchGuildMessagesMock).toHaveBeenCalledWith(SESSION, GUILD_ID, {
      query: "incident",
      limit: 20,
      channelId: CHANNEL_ID,
    });
    expect(searchResults()).toEqual(results);
    expect(searchError()).toBe("");
    expect(isSearching()).toBe(false);

    await controller.rebuildSearch();
    expect(rebuildGuildSearchIndexMock).toHaveBeenCalledWith(SESSION, GUILD_ID);
    expect(searchOpsStatus()).toBe("Search index rebuild queued.");
    expect(isRunningSearchOps()).toBe(false);

    await controller.reconcileSearch();
    expect(reconcileGuildSearchIndexMock).toHaveBeenCalledWith(SESSION, GUILD_ID);
    expect(searchOpsStatus()).toBe(
      "Reconciled search index (upserted 2, deleted 1).",
    );
    expect(isRunningSearchOps()).toBe(false);
  });
});
