import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AuthSession } from "../src/domain/auth";
import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";
import { userIdFromInput, type UserId } from "../src/domain/chat";
import {
  getCachedUsername,
  invalidateUsernameCache,
  primeUsernameCache,
  resolveUsernames,
} from "../src/lib/username-cache";
import * as api from "../src/lib/api";

const SESSION: AuthSession = {
  accessToken: accessTokenFromInput("A".repeat(64)),
  refreshToken: refreshTokenFromInput("B".repeat(64)),
  expiresAtUnix: Math.floor(Date.now() / 1000) + 3600,
};

const USER_A = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const USER_B = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const USER_C = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");

function asLookupRecord(
  userId: UserId,
  username: string,
): { userId: UserId; username: string; avatarVersion: number } {
  return { userId, username, avatarVersion: 0 };
}

describe("username cache", () => {
  beforeEach(() => {
    invalidateUsernameCache();
    vi.restoreAllMocks();
  });

  it("primes and invalidates cached usernames", () => {
    primeUsernameCache([asLookupRecord(USER_A, "alice")]);
    expect(getCachedUsername(USER_A)).toBe("alice");
    invalidateUsernameCache(USER_A);
    expect(getCachedUsername(USER_A)).toBeUndefined();
  });

  it("deduplicates and caches lookups without refetching cached users", async () => {
    const lookupSpy = vi.spyOn(api, "lookupUsersByIds").mockResolvedValue([
      asLookupRecord(USER_A, "alice"),
      asLookupRecord(USER_B, "bob"),
    ]);

    const first = await resolveUsernames(SESSION, [USER_A, USER_A, USER_B]);
    expect(first[USER_A]).toEqual({ username: "alice", avatarVersion: 0 });
    expect(first[USER_B]).toEqual({ username: "bob", avatarVersion: 0 });
    expect(lookupSpy).toHaveBeenCalledTimes(1);
    expect(lookupSpy).toHaveBeenCalledWith(SESSION, [USER_A, USER_B]);

    const second = await resolveUsernames(SESSION, [USER_A, USER_B]);
    expect(second[USER_A]).toEqual({ username: "alice", avatarVersion: 0 });
    expect(second[USER_B]).toEqual({ username: "bob", avatarVersion: 0 });
    expect(lookupSpy).toHaveBeenCalledTimes(1);
  });

  it("stores negative cache entries for missing users", async () => {
    const lookupSpy = vi
      .spyOn(api, "lookupUsersByIds")
      .mockResolvedValue([asLookupRecord(USER_A, "alice")]);

    const first = await resolveUsernames(SESSION, [USER_A, USER_C]);
    expect(first[USER_A]).toEqual({ username: "alice", avatarVersion: 0 });
    expect(first[USER_C]).toBeUndefined();
    expect(lookupSpy).toHaveBeenCalledTimes(1);
    expect(lookupSpy).toHaveBeenCalledWith(SESSION, [USER_A, USER_C]);

    const second = await resolveUsernames(SESSION, [USER_C]);
    expect(second[USER_C]).toBeUndefined();
    expect(lookupSpy).toHaveBeenCalledTimes(1);
  });
});
