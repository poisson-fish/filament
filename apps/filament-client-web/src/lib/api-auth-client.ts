import {
  type AuthSession,
  type CaptchaToken,
  type Password,
  type RefreshToken,
  type Username,
} from "../domain/auth";
import {
  type ProfileRecord,
  type UserId,
  type UserLookupRecord,
} from "../domain/chat";
import type { AuthApi } from "./api-auth";

interface AuthClientDependencies {
  authApi: AuthApi;
}

export interface AuthClient {
  registerWithPassword(input: {
    username: Username;
    password: Password;
    captchaToken?: CaptchaToken;
  }): Promise<void>;
  loginWithPassword(input: {
    username: Username;
    password: Password;
    captchaToken?: CaptchaToken;
  }): Promise<AuthSession>;
  refreshAuthSession(refreshToken: RefreshToken): Promise<AuthSession>;
  logoutAuthSession(refreshToken: RefreshToken): Promise<void>;
  fetchMe(session: AuthSession): Promise<ProfileRecord>;
  fetchUserProfile(session: AuthSession, userId: UserId): Promise<ProfileRecord>;
  updateMyProfile(
    session: AuthSession,
    input: { username?: Username; aboutMarkdown?: string },
  ): Promise<ProfileRecord>;
  uploadMyProfileAvatar(session: AuthSession, file: File): Promise<ProfileRecord>;
  lookupUsersByIds(session: AuthSession, userIds: UserId[]): Promise<UserLookupRecord[]>;
  profileAvatarUrl(userId: UserId, avatarVersion: number): string;
}

export function createAuthClient(input: AuthClientDependencies): AuthClient {
  return {
    registerWithPassword(payload) {
      return input.authApi.registerWithPassword(payload);
    },

    loginWithPassword(payload) {
      return input.authApi.loginWithPassword(payload);
    },

    refreshAuthSession(refreshToken) {
      return input.authApi.refreshAuthSession(refreshToken);
    },

    logoutAuthSession(refreshToken) {
      return input.authApi.logoutAuthSession(refreshToken);
    },

    fetchMe(session) {
      return input.authApi.fetchMe(session);
    },

    fetchUserProfile(session, userId) {
      return input.authApi.fetchUserProfile(session, userId);
    },

    updateMyProfile(session, payload) {
      return input.authApi.updateMyProfile(session, payload);
    },

    uploadMyProfileAvatar(session, file) {
      return input.authApi.uploadMyProfileAvatar(session, file);
    },

    lookupUsersByIds(session, userIds) {
      return input.authApi.lookupUsersByIds(session, userIds);
    },

    profileAvatarUrl(userId, avatarVersion) {
      return input.authApi.profileAvatarUrl(userId, avatarVersion);
    },
  };
}