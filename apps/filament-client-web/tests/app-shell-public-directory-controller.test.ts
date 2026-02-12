import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import { guildFromResponse } from "../src/domain/chat";
import { createPublicDirectoryController } from "../src/features/app-shell/controllers/public-directory-controller";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

function guildFixture(id: string, name: string) {
  return guildFromResponse({
    guild_id: id,
    name,
    visibility: "public",
  });
}

function deferred<T>() {
  let resolve: ((value: T) => void) | undefined;
  let reject: ((reason?: unknown) => void) | undefined;
  const promise = new Promise<T>((innerResolve, innerReject) => {
    resolve = innerResolve;
    reject = innerReject;
  });
  return {
    promise,
    resolve: (value: T) => resolve?.(value),
    reject: (reason?: unknown) => reject?.(reason),
  };
}

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe("app shell public directory controller", () => {
  it("loads public directory on auth and resets state on logout", async () => {
    const [session, setSession] = createSignal<typeof SESSION | null>(SESSION);
    const [publicGuildSearchQuery] = createSignal("");
    const [isSearchingPublicGuilds, setSearchingPublicGuilds] = createSignal(false);
    const [publicGuildSearchError, setPublicGuildSearchError] = createSignal("");
    const [publicGuildDirectory, setPublicGuildDirectory] = createSignal<ReturnType<
      typeof guildFixture
    >[]>([]);

    const fetchPublicGuildDirectoryMock = vi.fn(async () => ({
      guilds: [guildFixture("01ARZ3NDEKTSV4RRFFQ69G5FAA", "Town Hall")],
    }));

    const dispose = createRoot((rootDispose) => {
      createPublicDirectoryController(
        {
          session,
          publicGuildSearchQuery,
          isSearchingPublicGuilds,
          setSearchingPublicGuilds,
          setPublicGuildSearchError,
          setPublicGuildDirectory,
        },
        {
          fetchPublicGuildDirectory: fetchPublicGuildDirectoryMock,
        },
      );
      return rootDispose;
    });

    await flush();
    expect(fetchPublicGuildDirectoryMock).toHaveBeenCalledTimes(1);
    expect(publicGuildDirectory()).toEqual([
      guildFixture("01ARZ3NDEKTSV4RRFFQ69G5FAA", "Town Hall"),
    ]);

    setSession(null);
    await flush();
    expect(publicGuildDirectory()).toEqual([]);
    expect(publicGuildSearchError()).toBe("");
    expect(isSearchingPublicGuilds()).toBe(false);

    dispose();
  });

  it("drops stale directory responses after auth reset", async () => {
    const [session, setSession] = createSignal<typeof SESSION | null>(SESSION);
    const [publicGuildSearchQuery] = createSignal("lobby");
    const [isSearchingPublicGuilds, setSearchingPublicGuilds] = createSignal(false);
    const [publicGuildSearchError, setPublicGuildSearchError] = createSignal("");
    const [publicGuildDirectory, setPublicGuildDirectory] = createSignal<ReturnType<
      typeof guildFixture
    >[]>([]);

    const pendingDirectory = deferred<{
      guilds: ReturnType<typeof guildFixture>[];
    }>();
    const fetchPublicGuildDirectoryMock = vi.fn(() => pendingDirectory.promise);

    const dispose = createRoot((rootDispose) => {
      createPublicDirectoryController(
        {
          session,
          publicGuildSearchQuery,
          isSearchingPublicGuilds,
          setSearchingPublicGuilds,
          setPublicGuildSearchError,
          setPublicGuildDirectory,
        },
        {
          fetchPublicGuildDirectory: fetchPublicGuildDirectoryMock,
        },
      );
      return rootDispose;
    });

    await flush();
    setSession(null);
    pendingDirectory.resolve({
      guilds: [guildFixture("01ARZ3NDEKTSV4RRFFQ69G5FAB", "Public Lobby")],
    });
    await flush();

    expect(publicGuildDirectory()).toEqual([]);
    expect(isSearchingPublicGuilds()).toBe(false);
    expect(publicGuildSearchError()).toBe("");

    dispose();
  });
});
