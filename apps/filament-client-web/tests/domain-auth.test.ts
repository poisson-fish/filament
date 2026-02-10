import {
  DomainValidationError,
  accessTokenFromInput,
  authSessionFromResponse,
  passwordFromInput,
  refreshTokenFromInput,
  usernameFromInput,
} from "../src/domain/auth";

describe("auth domain invariants", () => {
  it("accepts valid username and password bounds", () => {
    expect(usernameFromInput("user_name.01")).toBe("user_name.01");
    expect(passwordFromInput("123456789012")).toBe("123456789012");
  });

  it("rejects invalid username", () => {
    expect(() => usernameFromInput("ab")).toThrow(DomainValidationError);
    expect(() => usernameFromInput("bad user")).toThrow(DomainValidationError);
  });

  it("rejects invalid token chars", () => {
    expect(() => accessTokenFromInput(`A${"B".repeat(31)}\n`)).toThrow(
      DomainValidationError,
    );
  });

  it("maps login response into a bounded auth session", () => {
    const accessToken = "A".repeat(64);
    const refreshToken = "B".repeat(64);

    const session = authSessionFromResponse({
      access_token: accessToken,
      refresh_token: refreshToken,
      expires_in_secs: 900,
    });

    expect(session.accessToken).toBe(accessToken);
    expect(session.refreshToken).toBe(refreshToken);
    expect(session.expiresAtUnix).toBeGreaterThan(Math.floor(Date.now() / 1000));
    expect(refreshTokenFromInput(session.refreshToken)).toBe(refreshToken);
  });
});
