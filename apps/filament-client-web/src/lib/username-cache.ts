import type { AuthSession } from "../domain/auth";
import type { UserId } from "../domain/chat";
import { lookupUsersByIds } from "./api";

const MAX_CACHE_ENTRIES = 2048;
const LOOKUP_BATCH_SIZE = 32;
const CACHE_TTL_MS = 5 * 60 * 1000;
const NEGATIVE_CACHE_TTL_MS = 30 * 1000;

interface CacheEntry {
  username: string | null;
  avatarVersion: number | null;
  expiresAtUnixMs: number;
  touchedAtUnixMs: number;
}

const cache = new Map<UserId, CacheEntry>();
const inflightByUserId = new Map<UserId, Promise<{ username: string; avatarVersion: number } | null>>();

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

function setCacheEntry(userId: UserId, username: string | null, avatarVersion: number | null, ttlMs: number): void {
  const now = nowUnixMs();
  cache.set(userId, {
    username,
    avatarVersion,
    expiresAtUnixMs: now + ttlMs,
    touchedAtUnixMs: now,
  });
  evictIfNeeded();
}

function readCacheEntry(userId: UserId): CacheEntry | undefined {
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
  return entry;
}

function chunkedUserIds(userIds: UserId[]): UserId[][] {
  const chunks: UserId[][] = [];
  for (let index = 0; index < userIds.length; index += LOOKUP_BATCH_SIZE) {
    chunks.push(userIds.slice(index, index + LOOKUP_BATCH_SIZE));
  }
  return chunks;
}

export function primeUsernameCache(input: Array<{ userId: UserId; username?: string | null; avatarVersion?: number | null }>): void {
  for (const entry of input) {
    const existing = cache.get(entry.userId);
    const username = entry.username !== undefined ? entry.username : (existing?.username ?? null);
    const avatarVersion = entry.avatarVersion !== undefined ? entry.avatarVersion : (existing?.avatarVersion ?? null);
    setCacheEntry(entry.userId, username, avatarVersion, CACHE_TTL_MS);
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
  return readCacheEntry(userId)?.username;
}

export async function resolveUsernames(
  session: AuthSession,
  userIds: UserId[],
): Promise<Record<string, { username: string; avatarVersion: number }>> {
  const uniqueIds = [...new Set(userIds)];
  const resolved: Record<string, { username: string; avatarVersion: number }> = {};
  const misses: UserId[] = [];
  const waiters: Array<Promise<{ userId: UserId; username: string | null; avatarVersion: number | null }>> = [];

  for (const userId of uniqueIds) {
    const cached = readCacheEntry(userId);
    if (cached) {
      if (typeof cached.username === "string" && typeof cached.avatarVersion === "number") {
        resolved[userId] = { username: cached.username, avatarVersion: cached.avatarVersion };
        continue;
      }
      if (cached.username === null) {
        continue;
      }
    }
    const inFlight = inflightByUserId.get(userId);
    if (inFlight) {
      waiters.push(inFlight.then((result) => ({ userId, username: result?.username ?? null, avatarVersion: result?.avatarVersion ?? null })));
      continue;
    }
    misses.push(userId);
  }

  for (const chunk of chunkedUserIds(misses)) {
    const chunkPromise = lookupUsersByIds(session, chunk).then((records) => {
      const recordMap = new Map(records.map((record) => [record.userId, record]));
      for (const userId of chunk) {
        const record = recordMap.get(userId);
        setCacheEntry(userId, record?.username ?? null, record?.avatarVersion ?? null, record ? CACHE_TTL_MS : NEGATIVE_CACHE_TTL_MS);
      }
      return recordMap;
    });

    for (const userId of chunk) {
      const perIdPromise = chunkPromise
        .then((recordMap) => {
          const record = recordMap.get(userId);
          return record ? { username: record.username, avatarVersion: record.avatarVersion } : null;
        })
        .finally(() => {
          inflightByUserId.delete(userId);
        });
      inflightByUserId.set(userId, perIdPromise);
      waiters.push(perIdPromise.then((result) => ({ userId, username: result?.username ?? null, avatarVersion: result?.avatarVersion ?? null })));
    }
  }

  const settled = await Promise.all(waiters);
  for (const entry of settled) {
    if (entry.username && entry.avatarVersion !== null) {
      resolved[entry.userId] = { username: entry.username, avatarVersion: entry.avatarVersion };
    }
  }

  return resolved;
}
