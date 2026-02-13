import { describe, expect, it, vi } from "vitest";
import { guildIdFromInput, guildNameFromInput } from "../src/domain/chat";
import { createPublicDirectoryPanelProps } from "../src/features/app-shell/runtime/public-directory-panel-props";

describe("app shell public directory panel props", () => {
  it("maps public directory values and handlers", async () => {
    const onSubmitPublicGuildSearch = vi.fn();
    const onJoinGuildFromDirectory = vi.fn();
    const setPublicGuildSearchQuery = vi.fn();
    const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");

    const panelProps = createPublicDirectoryPanelProps({
      publicGuildSearchQuery: "filament",
      isSearchingPublicGuilds: false,
      publicGuildSearchError: "",
      publicGuildDirectory: [
        {
          guildId,
          name: guildNameFromInput("Filament Guild"),
          visibility: "public",
        },
      ],
      publicGuildJoinStatusByGuildId: {
        [guildId]: "idle",
      },
      publicGuildJoinErrorByGuildId: {
        [guildId]: "",
      },
      onSubmitPublicGuildSearch,
      onJoinGuildFromDirectory,
      setPublicGuildSearchQuery,
    });

    expect(panelProps.publicGuildSearchQuery).toBe("filament");
    expect(panelProps.isSearchingPublicGuilds).toBe(false);
    expect(panelProps.publicGuildDirectory).toHaveLength(1);

    const submitEvent = {
      preventDefault: vi.fn(),
    } as unknown as SubmitEvent;

    await panelProps.onSubmitPublicGuildSearch(submitEvent);
    expect(onSubmitPublicGuildSearch).toHaveBeenCalledWith(submitEvent);

    await panelProps.onJoinGuildFromDirectory(guildId);
    expect(onJoinGuildFromDirectory).toHaveBeenCalledWith(guildId);

    panelProps.setPublicGuildSearchQuery("security");
    expect(setPublicGuildSearchQuery).toHaveBeenCalledWith("security");
  });
});
