import { createEffect, createMemo, onCleanup, type Accessor, type Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import {
  userIdFromInput,
  type FriendRecord,
  type FriendRequestList,
  type MessageRecord,
  type ProfileRecord,
  type SearchResults,
  type UserId,
} from "../../../domain/chat";
import {
  clearUsernameLookupCache,
  primeUsernameCache,
  resolveUsernames,
} from "../../../lib/username-cache";
import { userIdFromVoiceIdentity } from "../helpers";
import type { VoiceRosterEntry } from "../types";

export interface IdentityResolutionControllerOptions {
  session: Accessor<AuthSession | null>;
  messages: Accessor<MessageRecord[]>;
  onlineMembers: Accessor<string[]>;
  voiceRosterEntries: Accessor<VoiceRosterEntry[]>;
  searchResults: Accessor<SearchResults | null>;
  profile: Accessor<ProfileRecord | undefined>;
  selectedProfile: Accessor<ProfileRecord | null | undefined>;
  friends: Accessor<FriendRecord[]>;
  friendRequests: Accessor<FriendRequestList>;
  setResolvedUsernames: Setter<Record<string, string>>;
  setAvatarVersionByUserId: Setter<Record<string, number>>;
}

export interface IdentityResolutionControllerDependencies {
  clearUsernameLookupCache: typeof clearUsernameLookupCache;
  primeUsernameCache: typeof primeUsernameCache;
  resolveUsernames: typeof resolveUsernames;
}

const DEFAULT_IDENTITY_RESOLUTION_CONTROLLER_DEPENDENCIES: IdentityResolutionControllerDependencies =
{
  clearUsernameLookupCache,
  primeUsernameCache,
  resolveUsernames,
};

export function collectVisibleUserIds(input: {
  messages: MessageRecord[];
  onlineMembers: string[];
  voiceRosterEntries: VoiceRosterEntry[];
  searchResults: SearchResults | null;
}): UserId[] {
  const lookupIds = new Set<UserId>();
  for (const message of input.messages) {
    lookupIds.add(message.authorId);
  }
  for (const memberId of input.onlineMembers) {
    try {
      lookupIds.add(userIdFromInput(memberId));
    } catch {
      continue;
    }
  }
  for (const participant of input.voiceRosterEntries) {
    const participantUserId = userIdFromVoiceIdentity(participant.identity);
    if (participantUserId) {
      lookupIds.add(participantUserId);
    }
  }
  if (input.searchResults) {
    for (const message of input.searchResults.messages) {
      lookupIds.add(message.authorId);
    }
  }
  return [...lookupIds];
}

export function createIdentityResolutionController(
  options: IdentityResolutionControllerOptions,
  dependencies: Partial<IdentityResolutionControllerDependencies> = {},
): void {
  const deps = {
    ...DEFAULT_IDENTITY_RESOLUTION_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };

  createEffect(() => {
    const session = options.session();
    if (session) {
      return;
    }
    deps.clearUsernameLookupCache();
    options.setResolvedUsernames({});
    options.setAvatarVersionByUserId({});
  });

  createEffect(() => {
    const session = options.session();
    const value = options.profile();
    if (!session || !value) {
      return;
    }
    deps.primeUsernameCache([{ userId: value.userId, username: value.username, avatarVersion: value.avatarVersion }]);
    options.setResolvedUsernames((existing) => ({
      ...existing,
      [value.userId]: value.username,
    }));
    options.setAvatarVersionByUserId((existing) => ({
      ...existing,
      [value.userId]: value.avatarVersion,
    }));
  });

  createEffect(() => {
    const session = options.session();
    const value = options.selectedProfile();
    if (!session || !value) {
      return;
    }
    options.setAvatarVersionByUserId((existing) => ({
      ...existing,
      [value.userId]: value.avatarVersion,
    }));
  });

  createEffect(() => {
    const session = options.session();
    const known = [
      ...options.friends().map((friend) => ({
        userId: friend.userId,
        username: friend.username,
      })),
      ...options.friendRequests().incoming.map((request) => ({
        userId: request.senderUserId,
        username: request.senderUsername,
      })),
      ...options.friendRequests().outgoing.map((request) => ({
        userId: request.recipientUserId,
        username: request.recipientUsername,
      })),
    ];
    if (!session || known.length === 0) {
      return;
    }
    deps.primeUsernameCache(known);
    options.setResolvedUsernames((existing) => ({
      ...existing,
      ...Object.fromEntries(known.map((entry) => [entry.userId, entry.username])),
    }));
  });

  const visibleLookupKey = createMemo(() => {
    const lookupIds = collectVisibleUserIds({
      messages: options.messages(),
      onlineMembers: options.onlineMembers(),
      voiceRosterEntries: options.voiceRosterEntries(),
      searchResults: options.searchResults(),
    });
    if (lookupIds.length === 0) {
      return "";
    }
    return [...lookupIds].sort((left, right) => left.localeCompare(right)).join("|");
  });

  createEffect(() => {
    const session = options.session();
    const lookupKey = visibleLookupKey();
    if (!session || lookupKey.length === 0) {
      return;
    }
    const lookupIds = lookupKey.split("|") as UserId[];

    let cancelled = false;
    const resolveVisibleUsernames = async () => {
      try {
        const resolved = await deps.resolveUsernames(session, lookupIds);
        if (cancelled || Object.keys(resolved).length === 0) {
          return;
        }
        options.setResolvedUsernames((existing) => {
          const next = { ...existing };
          for (const [userId, lookup] of Object.entries(resolved)) {
            next[userId] = lookup.username;
          }
          return next;
        });
        options.setAvatarVersionByUserId((existing) => {
          const next = { ...existing };
          for (const [userId, lookup] of Object.entries(resolved)) {
            next[userId] = lookup.avatarVersion;
          }
          return next;
        });
      } catch {
        // Keep user-id fallback rendering if lookup fails.
      }
    };
    void resolveVisibleUsernames();

    onCleanup(() => {
      cancelled = true;
    });
  });
}
