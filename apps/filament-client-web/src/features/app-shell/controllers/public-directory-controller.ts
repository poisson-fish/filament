import { createEffect, untrack, type Accessor, type Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import type { GuildRecord } from "../../../domain/chat";
import { fetchPublicGuildDirectory } from "../../../lib/api";
import { mapError } from "../helpers";

export interface PublicDirectoryControllerOptions {
  session: Accessor<AuthSession | null>;
  publicGuildSearchQuery: Accessor<string>;
  isSearchingPublicGuilds: Accessor<boolean>;
  setSearchingPublicGuilds: Setter<boolean>;
  setPublicGuildSearchError: Setter<string>;
  setPublicGuildDirectory: Setter<GuildRecord[]>;
}

export interface PublicDirectoryControllerDependencies {
  fetchPublicGuildDirectory: typeof fetchPublicGuildDirectory;
}

export interface PublicDirectoryController {
  loadPublicGuildDirectory: (query?: string) => Promise<void>;
  runPublicGuildSearch: (event: SubmitEvent) => Promise<void>;
}

const DEFAULT_PUBLIC_DIRECTORY_CONTROLLER_DEPENDENCIES: PublicDirectoryControllerDependencies = {
  fetchPublicGuildDirectory,
};

export function createPublicDirectoryController(
  options: PublicDirectoryControllerOptions,
  dependencies: Partial<PublicDirectoryControllerDependencies> = {},
): PublicDirectoryController {
  const deps = {
    ...DEFAULT_PUBLIC_DIRECTORY_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };
  let directoryRequestVersion = 0;

  const loadPublicGuildDirectory = async (query?: string): Promise<void> => {
    const session = options.session();
    if (!session) {
      options.setPublicGuildDirectory([]);
      return;
    }
    if (options.isSearchingPublicGuilds()) {
      return;
    }
    const requestVersion = ++directoryRequestVersion;
    options.setSearchingPublicGuilds(true);
    options.setPublicGuildSearchError("");
    try {
      const directory = await deps.fetchPublicGuildDirectory(session, {
        query,
        limit: 20,
      });
      if (requestVersion !== directoryRequestVersion) {
        return;
      }
      options.setPublicGuildDirectory(directory.guilds);
    } catch (error) {
      if (requestVersion !== directoryRequestVersion) {
        return;
      }
      options.setPublicGuildSearchError(
        mapError(error, "Unable to load public workspace directory."),
      );
      options.setPublicGuildDirectory([]);
    } finally {
      if (requestVersion === directoryRequestVersion) {
        options.setSearchingPublicGuilds(false);
      }
    }
  };

  const runPublicGuildSearch = async (event: SubmitEvent): Promise<void> => {
    event.preventDefault();
    await loadPublicGuildDirectory(options.publicGuildSearchQuery());
  };

  createEffect(() => {
    const session = options.session();
    directoryRequestVersion += 1;
    if (!session) {
      options.setSearchingPublicGuilds(false);
      options.setPublicGuildDirectory([]);
      options.setPublicGuildSearchError("");
      return;
    }
    void untrack(() => loadPublicGuildDirectory());
  });

  return {
    loadPublicGuildDirectory,
    runPublicGuildSearch,
  };
}
