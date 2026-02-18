import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import { guildIdFromInput, guildNameFromInput } from "../src/domain/chat";
import {
  PublicDirectoryPanel,
  type PublicDirectoryPanelProps,
} from "../src/features/app-shell/components/panels/PublicDirectoryPanel";

const PRIMARY_GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const SECONDARY_GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");

function publicDirectoryPanelPropsFixture(
  overrides: Partial<PublicDirectoryPanelProps> = {},
): PublicDirectoryPanelProps {
  return {
    searchQuery: "filament",
    isSearching: false,
    searchError: "",
    guilds: [
      {
        guildId: PRIMARY_GUILD_ID,
        name: guildNameFromInput("Town Hall"),
        visibility: "public",
      },
      {
        guildId: SECONDARY_GUILD_ID,
        name: guildNameFromInput("Incident Response"),
        visibility: "public",
      },
    ],
    joinStatusByGuildId: {
      [PRIMARY_GUILD_ID]: "joined",
      [SECONDARY_GUILD_ID]: "idle",
    },
    joinErrorByGuildId: {
      [PRIMARY_GUILD_ID]: "",
      [SECONDARY_GUILD_ID]: "",
    },
    onSubmitSearch: vi.fn((event: SubmitEvent) => event.preventDefault()),
    onJoinGuild: vi.fn(),
    onSearchInput: vi.fn(),
    ...overrides,
  };
}

describe("app shell public directory panel", () => {
  it("renders with Uno utility classes and without legacy internal class hooks", () => {
    render(() => <PublicDirectoryPanel {...publicDirectoryPanelPropsFixture()} />);

    const panel = screen.getByLabelText("public-workspace-directory");
    expect(panel).toHaveClass("public-directory");
    expect(panel).toHaveClass("grid");

    const searchInput = screen.getByLabelText("Search");
    expect(searchInput).toHaveClass("rounded-[0.62rem]");
    expect(searchInput).toHaveClass("border-line-soft");

    const joinButton = screen.getByRole("button", { name: "Joined" });
    expect(joinButton).toBeDisabled();
    expect(joinButton).toHaveClass("border-brand/45");

    const joinedChip = screen.getByText("joined");
    expect(joinedChip).toHaveClass("border-ok/80");
    expect(joinedChip).toHaveClass("text-ok");

    expect(document.querySelector(".inline-form")).toBeNull();
    expect(document.querySelector(".public-directory-row")).toBeNull();
    expect(document.querySelector(".public-directory-row-main")).toBeNull();
    expect(document.querySelector(".public-directory-row-actions")).toBeNull();
    expect(document.querySelector(".directory-status-chip")).toBeNull();
    expect(document.querySelector(".public-directory-row-error")).toBeNull();
  });

  it("keeps search and join actions wired", async () => {
    const onSubmitSearch = vi.fn((event: SubmitEvent) => event.preventDefault());
    const onJoinGuild = vi.fn();
    const onSearchInput = vi.fn();

    render(() => (
      <PublicDirectoryPanel
        {...publicDirectoryPanelPropsFixture({
          joinStatusByGuildId: {
            [PRIMARY_GUILD_ID]: "idle",
          },
          guilds: [
            {
              guildId: PRIMARY_GUILD_ID,
              name: guildNameFromInput("Town Hall"),
              visibility: "public",
            },
          ],
          onSubmitSearch,
          onJoinGuild,
          onSearchInput,
        })}
      />
    ));

    await fireEvent.input(screen.getByLabelText("Search"), {
      target: { value: "ops" },
    });
    expect(onSearchInput).toHaveBeenCalledWith("ops");

    const searchForm = screen.getByRole("button", { name: "Find public" }).closest("form");
    expect(searchForm).not.toBeNull();
    await fireEvent.submit(searchForm!);
    expect(onSubmitSearch).toHaveBeenCalledTimes(1);

    await fireEvent.click(screen.getByRole("button", { name: "Join" }));
    expect(onJoinGuild).toHaveBeenCalledWith(PRIMARY_GUILD_ID);
  });
});
