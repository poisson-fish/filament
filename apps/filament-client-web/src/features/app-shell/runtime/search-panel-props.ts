import type { SearchPanelBuilderOptions } from "../adapters/panel-host-props";

export interface SearchPanelPropsOptions {
  searchQuery: string;
  isSearching: boolean;
  hasActiveWorkspace: boolean;
  canManageSearchMaintenance: boolean;
  isRunningSearchOps: boolean;
  searchOpsStatus: string;
  searchError: string;
  searchResults: SearchPanelBuilderOptions["searchResults"];
  onSubmitSearch: (event: SubmitEvent) => Promise<void> | void;
  setSearchQuery: (value: string) => void;
  onRebuildSearch: () => Promise<void> | void;
  onReconcileSearch: () => Promise<void> | void;
  displayUserLabel: (userId: string) => string;
}

export function createSearchPanelProps(
  options: SearchPanelPropsOptions,
): SearchPanelBuilderOptions {
  return {
    searchQuery: options.searchQuery,
    isSearching: options.isSearching,
    hasActiveWorkspace: options.hasActiveWorkspace,
    canManageSearchMaintenance: options.canManageSearchMaintenance,
    isRunningSearchOps: options.isRunningSearchOps,
    searchOpsStatus: options.searchOpsStatus,
    searchError: options.searchError,
    searchResults: options.searchResults,
    onSubmitSearch: options.onSubmitSearch,
    setSearchQuery: options.setSearchQuery,
    onRebuildSearch: options.onRebuildSearch,
    onReconcileSearch: options.onReconcileSearch,
    displayUserLabel: options.displayUserLabel,
  };
}