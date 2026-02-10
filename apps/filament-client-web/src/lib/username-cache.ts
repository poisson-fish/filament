import type { AuthSession } from "../domain/auth";
import type { UserId } from "../domain/chat";
import { lookupUsersByIds } from "./api";

const MAX_CACHE_ENTRIES = 2048;
const LOOKUP_BATCH_SIZE = 32;
const CACHE_TTL_MS = 5 * 60 * 1000;
const NEGATIVE_CACHE_TTL_MS = 30 * 1000;

interface CacheEntry {
  username: string | null;
  expiresAtUnixMs: number;
  touchedAtUnixMs: number;
}

const cache = new Map<UserId, CacheEntry>();
const inflightByUserId = new Map<UserId, Promise<string | null>>();

function nowUnixMs(): number {
  return Date.now();
}

function evictIfNeeded(): void {
  if (cache.size <= MAX_CACHE_ENTRIES) {
    return;
  }
  const entries = [...cache.entries()].sort(
    (left, right) => left[1].touchedAtUnixMs - right[1].touchedAtUnixMs,
  );
  const removeCount = cache.size - MAX_CACHE_ENTRIES;
  for (const [userId] of entries.slice(0, removeCount)) {
    cache.delete(userId);
  }
}

function setCacheEntry(userId: UserId, username: string | null, ttlMs: number): void {
  const now = nowUnixMs();
  cache.set(userId, {
    username,
    expiresAtUnixMs: now + ttlMs,
    touchedAtUnixMs: now,
  });
  evictIfNeeded();
}

function readCacheEntry(userId: UserId): string | null | undefined {
  const entry = cache.get(userId);
  if (!entry) {
    return undefined;
  }
  const now = nowUnixMs();
  if (entry.expiresAtUnixMs <= now) {
    cache.delete(userId);
    return undefined;
  }
  cache.set(userId, { ...entry, touchedAtUnixMs: now });
  return entry.username;
}

function chunkedUserIds(userIds: UserId[]): UserId[][] {
  const chunks: UserId[][] = [];
  for (let index = 0; index < userIds.length; index += LOOKUP_BATCH_SIZE) {
    chunks.push(userIds.slice(index, index + LOOKUP_BATCH_SIZE));
  }
  return chunks;
}

export function primeUsernameCache(input: Array<{ userId: UserId; username: string }>): void {
  for (const entry of input) {
    setCacheEntry(entry.userId, entry.username, CACHE_TTL_MS);
  }
}

export function invalidateUsernameCache(userId?: UserId): void {
  if (!userId) {
    cache.clear();
    inflightByUserId.clear();
    return;
  }
  cache.delete(userId);
  inflightByUserId.delete(userId);
}

export function clearUsernameLookupCache(): void {
  invalidateUsernameCache();
}

export function getCachedUsername(userId: UserId): string | null | undefined {
  return readCacheEntry(userId);
}

export async function resolveUsernames(
  session: AuthSession,
  userIds: UserId[],
): Promise<Record<string, string>> {
  const uniqueIds = [...new Set(userIds)];
  const resolved: Record<string, string> = {};
  const misses: UserId[] = [];
  const waiters: Array<Promise<{ userId: UserId; username: string | null }>> = [];

  for (const userId of uniqueIds) {
    const cached = readCacheEntry(userId);
    if (typeof cached === "string") {
      resolved[userId] = cached;
      continue;
    }
    if (cached === null) {
      continue;
    }
    const inFlight = inflightByUserId.get(userId);
    if (inFlight) {
      waiters.push(inFlight.then((username) => ({ userId, username })));
      continue;
    }
    misses.push(userId);
  }

  for (const chunk of chunkedUserIds(misses)) {
    const chunkPromise = lookupUsersByIds(session, chunk).then((records) => {
      const recordMap = new Map(records.map((record) => [record.userId, record.username]));
      for (const userId of chunk) {
        const username = recordMap.get(userId) ?? null;
        setCacheEntry(userId, username, username ? CACHE_TTL_MS : NEGATIVE_CACHE_TTL_MS);
      }
      return recordMap;
    });

    for (const userId of chunk) {
      const perIdPromise = chunkPromise
        .then((recordMap) => recordMap.get(userId) ?? null)
        .finally(() => {
          inflightByUserId.delete(userId);
        });
      inflightByUserId.set(userId, perIdPromise);
      waiters.push(perIdPromise.then((username) => ({ userId, username })));
    }
  }

  const settled = await Promise.all(waiters);
  for (const entry of settled) {
    if (entry.username) {
      resolved[entry.userId] = entry.username;
    }
  }

  return resolved;
}
