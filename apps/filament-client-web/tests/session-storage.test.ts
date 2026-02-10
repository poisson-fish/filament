import { saveSession, loadSession, clearSession, isSessionExpired } from "../src/lib/session";
import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";

describe("session storage", () => {
  beforeEach(() => {
    window.sessionStorage.clear();
  });

  it("roundtrips a valid session", () => {
    const session = {
      accessToken: accessTokenFromInput("A".repeat(64)),
      refreshToken: refreshTokenFromInput("B".repeat(64)),
      expiresAtUnix: Math.floor(Date.now() / 1000) + 900,
    };

    saveSession(session);
    expect(loadSession()).toEqual(session);
  });

  it("drops malformed storage payload", () => {
    window.sessionStorage.setItem(
      "filament.auth.session.v1",
      JSON.stringify({
        accessToken: "bad",
        refreshToken: "also_bad",
        expiresAtUnix: 5,
      }),
    );

    expect(loadSession()).toBeNull();
  });

  it("flags near-expiry sessions", () => {
    const now = Math.floor(Date.now() / 1000);
    expect(
      isSessionExpired({
        accessToken: accessTokenFromInput("A".repeat(64)),
        refreshToken: refreshTokenFromInput("B".repeat(64)),
        expiresAtUnix: now + 5,
      }),
    ).toBe(true);
  });

  it("clears session", () => {
    const session = {
      accessToken: accessTokenFromInput("A".repeat(64)),
      refreshToken: refreshTokenFromInput("B".repeat(64)),
      expiresAtUnix: Math.floor(Date.now() / 1000) + 900,
    };

    saveSession(session);
    clearSession();
    expect(loadSession()).toBeNull();
  });
});
