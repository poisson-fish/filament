import {
  createEffect,
  createResource,
  type Accessor,
  type Setter,
} from "solid-js";
import { usernameFromInput, type AuthSession } from "../../../domain/auth";
import {
  profileAboutFromInput,
  userIdFromInput,
  type UserId,
} from "../../../domain/chat";
import {
  fetchMe,
  fetchUserProfile,
  profileAvatarUrl,
  profileBannerUrl,
  updateMyProfile,
  uploadMyProfileAvatar,
  uploadMyProfileBanner,
} from "../../../lib/api";
import { mapError } from "../helpers";

export interface ProfileControllerOptions {
  session: Accessor<AuthSession | null>;
  selectedProfileUserId: Accessor<UserId | null>;
  avatarVersionByUserId: Accessor<Record<string, number>>;
  bannerVersionByUserId: Accessor<Record<string, number>>;
  profileDraftUsername: Accessor<string>;
  profileDraftAbout: Accessor<string>;
  selectedProfileAvatarFile: Accessor<File | null>;
  selectedProfileBannerFile: Accessor<File | null>;
  isSavingProfile: Accessor<boolean>;
  isUploadingProfileAvatar: Accessor<boolean>;
  isUploadingProfileBanner: Accessor<boolean>;
  setProfileDraftUsername: Setter<string>;
  setProfileDraftAbout: Setter<string>;
  setSelectedProfileAvatarFile: Setter<File | null>;
  setSelectedProfileBannerFile: Setter<File | null>;
  setProfileSettingsStatus: Setter<string>;
  setProfileSettingsError: Setter<string>;
  setSavingProfile: Setter<boolean>;
  setUploadingProfileAvatar: Setter<boolean>;
  setUploadingProfileBanner: Setter<boolean>;
  setAvatarVersionByUserId: Setter<Record<string, number>>;
  setBannerVersionByUserId: Setter<Record<string, number>>;
  setSelectedProfileUserId: Setter<UserId | null>;
  setSelectedProfileError: Setter<string>;
}

function mergeAvatarVersion(
  existing: Record<string, number>,
  userId: string,
  avatarVersion: number,
): Record<string, number> {
  const current = existing[userId] ?? 0;
  const nextVersion = Math.max(current, avatarVersion);
  if (current === nextVersion) {
    return existing;
  }
  return {
    ...existing,
    [userId]: nextVersion,
  };
}

function mergeBannerVersion(
  existing: Record<string, number>,
  userId: string,
  bannerVersion: number,
): Record<string, number> {
  const current = existing[userId] ?? 0;
  const nextVersion = Math.max(current, bannerVersion);
  if (current === nextVersion) {
    return existing;
  }
  return {
    ...existing,
    [userId]: nextVersion,
  };
}

export interface ProfileControllerDependencies {
  fetchMe: typeof fetchMe;
  fetchUserProfile: typeof fetchUserProfile;
  updateMyProfile: typeof updateMyProfile;
  uploadMyProfileAvatar: typeof uploadMyProfileAvatar;
  uploadMyProfileBanner: typeof uploadMyProfileBanner;
  profileAvatarUrl: typeof profileAvatarUrl;
  profileBannerUrl: typeof profileBannerUrl;
}

const DEFAULT_PROFILE_CONTROLLER_DEPENDENCIES: ProfileControllerDependencies = {
  fetchMe,
  fetchUserProfile,
  updateMyProfile,
  uploadMyProfileAvatar,
  uploadMyProfileBanner,
  profileAvatarUrl,
  profileBannerUrl,
};

