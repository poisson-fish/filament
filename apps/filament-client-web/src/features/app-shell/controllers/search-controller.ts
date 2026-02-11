import type { Accessor, Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import type {
  ChannelId,
  GuildId,
  SearchResults,
} from "../../../domain/chat";
import { searchQueryFromInput } from "../../../domain/chat";
import {
  rebuildGuildSearchIndex,
  reconcileGuildSearchIndex,
  searchGuildMessages,
} from "../../../lib/api";
import { mapError } from "../helpers";

export interface SearchControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  searchQuery: Accessor<string>;
  isSearching: Accessor<boolean>;
  setSearching: Setter<boolean>;
  setSearchError: Setter<string>;
  setSearchResults: Setter<SearchResults | null>;
  isRunningSearchOps: Accessor<boolean>;
  setRunningSearchOps: Setter<boolean>;
  setSearchOpsStatus: Setter<string>;
}

export interface SearchControllerDependencies {
  searchGuildMessages: typeof searchGuildMessages;
  rebuildGuildSearchIndex: typeof rebuildGuildSearchIndex;
  reconcileGuildSearchIndex: typeof reconcileGuildSearchIndex;
}

export interface SearchController {
  runSearch: (event: SubmitEvent) => Promise<void>;
  rebuildSearch: () => Promise<void>;
  reconcileSearch: () => Promise<void>;
}

const DEFAULT_SEARCH_CONTROLLER_DEPENDENCIES: SearchControllerDependencies = {
  searchGuildMessages,
  rebuildGuildSearchIndex,
  reconcileGuildSearchIndex,
};

export function createSearchController(
  options: SearchControllerOptions,
  dependencies: Partial<SearchControllerDependencies> = {},
): SearchController {
  const deps = {
    ...DEFAULT_SEARCH_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };

  const runSearch = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId) {
      options.setSearchError("Select a workspace first.");
      return;
    }

    if (options.isSearching()) {
      return;
    }

    options.setSearching(true);
    options.setSearchError("");
    try {
      const results = await deps.searchGuildMessages(session, guildId, {
        query: searchQueryFromInput(options.searchQuery()),
        limit: 20,
        channelId: options.activeChannelId() ?? undefined,
      });
      options.setSearchResults(results);
    } catch (error) {
      options.setSearchError(mapError(error, "Search request failed."));
      options.setSearchResults(null);
    } finally {
      options.setSearching(false);
    }
  };

  const rebuildSearch = async () => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId || options.isRunningSearchOps()) {
      return;
    }

    options.setRunningSearchOps(true);
    options.setSearchError("");
    options.setSearchOpsStatus("");
    try {
      await deps.rebuildGuildSearchIndex(session, guildId);
      options.setSearchOpsStatus("Search index rebuild queued.");
    } catch (error) {
      options.setSearchError(mapError(error, "Unable to rebuild search index."));
    } finally {
      options.setRunningSearchOps(false);
    }
  };

  const reconcileSearch = async () => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId || options.isRunningSearchOps()) {
      return;
    }

    options.setRunningSearchOps(true);
    options.setSearchError("");
    options.setSearchOpsStatus("");
    try {
      const result = await deps.reconcileGuildSearchIndex(session, guildId);
      options.setSearchOpsStatus(
        `Reconciled search index (upserted ${result.upserted}, deleted ${result.deleted}).`,
      );
    } catch (error) {
      options.setSearchError(mapError(error, "Unable to reconcile search index."));
    } finally {
      options.setRunningSearchOps(false);
    }
  };

  return {
    runSearch,
    rebuildSearch,
    reconcileSearch,
  };
}
