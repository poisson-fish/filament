import { describe, expect, it, vi } from "vitest";
import { createSearchPanelProps } from "../src/features/app-shell/runtime/search-panel-props";

describe("app shell search panel props", () => {
  it("maps search values and handlers", async () => {
    const onSubmitSearch = vi.fn();
    const setSearchQuery = vi.fn();
    const onRebuildSearch = vi.fn();
    const onReconcileSearch = vi.fn();
    const displayUserLabel = vi.fn((userId: string) => `@${userId}`);

    const panelProps = createSearchPanelProps({
      searchQuery: "hello",
      isSearching: false,
      hasActiveWorkspace: true,
      canManageSearchMaintenance: true,
      isRunningSearchOps: false,
      searchOpsStatus: "idle",
      searchError: "",
      searchResults: null,
      onSubmitSearch,
      setSearchQuery,
      onRebuildSearch,
      onReconcileSearch,
      displayUserLabel,
    });

    expect(panelProps.searchQuery).toBe("hello");
    expect(panelProps.hasActiveWorkspace).toBe(true);
    expect(panelProps.canManageSearchMaintenance).toBe(true);

    const submitEvent = {
      preventDefault: vi.fn(),
    } as unknown as SubmitEvent;

    await panelProps.onSubmitSearch(submitEvent);
    expect(onSubmitSearch).toHaveBeenCalledWith(submitEvent);

    panelProps.setSearchQuery("world");
    expect(setSearchQuery).toHaveBeenCalledWith("world");

    await panelProps.onRebuildSearch();
    expect(onRebuildSearch).toHaveBeenCalledTimes(1);

    await panelProps.onReconcileSearch();
    expect(onReconcileSearch).toHaveBeenCalledTimes(1);

    expect(panelProps.displayUserLabel("01ARZ3NDEKTSV4RRFFQ69G5FAA")).toBe(
      "@01ARZ3NDEKTSV4RRFFQ69G5FAA",
    );
  });
});