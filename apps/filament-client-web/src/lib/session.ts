import {
  type AccessToken,
  type AuthSession,
  accessTokenFromInput,
  refreshTokenFromInput,
} from "../domain/auth";

const SESSION_STORAGE_KEY = "filament.auth.session.v1";

function canUseStorage(): boolean {
  return typeof window !== "undefined" && typeof window.sessionStorage !== "undefined";
}

export function loadSession(): AuthSession | null {
  if (!canUseStorage()) {
    return null;
  }

  const raw = window.sessionStorage.getItem(SESSION_STORAGE_KEY);
  if (!raw || raw.length > 16_384) {
    return null;
  }

  try {
    const parsed = JSON.parse(raw) as {
      accessToken: string;
      refreshToken: string;
      expiresAtUnix: number;
    };
    if (!Number.isInteger(parsed.expiresAtUnix) || parsed.expiresAtUnix <= 0) {
      return null;
    }

    return {
      accessToken: accessTokenFromInput(parsed.accessToken),
      refreshToken: refreshTokenFromInput(parsed.refreshToken),
      expiresAtUnix: parsed.expiresAtUnix,
    };
  } catch {
    return null;
  }
}

export function saveSession(session: AuthSession): void {
  if (!canUseStorage()) {
    return;
  }

  window.sessionStorage.setItem(SESSION_STORAGE_KEY, JSON.stringify(session));
}

export function clearSession(): void {
  if (!canUseStorage()) {
    return;
  }
  window.sessionStorage.removeItem(SESSION_STORAGE_KEY);
}

export function isSessionExpired(session: AuthSession): boolean {
  const nowUnix = Math.floor(Date.now() / 1000);
  return session.expiresAtUnix <= nowUnix + 15;
}

export function bearerHeader(token: AccessToken): string {
  return `Bearer ${token}`;
}