export function createProfileController(
  options: ProfileControllerOptions,
  dependencies: Partial<ProfileControllerDependencies> = {},
) {
  const deps = {
    ...DEFAULT_PROFILE_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };

  const [profile, { mutate: mutateProfile }] = createResource(async () => {
    const session = options.session();
    if (!session) {
      throw new Error("missing_session");
    }
    return deps.fetchMe(session);
  });
  const [selectedProfile] = createResource(
    () => options.selectedProfileUserId() ?? undefined,
    async (userId) => {
      const session = options.session();
      if (!session) {
        return null;
      }
      try {
        return await deps.fetchUserProfile(session, userId);
      } catch (error) {
        options.setSelectedProfileError(mapError(error, "Profile unavailable."));
        return null;
      }
    },
  );

  const avatarUrlForUser = (rawUserId: string): string | null => {
    try {
      const userId = userIdFromInput(rawUserId);
      const avatarVersion = options.avatarVersionByUserId()[userId] ?? 0;
      return deps.profileAvatarUrl(userId, avatarVersion);
    } catch {
      return null;
    }
  };

  const openUserProfile = (rawUserId: string): void => {
    try {
      const userId = userIdFromInput(rawUserId);
      options.setSelectedProfileError("");
      options.setSelectedProfileUserId(userId);
    } catch {
      options.setSelectedProfileError("User profile is unavailable.");
    }
  };

  const bannerUrlForUser = (rawUserId: string): string | null => {
    try {
      const userId = userIdFromInput(rawUserId);
      const bannerVersion = options.bannerVersionByUserId()[userId] ?? 0;
      return deps.profileBannerUrl(userId, bannerVersion);
    } catch {
      return null;
    }
  };

  const saveProfileSettings = async (): Promise<void> => {
    const session = options.session();
    const currentProfile = profile();
    if (!session || !currentProfile || options.isSavingProfile()) {
      return;
    }

    options.setProfileSettingsStatus("");
    options.setProfileSettingsError("");
    const nextUsernameInput = options.profileDraftUsername().trim();
    const nextAboutInput = options.profileDraftAbout();
    let nextUsername: ReturnType<typeof usernameFromInput>;
    let nextAbout: ReturnType<typeof profileAboutFromInput>;
    try {
      nextUsername = usernameFromInput(nextUsernameInput);
      nextAbout = profileAboutFromInput(nextAboutInput);
    } catch (error) {
      options.setProfileSettingsError(mapError(error, "Unable to save profile settings."));
      return;
    }

    options.setSavingProfile(true);
    try {
      const updated = await deps.updateMyProfile(session, {
        username: nextUsername,
        aboutMarkdown: nextAbout,
      });
      mutateProfile(updated);
      options.setProfileSettingsStatus("Profile updated.");
    } catch (error) {
      options.setProfileSettingsError(mapError(error, "Unable to save profile settings."));
    } finally {
      options.setSavingProfile(false);
    }
  };

  const uploadProfileAvatar = async (): Promise<void> => {
    const session = options.session();
    const selectedFile = options.selectedProfileAvatarFile();
    if (!session || !selectedFile || options.isUploadingProfileAvatar()) {
      return;
    }

    options.setUploadingProfileAvatar(true);
    options.setProfileSettingsStatus("");
    options.setProfileSettingsError("");
    try {
      const updated = await deps.uploadMyProfileAvatar(session, selectedFile);
      const previousVersion = options.avatarVersionByUserId()[updated.userId] ?? 0;
      mutateProfile(updated);
      options.setAvatarVersionByUserId((existing) => {
        const current = existing[updated.userId] ?? 0;
        const serverVersion = Math.max(0, updated.avatarVersion);
        const baselineVersion = Math.max(previousVersion, serverVersion);
        const nextVersion =
          baselineVersion > previousVersion ? baselineVersion : previousVersion + 1;
        if (current >= nextVersion) {
          return existing;
        }
        return {
          ...existing,
          [updated.userId]: nextVersion,
        };
      });
      options.setSelectedProfileAvatarFile(null);
      options.setProfileSettingsStatus("Profile avatar updated.");
    } catch (error) {
      options.setProfileSettingsError(mapError(error, "Unable to upload profile avatar."));
    } finally {
      options.setUploadingProfileAvatar(false);
    }
  };

  const uploadProfileBanner = async (): Promise<void> => {
    const session = options.session();
    const selectedFile = options.selectedProfileBannerFile();
    if (!session || !selectedFile || options.isUploadingProfileBanner()) {
      return;
    }

    options.setUploadingProfileBanner(true);
    options.setProfileSettingsStatus("");
    options.setProfileSettingsError("");
    try {
      const updated = await deps.uploadMyProfileBanner(session, selectedFile);
      const previousVersion = options.bannerVersionByUserId()[updated.userId] ?? 0;
      mutateProfile(updated);
      options.setBannerVersionByUserId((existing) => {
        const current = existing[updated.userId] ?? 0;
        const serverVersion = Math.max(0, updated.bannerVersion);
        const baselineVersion = Math.max(previousVersion, serverVersion);
        const nextVersion =
          baselineVersion > previousVersion ? baselineVersion : previousVersion + 1;
        if (current >= nextVersion) {
          return existing;
        }
        return {
          ...existing,
          [updated.userId]: nextVersion,
        };
      });
      options.setSelectedProfileBannerFile(null);
      options.setProfileSettingsStatus("Profile banner updated.");
    } catch (error) {
      options.setProfileSettingsError(mapError(error, "Unable to upload profile banner."));
    } finally {
      options.setUploadingProfileBanner(false);
    }
  };

  createEffect(() => {
    const session = options.session();
    if (!session) {
      options.setSavingProfile(false);
      options.setUploadingProfileAvatar(false);
      options.setUploadingProfileBanner(false);
      options.setProfileDraftUsername("");
      options.setProfileDraftAbout("");
      options.setSelectedProfileAvatarFile(null);
      options.setSelectedProfileBannerFile(null);
      options.setProfileSettingsStatus("");
      options.setProfileSettingsError("");
      options.setSelectedProfileUserId(null);
      options.setSelectedProfileError("");
    }
  });

  createEffect(() => {
    const session = options.session();
    const value = profile();
    if (!session || !value) {
      return;
    }
    options.setAvatarVersionByUserId((existing) =>
      mergeAvatarVersion(existing, value.userId, value.avatarVersion),
    );
    options.setBannerVersionByUserId((existing) =>
      mergeBannerVersion(existing, value.userId, value.bannerVersion),
    );
    options.setProfileDraftUsername(value.username);
    options.setProfileDraftAbout(value.aboutMarkdown);
  });

  createEffect(() => {
    const session = options.session();
    const value = selectedProfile();
    if (!session || !value) {
      return;
    }
    options.setSelectedProfileError("");
  });

  return {
    profile,
    selectedProfile,
    avatarUrlForUser,
    bannerUrlForUser,
    openUserProfile,
    saveProfileSettings,
    uploadProfileAvatar,
    uploadProfileBanner,
  };
}
