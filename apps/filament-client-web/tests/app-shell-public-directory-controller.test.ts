import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  channelFromResponse,
  channelIdFromInput,
  guildFromResponse,
  guildIdFromInput,
  guildNameFromInput,
  type WorkspaceRecord,
} from "../src/domain/chat";
import { ApiError } from "../src/lib/api";
import { createPublicDirectoryController } from "../src/features/app-shell/controllers/public-directory-controller";
import type { PublicDirectoryJoinStatus } from "../src/features/app-shell/types";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

const GUILD_A = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAA");
const GUILD_B = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAB");
const CHANNEL_A = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAC");
const CHANNEL_B = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAD");

function guildFixture(id: string, name: string) {
  return guildFromResponse({
    guild_id: id,
    name,
    visibility: "public",
  });
}

function channelFixture(id: string, name: string) {
  return channelFromResponse({
    channel_id: id,
    name,
    kind: "text",
  });
}

function workspaceFixture(input: {
  guildId: string;
  guildName: string;
  visibility?: "public" | "private";
  channelId: string;
  channelName: string;
}): WorkspaceRecord {
  return {
    guildId: guildIdFromInput(input.guildId),
    guildName: guildNameFromInput(input.guildName),
    visibility: input.visibility ?? "private",
    channels: [channelFixture(input.channelId, input.channelName)],
  };
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
    const [activeGuildId, setActiveGuildId] = createSignal<typeof GUILD_A | null>(GUILD_A);
    const [activeChannelId, setActiveChannelId] = createSignal<typeof CHANNEL_A | null>(CHANNEL_A);
    const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([
      workspaceFixture({
        guildId: GUILD_A,
        guildName: "Member Guild",
        channelId: CHANNEL_A,
        channelName: "incident-room",
      }),
    ]);
    const [publicGuildSearchQuery] = createSignal("");
    const [isSearchingPublicGuilds, setSearchingPublicGuilds] = createSignal(false);
    const [publicGuildSearchError, setPublicGuildSearchError] = createSignal("");
    const [publicGuildDirectory, setPublicGuildDirectory] = createSignal<ReturnType<
      typeof guildFixture
    >[]>([]);
    const [publicGuildJoinStatusByGuildId, setPublicGuildJoinStatusByGuildId] = createSignal<
      Record<string, PublicDirectoryJoinStatus>
    >({
      [GUILD_B]: "joined",
    });
    const [publicGuildJoinErrorByGuildId, setPublicGuildJoinErrorByGuildId] = createSignal<
      Record<string, string>
    >({
      [GUILD_B]: "joined-before-reset",
    });

    const fetchPublicGuildDirectoryMock = vi.fn(async () => ({
      guilds: [guildFixture("01ARZ3NDEKTSV4RRFFQ69G5FAE", "Town Hall")],
    }));

    const dispose = createRoot((rootDispose) => {
      createPublicDirectoryController(
        {
          session,
          activeGuildId,
          activeChannelId,
          publicGuildSearchQuery,
          isSearchingPublicGuilds,
          publicGuildJoinStatusByGuildId,
          setSearchingPublicGuilds,
          setPublicGuildSearchError,
          setPublicGuildDirectory,
          setPublicGuildJoinStatusByGuildId,
          setPublicGuildJoinErrorByGuildId,
          setWorkspaces,
          setActiveGuildId,
          setActiveChannelId,
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
      guildFixture("01ARZ3NDEKTSV4RRFFQ69G5FAE", "Town Hall"),
    ]);

    setSession(null);
    await flush();
    expect(publicGuildDirectory()).toEqual([]);
    expect(publicGuildSearchError()).toBe("");
    expect(isSearchingPublicGuilds()).toBe(false);
    expect(publicGuildJoinStatusByGuildId()).toEqual({});
    expect(publicGuildJoinErrorByGuildId()).toEqual({});
    expect(workspaces()).toEqual([
      workspaceFixture({
        guildId: GUILD_A,
        guildName: "Member Guild",
        channelId: CHANNEL_A,
        channelName: "incident-room",
      }),
    ]);

    dispose();
  });

  it("drops stale directory responses after auth reset", async () => {
    const [session, setSession] = createSignal<typeof SESSION | null>(SESSION);
    const [activeGuildId, setActiveGuildId] = createSignal<typeof GUILD_A | null>(GUILD_A);
    const [activeChannelId, setActiveChannelId] = createSignal<typeof CHANNEL_A | null>(CHANNEL_A);
    const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([]);
    const [publicGuildSearchQuery] = createSignal("lobby");
    const [isSearchingPublicGuilds, setSearchingPublicGuilds] = createSignal(false);
    const [publicGuildSearchError, setPublicGuildSearchError] = createSignal("");
    const [publicGuildDirectory, setPublicGuildDirectory] = createSignal<ReturnType<
      typeof guildFixture
    >[]>([]);
    const [publicGuildJoinStatusByGuildId, setPublicGuildJoinStatusByGuildId] = createSignal<
      Record<string, PublicDirectoryJoinStatus>
    >({});
    const [publicGuildJoinErrorByGuildId, setPublicGuildJoinErrorByGuildId] = createSignal<
      Record<string, string>
    >({});

    const pendingDirectory = deferred<{
      guilds: ReturnType<typeof guildFixture>[];
    }>();
    const fetchPublicGuildDirectoryMock = vi.fn(() => pendingDirectory.promise);

    const dispose = createRoot((rootDispose) => {
      createPublicDirectoryController(
        {
          session,
          activeGuildId,
          activeChannelId,
          publicGuildSearchQuery,
          isSearchingPublicGuilds,
          publicGuildJoinStatusByGuildId,
          setSearchingPublicGuilds,
          setPublicGuildSearchError,
          setPublicGuildDirectory,
          setPublicGuildJoinStatusByGuildId,
          setPublicGuildJoinErrorByGuildId,
          setWorkspaces,
          setActiveGuildId,
          setActiveChannelId,
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
      guilds: [guildFixture("01ARZ3NDEKTSV4RRFFQ69G5FAF", "Public Lobby")],
    });
    await flush();

    expect(publicGuildDirectory()).toEqual([]);
    expect(isSearchingPublicGuilds()).toBe(false);
    expect(publicGuildSearchError()).toBe("");
    expect(workspaces()).toEqual([]);

    dispose();
  });

  it("joins a workspace and refreshes workspace/channel state while preserving active selection", async () => {
    const [session] = createSignal<typeof SESSION | null>(SESSION);
    const [activeGuildId, setActiveGuildId] = createSignal<typeof GUILD_A | null>(GUILD_A);
    const [activeChannelId, setActiveChannelId] = createSignal<typeof CHANNEL_A | null>(CHANNEL_A);
    const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([
      workspaceFixture({
        guildId: GUILD_A,
        guildName: "Member Guild",
        channelId: CHANNEL_A,
        channelName: "incident-room",
      }),
    ]);
    const [publicGuildSearchQuery] = createSignal("");
    const [isSearchingPublicGuilds, setSearchingPublicGuilds] = createSignal(false);
    const [publicGuildSearchError, setPublicGuildSearchError] = createSignal("");
    const [publicGuildDirectory, setPublicGuildDirectory] = createSignal<ReturnType<
      typeof guildFixture
    >[]>([guildFixture(GUILD_B, "Town Hall")]);
    const [publicGuildJoinStatusByGuildId, setPublicGuildJoinStatusByGuildId] = createSignal<
      Record<string, PublicDirectoryJoinStatus>
    >({});
    const [publicGuildJoinErrorByGuildId, setPublicGuildJoinErrorByGuildId] = createSignal<
      Record<string, string>
    >({});

    const joinPublicGuildMock = vi.fn(async () => ({
      guildId: GUILD_B,
      outcome: "accepted" as const,
      joined: true,
    }));
    const fetchGuildsMock = vi.fn(async () => [
      guildFixture(GUILD_A, "Member Guild"),
      guildFixture(GUILD_B, "Town Hall"),
    ]);
    const fetchGuildChannelsMock = vi.fn(async (_session, guildId) => {
      if (guildId === GUILD_B) {
        return [channelFixture(CHANNEL_B, "town-square")];
      }
      return [channelFixture(CHANNEL_A, "incident-room")];
    });

    const controller = createRoot(() =>
      createPublicDirectoryController(
        {
          session,
          activeGuildId,
          activeChannelId,
          publicGuildSearchQuery,
          isSearchingPublicGuilds,
          publicGuildJoinStatusByGuildId,
          setSearchingPublicGuilds,
          setPublicGuildSearchError,
          setPublicGuildDirectory,
          setPublicGuildJoinStatusByGuildId,
          setPublicGuildJoinErrorByGuildId,
          setWorkspaces,
          setActiveGuildId,
          setActiveChannelId,
        },
        {
          joinPublicGuild: joinPublicGuildMock,
          fetchGuilds: fetchGuildsMock,
          fetchGuildChannels: fetchGuildChannelsMock,
        },
      ),
    );

    await controller.joinGuildFromDirectory(GUILD_B);

    expect(joinPublicGuildMock).toHaveBeenCalledWith(SESSION, GUILD_B);
    expect(publicGuildJoinStatusByGuildId()[GUILD_B]).toBe("joined");
    expect(publicGuildJoinErrorByGuildId()[GUILD_B]).toBeUndefined();
    expect(workspaces().map((workspace) => workspace.guildId)).toEqual([GUILD_A, GUILD_B]);
    expect(activeGuildId()).toBe(GUILD_A);
    expect(activeChannelId()).toBe(CHANNEL_A);
  });

  it("maps IP-ban join failures to deterministic banned row state", async () => {
    const [session] = createSignal<typeof SESSION | null>(SESSION);
    const [activeGuildId, setActiveGuildId] = createSignal<typeof GUILD_A | null>(GUILD_A);
    const [activeChannelId, setActiveChannelId] = createSignal<typeof CHANNEL_A | null>(CHANNEL_A);
    const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([]);
    const [publicGuildSearchQuery] = createSignal("");
    const [isSearchingPublicGuilds, setSearchingPublicGuilds] = createSignal(false);
    const [publicGuildSearchError, setPublicGuildSearchError] = createSignal("");
    const [publicGuildDirectory, setPublicGuildDirectory] = createSignal<ReturnType<
      typeof guildFixture
    >[]>([]);
    const [publicGuildJoinStatusByGuildId, setPublicGuildJoinStatusByGuildId] = createSignal<
      Record<string, PublicDirectoryJoinStatus>
    >({});
    const [publicGuildJoinErrorByGuildId, setPublicGuildJoinErrorByGuildId] = createSignal<
      Record<string, string>
    >({});

    const controller = createRoot(() =>
      createPublicDirectoryController(
        {
          session,
          activeGuildId,
          activeChannelId,
          publicGuildSearchQuery,
          isSearchingPublicGuilds,
          publicGuildJoinStatusByGuildId,
          setSearchingPublicGuilds,
          setPublicGuildSearchError,
          setPublicGuildDirectory,
          setPublicGuildJoinStatusByGuildId,
          setPublicGuildJoinErrorByGuildId,
          setWorkspaces,
          setActiveGuildId,
          setActiveChannelId,
        },
        {
          joinPublicGuild: vi.fn(async () => {
            throw new ApiError(
              403,
              "directory_join_ip_banned",
              "directory_join_ip_banned",
            );
          }),
        },
      ),
    );

    await controller.joinGuildFromDirectory(GUILD_B);

    expect(publicGuildJoinStatusByGuildId()[GUILD_B]).toBe("banned");
    expect(publicGuildJoinErrorByGuildId()[GUILD_B]).toBe(
      "Join blocked by a workspace IP moderation policy.",
    );
  });

  it("ignores stale join responses after auth reset", async () => {
    const [session, setSession] = createSignal<typeof SESSION | null>(SESSION);
    const [activeGuildId, setActiveGuildId] = createSignal<typeof GUILD_A | null>(GUILD_A);
    const [activeChannelId, setActiveChannelId] = createSignal<typeof CHANNEL_A | null>(CHANNEL_A);
    const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([]);
    const [publicGuildSearchQuery] = createSignal("");
    const [isSearchingPublicGuilds, setSearchingPublicGuilds] = createSignal(false);
    const [publicGuildSearchError, setPublicGuildSearchError] = createSignal("");
    const [publicGuildDirectory, setPublicGuildDirectory] = createSignal<ReturnType<
      typeof guildFixture
    >[]>([]);
    const [publicGuildJoinStatusByGuildId, setPublicGuildJoinStatusByGuildId] = createSignal<
      Record<string, PublicDirectoryJoinStatus>
    >({});
    const [publicGuildJoinErrorByGuildId, setPublicGuildJoinErrorByGuildId] = createSignal<
      Record<string, string>
    >({});

    const pendingJoin = deferred<{
      guildId: ReturnType<typeof guildIdFromInput>;
      outcome: "accepted";
      joined: true;
    }>();
    const joinPublicGuildMock = vi.fn(() => pendingJoin.promise);

    const controller = createRoot(() =>
      createPublicDirectoryController(
        {
          session,
          activeGuildId,
          activeChannelId,
          publicGuildSearchQuery,
          isSearchingPublicGuilds,
          publicGuildJoinStatusByGuildId,
          setSearchingPublicGuilds,
          setPublicGuildSearchError,
          setPublicGuildDirectory,
          setPublicGuildJoinStatusByGuildId,
          setPublicGuildJoinErrorByGuildId,
          setWorkspaces,
          setActiveGuildId,
          setActiveChannelId,
        },
        {
          joinPublicGuild: joinPublicGuildMock,
        },
      ),
    );

    void controller.joinGuildFromDirectory(GUILD_B);
    await flush();
    expect(publicGuildJoinStatusByGuildId()[GUILD_B]).toBe("joining");

    setSession(null);
    pendingJoin.resolve({
      guildId: GUILD_B,
      outcome: "accepted",
      joined: true,
    });
    await flush();

    expect(publicGuildJoinStatusByGuildId()).toEqual({});
    expect(publicGuildJoinErrorByGuildId()).toEqual({});
  });
});
