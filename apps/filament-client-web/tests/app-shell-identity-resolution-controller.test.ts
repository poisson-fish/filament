import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  friendListFromResponse,
  friendRequestListFromResponse,
  messageFromResponse,
  profileFromResponse,
  searchResultsFromResponse,
} from "../src/domain/chat";
import {
  collectVisibleUserIds,
  createIdentityResolutionController,
} from "../src/features/app-shell/controllers/identity-resolution-controller";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

function messageFixture(authorId: string, messageId: string) {
  return messageFromResponse({
    message_id: messageId,
    guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
    channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FAW",
    author_id: authorId,
    content: "message",
    markdown_tokens: [{ type: "text", text: "message" }],
    attachments: [],
    created_at_unix: 1,
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

describe("app shell identity resolution controller", () => {
  it("collects visible lookup IDs from messages, presence, voice roster, and search results", () => {
    const ids = collectVisibleUserIds({
      messages: [
        messageFixture("01ARZ3NDEKTSV4RRFFQ69G5FAA", "01ARZ3NDEKTSV4RRFFQ69G5FAB"),
      ],
      onlineMembers: ["invalid", "01ARZ3NDEKTSV4RRFFQ69G5FAC"],
      voiceRosterEntries: [
        {
          identity: "u.01ARZ3NDEKTSV4RRFFQ69G5FAD.remote",
          isLocal: false,
          isMuted: false,
          isDeafened: false,
          isSpeaking: false,
          hasCamera: false,
          hasScreenShare: false,
        },
      ],
      searchResults: searchResultsFromResponse({
        message_ids: ["01ARZ3NDEKTSV4RRFFQ69G5FAF"],
        messages: [
          {
            message_id: "01ARZ3NDEKTSV4RRFFQ69G5FAF",
            guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            channel_id: "01ARZ3NDEKTSV4RRFFQ69G5FAW",
            author_id: "01ARZ3NDEKTSV4RRFFQ69G5FAE",
            content: "search",
            markdown_tokens: [{ type: "text", text: "search" }],
            attachments: [],
            created_at_unix: 2,
          },
        ],
      }),
      workspaceMembers: [],
    });

    expect(ids).toEqual([
      "01ARZ3NDEKTSV4RRFFQ69G5FAA",
      "01ARZ3NDEKTSV4RRFFQ69G5FAC",
      "01ARZ3NDEKTSV4RRFFQ69G5FAD",
      "01ARZ3NDEKTSV4RRFFQ69G5FAE",
    ]);
  });

  it("primes cache from known identities and cancels stale username resolutions", async () => {
    const [session, setSession] = createSignal<typeof SESSION | null>(SESSION);
    const [messages] = createSignal([
      messageFixture("01ARZ3NDEKTSV4RRFFQ69G5FAA", "01ARZ3NDEKTSV4RRFFQ69G5FAB"),
    ]);
    const [onlineMembers] = createSignal<string[]>([]);
    const [voiceRosterEntries] = createSignal([]);
    const [searchResults] = createSignal(null);
    const [workspaceMembers] = createSignal<string[]>([]);
    const [profile] = createSignal(
      profileFromResponse({
        user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
        username: "alice",
        about_markdown: "",
        about_markdown_tokens: [],
        avatar_version: 4,
        banner_version: 2,
      }),
    );
    const [selectedProfile] = createSignal(null);
    const [friends] = createSignal(
      friendListFromResponse({
        friends: [
          {
            user_id: "01ARZ3NDEKTSV4RRFFQ69G5FAC",
            username: "bob",
            created_at_unix: 3,
          },
        ],
      }),
    );
    const [friendRequests] = createSignal(
      friendRequestListFromResponse({ incoming: [], outgoing: [] }),
    );
    const [resolvedUsernames, setResolvedUsernames] = createSignal<Record<string, string>>({});
    const [avatarVersionByUserId, setAvatarVersionByUserId] = createSignal<Record<string, number>>({});

    const pendingResolution = deferred<Record<string, { username: string; avatarVersion: number; }>>();
    const clearUsernameLookupCacheMock = vi.fn();
    const primeUsernameCacheMock = vi.fn();
    const resolveUsernamesMock = vi.fn(() => pendingResolution.promise);

    const dispose = createRoot((rootDispose) => {
      createIdentityResolutionController(
        {
          session,
          messages,
          onlineMembers,
          voiceRosterEntries,
          searchResults,
          workspaceMembers,
          profile,
          selectedProfile,
          friends,
          friendRequests,
          setResolvedUsernames,
          setAvatarVersionByUserId,
        },
        {
          clearUsernameLookupCache: clearUsernameLookupCacheMock,
          primeUsernameCache: primeUsernameCacheMock,
          resolveUsernames: resolveUsernamesMock,
        },
      );
      return rootDispose;
    });

    await flush();
    expect(primeUsernameCacheMock).toHaveBeenCalledTimes(2);
    expect(resolvedUsernames()).toEqual({
      "01ARZ3NDEKTSV4RRFFQ69G5FAA": "alice",
      "01ARZ3NDEKTSV4RRFFQ69G5FAC": "bob",
    });
    expect(avatarVersionByUserId()).toEqual({
      "01ARZ3NDEKTSV4RRFFQ69G5FAA": 4,
    });

    setSession(null);
    pendingResolution.resolve({
      "01ARZ3NDEKTSV4RRFFQ69G5FAA": { username: "late-value", avatarVersion: 1 },
    });
    await flush();

    expect(clearUsernameLookupCacheMock).toHaveBeenCalledTimes(1);
    expect(resolvedUsernames()).toEqual({});
    expect(avatarVersionByUserId()).toEqual({});

    dispose();
  });

  it("does not re-resolve when only voice speaking flags change", async () => {
    const [session] = createSignal<typeof SESSION | null>(SESSION);
    const [messages] = createSignal([]);
    const [onlineMembers] = createSignal<string[]>([]);
    const [voiceRosterEntries, setVoiceRosterEntries] = createSignal([
      {
        identity: "u.01ARZ3NDEKTSV4RRFFQ69G5FAD.remote",
        isLocal: false,
        isMuted: false,
        isDeafened: false,
        isSpeaking: false,
        hasCamera: false,
        hasScreenShare: false,
      },
    ]);
    const [searchResults] = createSignal(null);
    const [workspaceMembers] = createSignal<string[]>([]);
    const [profile] = createSignal(undefined);
    const [selectedProfile] = createSignal(null);
    const [friends] = createSignal(friendListFromResponse({ friends: [] }));
    const [friendRequests] = createSignal(
      friendRequestListFromResponse({ incoming: [], outgoing: [] }),
    );
    const [, setResolvedUsernames] = createSignal<Record<string, string>>({});
    const [, setAvatarVersionByUserId] = createSignal<Record<string, number>>({});

    const resolveUsernamesMock = vi.fn(async () => ({}));

    const dispose = createRoot((rootDispose) => {
      createIdentityResolutionController(
        {
          session,
          messages,
          onlineMembers,
          voiceRosterEntries,
          searchResults,
          workspaceMembers,
          profile,
          selectedProfile,
          friends,
          friendRequests,
          setResolvedUsernames,
          setAvatarVersionByUserId,
        },
        {
          resolveUsernames: resolveUsernamesMock,
        },
      );
      return rootDispose;
    });

    await flush();
    expect(resolveUsernamesMock).toHaveBeenCalledTimes(1);

    setVoiceRosterEntries([
      {
        identity: "u.01ARZ3NDEKTSV4RRFFQ69G5FAD.remote",
        isLocal: false,
        isMuted: false,
        isDeafened: false,
        isSpeaking: true,
        hasCamera: false,
        hasScreenShare: false,
      },
    ]);
    await flush();
    expect(resolveUsernamesMock).toHaveBeenCalledTimes(1);

    dispose();
  });
});
