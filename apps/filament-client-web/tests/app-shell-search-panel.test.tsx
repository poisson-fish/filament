import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  channelIdFromInput,
  guildIdFromInput,
  searchResultsFromResponse,
} from "../src/domain/chat";
import {
  SearchPanel,
  type SearchPanelProps,
} from "../src/features/app-shell/components/panels/SearchPanel";

const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const MESSAGE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";
const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAY";

function searchPanelPropsFixture(
  overrides: Partial<SearchPanelProps> = {},
): SearchPanelProps {
  return {
    searchQuery: "incident",
    isSearching: false,
    hasActiveWorkspace: true,
    canManageSearchMaintenance: true,
    isRunningSearchOps: false,
    searchOpsStatus: "",
    searchError: "",
    searchResults: searchResultsFromResponse({
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
    }),
    onSubmitSearch: vi.fn((event: SubmitEvent) => event.preventDefault()),
    onSearchQueryInput: vi.fn(),
    onRebuildSearch: vi.fn(),
    onReconcileSearch: vi.fn(),
    displayUserLabel: (userId) => (userId === USER_ID ? "Remote User" : userId),
    ...overrides,
  };
}

describe("app shell search panel", () => {
  it("renders result rows with utility presence dots and no legacy presence hook", () => {
    render(() => <SearchPanel {...searchPanelPropsFixture()} />);

    const resultText = screen.getByText("Remote User: incident logs");
    expect(resultText).toBeInTheDocument();
    expect(resultText.closest("ul")).toHaveClass("list-none");
    expect(resultText.closest("li")).toHaveClass("rounded-[0.6rem]");
    expect(resultText.closest("li")?.querySelector("span.bg-presence-online")).not.toBeNull();
    expect(document.querySelector(".presence")).toBeNull();
  });

  it("keeps search and maintenance handlers wired", async () => {
    const onSubmitSearch = vi.fn((event: SubmitEvent) => event.preventDefault());
    const onSearchQueryInput = vi.fn();
    const onRebuildSearch = vi.fn();
    const onReconcileSearch = vi.fn();

    render(() => (
      <SearchPanel
        {...searchPanelPropsFixture({
          onSubmitSearch,
          onSearchQueryInput,
          onRebuildSearch,
          onReconcileSearch,
          searchResults: null,
        })}
      />
    ));

    await fireEvent.input(screen.getByLabelText("Query"), {
      target: { value: "ops" },
    });
    expect(onSearchQueryInput).toHaveBeenCalledWith("ops");

    const searchForm = screen.getByRole("button", { name: "Search" }).closest("form");
    expect(searchForm).not.toBeNull();
    await fireEvent.submit(searchForm!);
    expect(onSubmitSearch).toHaveBeenCalledTimes(1);

    await fireEvent.click(screen.getByRole("button", { name: "Rebuild Index" }));
    await fireEvent.click(screen.getByRole("button", { name: "Reconcile Index" }));
    expect(onRebuildSearch).toHaveBeenCalledTimes(1);
    expect(onReconcileSearch).toHaveBeenCalledTimes(1);
  });
});
