export class DomainValidationError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "DomainValidationError";
  }
}

export type Username = string & { readonly __brand: "username" };
export type Password = string & { readonly __brand: "password" };
export type AccessToken = string & { readonly __brand: "access_token" };
export type RefreshToken = string & { readonly __brand: "refresh_token" };
export type CaptchaToken = string & { readonly __brand: "captcha_token" };

const USERNAME_PATTERN = /^[A-Za-z0-9_.]+$/;

export function usernameFromInput(input: string): Username {
  if (input.length < 3 || input.length > 32 || !USERNAME_PATTERN.test(input)) {
    throw new DomainValidationError(
      "Username must be 3-32 chars and contain only letters, numbers, '_' or '.'.",
    );
  }
  return input as Username;
}

export function passwordFromInput(input: string): Password {
  if (input.length < 12 || input.length > 128) {
    throw new DomainValidationError("Password must be 12-128 characters.");
  }
  return input as Password;
}

function tokenFromInput(
  input: string,
  tokenName: string,
): AccessToken | RefreshToken {
  if (input.length < 32 || input.length > 4096) {
    throw new DomainValidationError(`${tokenName} has invalid length.`);
  }
  for (const char of input) {
    const code = char.charCodeAt(0);
    if (code < 0x21 || code > 0x7e) {
      throw new DomainValidationError(`${tokenName} has invalid charset.`);
    }
  }
  return input as AccessToken | RefreshToken;
}

export function accessTokenFromInput(input: string): AccessToken {
  return tokenFromInput(input, "access token") as AccessToken;
}

export function refreshTokenFromInput(input: string): RefreshToken {
  return tokenFromInput(input, "refresh token") as RefreshToken;
}

export function captchaTokenFromInput(input: string): CaptchaToken {
  if (input.length < 20 || input.length > 4096) {
    throw new DomainValidationError("Captcha token has invalid length.");
  }
  for (const char of input) {
    const code = char.charCodeAt(0);
    if (code < 0x21 || code > 0x7e) {
      throw new DomainValidationError("Captcha token has invalid charset.");
    }
  }
  return input as CaptchaToken;
}

export interface AuthSession {
  accessToken: AccessToken;
  refreshToken: RefreshToken;
  expiresAtUnix: number;
}

export function authSessionFromResponse(dto: {
  access_token: string;
  refresh_token: string;
  expires_in_secs: number;
}): AuthSession {
  if (!Number.isInteger(dto.expires_in_secs) || dto.expires_in_secs <= 0) {
    throw new DomainValidationError("Token expiry must be a positive integer.");
  }

  const nowUnix = Math.floor(Date.now() / 1000);
  const expiresAtUnix = nowUnix + dto.expires_in_secs;
  return {
    accessToken: accessTokenFromInput(dto.access_token),
    refreshToken: refreshTokenFromInput(dto.refresh_token),
    expiresAtUnix,
  };
}
