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
  updateMyProfile,
  uploadMyProfileAvatar,
} from "../../../lib/api";
import { mapError } from "../helpers";

export interface ProfileControllerOptions {
  session: Accessor<AuthSession | null>;
  selectedProfileUserId: Accessor<UserId | null>;
  avatarVersionByUserId: Accessor<Record<string, number>>;
  profileDraftUsername: Accessor<string>;
  profileDraftAbout: Accessor<string>;
  selectedProfileAvatarFile: Accessor<File | null>;
  isSavingProfile: Accessor<boolean>;
  isUploadingProfileAvatar: Accessor<boolean>;
  setProfileDraftUsername: Setter<string>;
  setProfileDraftAbout: Setter<string>;
  setSelectedProfileAvatarFile: Setter<File | null>;
  setProfileSettingsStatus: Setter<string>;
  setProfileSettingsError: Setter<string>;
  setSavingProfile: Setter<boolean>;
  setUploadingProfileAvatar: Setter<boolean>;
  setAvatarVersionByUserId: Setter<Record<string, number>>;
  setSelectedProfileUserId: Setter<UserId | null>;
  setSelectedProfileError: Setter<string>;
}

function mergeAvatarVersion(
  existing: Record<string, number>,
  userId: string,
  avatarVersion: number,
): Record<string, number> {
  if (existing[userId] === avatarVersion) {
    return existing;
  }
  return {
    ...existing,
    [userId]: avatarVersion,
  };
}

export interface ProfileControllerDependencies {
  fetchMe: typeof fetchMe;
  fetchUserProfile: typeof fetchUserProfile;
  updateMyProfile: typeof updateMyProfile;
  uploadMyProfileAvatar: typeof uploadMyProfileAvatar;
  profileAvatarUrl: typeof profileAvatarUrl;
}

const DEFAULT_PROFILE_CONTROLLER_DEPENDENCIES: ProfileControllerDependencies = {
  fetchMe,
  fetchUserProfile,
  updateMyProfile,
  uploadMyProfileAvatar,
  profileAvatarUrl,
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

  const saveProfileSettings = async (): Promise<void> => {
    const session = options.session();
    const currentProfile = profile();
    if (!session || !currentProfile || options.isSavingProfile()) {
      return;
    }

    options.setSavingProfile(true);
    options.setProfileSettingsStatus("");
    options.setProfileSettingsError("");
    try {
      const nextUsername = usernameFromInput(options.profileDraftUsername().trim());
      const nextAbout = profileAboutFromInput(options.profileDraftAbout());
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
      mutateProfile(updated);
      options.setAvatarVersionByUserId((existing) =>
        mergeAvatarVersion(existing, updated.userId, updated.avatarVersion),
      );
      options.setSelectedProfileAvatarFile(null);
      options.setProfileSettingsStatus("Profile avatar updated.");
    } catch (error) {
      options.setProfileSettingsError(mapError(error, "Unable to upload profile avatar."));
    } finally {
      options.setUploadingProfileAvatar(false);
    }
  };

  createEffect(() => {
    const session = options.session();
    if (!session) {
      options.setSavingProfile(false);
      options.setUploadingProfileAvatar(false);
      options.setProfileDraftUsername("");
      options.setProfileDraftAbout("");
      options.setSelectedProfileAvatarFile(null);
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
    openUserProfile,
    saveProfileSettings,
    uploadProfileAvatar,
  };
}
