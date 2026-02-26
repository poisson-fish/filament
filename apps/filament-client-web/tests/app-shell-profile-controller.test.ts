import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import { PROFILE_ABOUT_MAX_CHARS, profileFromResponse, userIdFromInput } from "../src/domain/chat";
import { createProfileController } from "../src/features/app-shell/controllers/profile-controller";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

const USER_ID = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");

function profileFixture(input: {
  userId: string;
  username: string;
  aboutMarkdown?: string;
  avatarVersion?: number;
}) {
  return profileFromResponse({
    user_id: input.userId,
    username: input.username,
    about_markdown: input.aboutMarkdown ?? "",
    about_markdown_tokens: [{ type: "text", text: input.aboutMarkdown ?? "" }],
    avatar_version: input.avatarVersion ?? 0,
  });
}

async function flush(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

async function flushUntil(predicate: () => boolean): Promise<void> {
  for (let index = 0; index < 8; index += 1) {
    if (predicate()) {
      return;
    }
    await flush();
  }
  throw new Error("condition not reached");
}

describe("app shell profile controller", () => {
  it("loads profile and runs profile save + avatar upload actions", async () => {
    const [session] = createSignal(SESSION);
    const [selectedProfileUserId, setSelectedProfileUserId] = createSignal<ReturnType<
      typeof userIdFromInput
    > | null>(null);
    const [avatarVersionByUserId, setAvatarVersionByUserId] = createSignal<
      Record<string, number>
    >({});
    const [profileDraftUsername, setProfileDraftUsername] = createSignal("");
    const [profileDraftAbout, setProfileDraftAbout] = createSignal("");
    const [selectedProfileAvatarFile, setSelectedProfileAvatarFile] = createSignal<File | null>(
      null,
    );
    const [isSavingProfile, setSavingProfile] = createSignal(false);
    const [isUploadingProfileAvatar, setUploadingProfileAvatar] = createSignal(false);
    const [profileSettingsStatus, setProfileSettingsStatus] = createSignal("");
    const [profileSettingsError, setProfileSettingsError] = createSignal("");
    const [selectedProfileError, setSelectedProfileError] = createSignal("");

    const fetchMeMock = vi.fn(async () =>
      profileFixture({
        userId: USER_ID,
        username: "alice",
        aboutMarkdown: "initial",
        avatarVersion: 1,
      }),
    );
    const fetchUserProfileMock = vi.fn(async () =>
      profileFixture({
        userId: USER_ID,
        username: "alice",
        aboutMarkdown: "public",
        avatarVersion: 3,
      }),
    );
    const updateMyProfileMock = vi.fn(async () =>
      profileFixture({
        userId: USER_ID,
        username: "aliceupdated",
        aboutMarkdown: "updated about",
        avatarVersion: 1,
      }),
    );
    const uploadMyProfileAvatarMock = vi.fn(async () =>
      profileFixture({
        userId: USER_ID,
        username: "aliceupdated",
        aboutMarkdown: "updated about",
        avatarVersion: 2,
      }),
    );

    const controller = createRoot(() =>
      createProfileController(
        {
          session,
          selectedProfileUserId,
          avatarVersionByUserId,
          profileDraftUsername,
          profileDraftAbout,
          selectedProfileAvatarFile,
          isSavingProfile,
          isUploadingProfileAvatar,
          setProfileDraftUsername,
          setProfileDraftAbout,
          setSelectedProfileAvatarFile,
          setProfileSettingsStatus,
          setProfileSettingsError,
          setSavingProfile,
          setUploadingProfileAvatar,
          setAvatarVersionByUserId,
          setSelectedProfileUserId,
          setSelectedProfileError,
        },
        {
          fetchMe: fetchMeMock,
          fetchUserProfile: fetchUserProfileMock,
          updateMyProfile: updateMyProfileMock,
          uploadMyProfileAvatar: uploadMyProfileAvatarMock,
        },
      ),
    );

    await flushUntil(() => Boolean(controller.profile()));
    expect(controller.profile()?.username).toBe("alice");
    expect(profileDraftUsername()).toBe("alice");
    expect(profileDraftAbout()).toBe("initial");
    expect(avatarVersionByUserId()[USER_ID]).toBe(1);

    setProfileDraftUsername("aliceupdated");
    setProfileDraftAbout("updated about");
    await controller.saveProfileSettings();
    expect(updateMyProfileMock).toHaveBeenCalledTimes(1);
    expect(profileSettingsStatus()).toBe("Profile updated.");
    expect(profileSettingsError()).toBe("");

    setSelectedProfileAvatarFile(
      new File(["avatar"], "avatar.png", {
        type: "image/png",
      }),
    );
    await controller.uploadProfileAvatar();
    expect(uploadMyProfileAvatarMock).toHaveBeenCalledTimes(1);
    expect(selectedProfileAvatarFile()).toBeNull();
    expect(profileSettingsStatus()).toBe("Profile avatar updated.");
    expect(avatarVersionByUserId()[USER_ID]).toBe(2);

    controller.openUserProfile("invalid");
    expect(selectedProfileError()).toBe("User profile is unavailable.");

    controller.openUserProfile(USER_ID);
    await flush();
    expect(fetchUserProfileMock).toHaveBeenCalledWith(SESSION, USER_ID);
    expect(selectedProfileError()).toBe("");
  });

  it("resets profile state when auth is cleared", async () => {
    const [session, setSession] = createSignal<typeof SESSION | null>(SESSION);
    const [selectedProfileUserId, setSelectedProfileUserId] = createSignal<ReturnType<
      typeof userIdFromInput
    > | null>(USER_ID);
    const [avatarVersionByUserId, setAvatarVersionByUserId] = createSignal<
      Record<string, number>
    >({
      [USER_ID]: 3,
    });
    const [profileDraftUsername, setProfileDraftUsername] = createSignal("alice");
    const [profileDraftAbout, setProfileDraftAbout] = createSignal("about");
    const [selectedProfileAvatarFile, setSelectedProfileAvatarFile] = createSignal<File | null>(
      new File(["avatar"], "avatar.png", {
        type: "image/png",
      }),
    );
    const [isSavingProfile, setSavingProfile] = createSignal(true);
    const [isUploadingProfileAvatar, setUploadingProfileAvatar] = createSignal(true);
    const [profileSettingsStatus, setProfileSettingsStatus] = createSignal("status");
    const [profileSettingsError, setProfileSettingsError] = createSignal("error");
    const [selectedProfileError, setSelectedProfileError] = createSignal("error");

    createRoot(() =>
      createProfileController(
        {
          session,
          selectedProfileUserId,
          avatarVersionByUserId,
          profileDraftUsername,
          profileDraftAbout,
          selectedProfileAvatarFile,
          isSavingProfile,
          isUploadingProfileAvatar,
          setProfileDraftUsername,
          setProfileDraftAbout,
          setSelectedProfileAvatarFile,
          setProfileSettingsStatus,
          setProfileSettingsError,
          setSavingProfile,
          setUploadingProfileAvatar,
          setAvatarVersionByUserId,
          setSelectedProfileUserId,
          setSelectedProfileError,
        },
        {
          fetchMe: async () =>
            profileFixture({
              userId: USER_ID,
              username: "alice",
              aboutMarkdown: "",
              avatarVersion: 3,
            }),
          fetchUserProfile: async () =>
            profileFixture({
              userId: USER_ID,
              username: "alice",
              aboutMarkdown: "",
              avatarVersion: 3,
            }),
        },
      ),
    );

    setSession(null);
    await flush();

    expect(isSavingProfile()).toBe(false);
    expect(isUploadingProfileAvatar()).toBe(false);
    expect(profileDraftUsername()).toBe("");
    expect(profileDraftAbout()).toBe("");
    expect(selectedProfileAvatarFile()).toBeNull();
    expect(profileSettingsStatus()).toBe("");
    expect(profileSettingsError()).toBe("");
    expect(selectedProfileUserId()).toBeNull();
    expect(selectedProfileError()).toBe("");
  });

  it("bumps avatar version locally after upload when server returns stale version", async () => {
    const [session] = createSignal(SESSION);
    const [selectedProfileUserId, setSelectedProfileUserId] = createSignal<ReturnType<
      typeof userIdFromInput
    > | null>(null);
    const [avatarVersionByUserId, setAvatarVersionByUserId] = createSignal<
      Record<string, number>
    >({
      [USER_ID]: 5,
    });
    const [profileDraftUsername, setProfileDraftUsername] = createSignal("");
    const [profileDraftAbout, setProfileDraftAbout] = createSignal("");
    const [selectedProfileAvatarFile, setSelectedProfileAvatarFile] = createSignal<File | null>(
      null,
    );
    const [isSavingProfile, setSavingProfile] = createSignal(false);
    const [isUploadingProfileAvatar, setUploadingProfileAvatar] = createSignal(false);
    const [, setProfileSettingsStatus] = createSignal("");
    const [, setProfileSettingsError] = createSignal("");
    const [, setSelectedProfileError] = createSignal("");

    const controller = createRoot(() =>
      createProfileController(
        {
          session,
          selectedProfileUserId,
          avatarVersionByUserId,
          profileDraftUsername,
          profileDraftAbout,
          selectedProfileAvatarFile,
          isSavingProfile,
          isUploadingProfileAvatar,
          setProfileDraftUsername,
          setProfileDraftAbout,
          setSelectedProfileAvatarFile,
          setProfileSettingsStatus,
          setProfileSettingsError,
          setSavingProfile,
          setUploadingProfileAvatar,
          setAvatarVersionByUserId,
          setSelectedProfileUserId,
          setSelectedProfileError,
        },
        {
          fetchMe: async () =>
            profileFixture({
              userId: USER_ID,
              username: "alice",
              aboutMarkdown: "",
              avatarVersion: 5,
            }),
          fetchUserProfile: async () =>
            profileFixture({
              userId: USER_ID,
              username: "alice",
              aboutMarkdown: "",
              avatarVersion: 5,
            }),
          uploadMyProfileAvatar: async () =>
            profileFixture({
              userId: USER_ID,
              username: "alice",
              aboutMarkdown: "",
              avatarVersion: 5,
            }),
        },
      ),
    );

    await flushUntil(() => Boolean(controller.profile()));
    setSelectedProfileAvatarFile(
      new File(["avatar"], "avatar.png", {
        type: "image/png",
      }),
    );

    await controller.uploadProfileAvatar();
    expect(avatarVersionByUserId()[USER_ID]).toBe(6);
  });

  it("rejects oversized profile about markdown before calling the API", async () => {
    const [session] = createSignal(SESSION);
    const [selectedProfileUserId, setSelectedProfileUserId] = createSignal<ReturnType<
      typeof userIdFromInput
    > | null>(null);
    const [avatarVersionByUserId, setAvatarVersionByUserId] = createSignal<Record<string, number>>(
      {},
    );
    const [profileDraftUsername, setProfileDraftUsername] = createSignal("alice");
    const [profileDraftAbout, setProfileDraftAbout] = createSignal("");
    const [selectedProfileAvatarFile, setSelectedProfileAvatarFile] = createSignal<File | null>(
      null,
    );
    const [isSavingProfile, setSavingProfile] = createSignal(false);
    const [isUploadingProfileAvatar, setUploadingProfileAvatar] = createSignal(false);
    const [profileSettingsStatus, setProfileSettingsStatus] = createSignal("");
    const [profileSettingsError, setProfileSettingsError] = createSignal("");
    const [selectedProfileError, setSelectedProfileError] = createSignal("");

    const updateMyProfileMock = vi.fn(async () =>
      profileFixture({
        userId: USER_ID,
        username: "alice",
        aboutMarkdown: "",
        avatarVersion: 1,
      }),
    );

    const controller = createRoot(() =>
      createProfileController(
        {
          session,
          selectedProfileUserId,
          avatarVersionByUserId,
          profileDraftUsername,
          profileDraftAbout,
          selectedProfileAvatarFile,
          isSavingProfile,
          isUploadingProfileAvatar,
          setProfileDraftUsername,
          setProfileDraftAbout,
          setSelectedProfileAvatarFile,
          setProfileSettingsStatus,
          setProfileSettingsError,
          setSavingProfile,
          setUploadingProfileAvatar,
          setAvatarVersionByUserId,
          setSelectedProfileUserId,
          setSelectedProfileError,
        },
        {
          fetchMe: async () =>
            profileFixture({
              userId: USER_ID,
              username: "alice",
              aboutMarkdown: "",
              avatarVersion: 1,
            }),
          fetchUserProfile: async () =>
            profileFixture({
              userId: USER_ID,
              username: "alice",
              aboutMarkdown: "",
              avatarVersion: 1,
            }),
          updateMyProfile: updateMyProfileMock,
        },
      ),
    );

    await flushUntil(() => Boolean(controller.profile()));
    setProfileDraftAbout("A".repeat(PROFILE_ABOUT_MAX_CHARS + 1));
    await controller.saveProfileSettings();

    expect(updateMyProfileMock).not.toHaveBeenCalled();
    expect(isSavingProfile()).toBe(false);
    expect(profileSettingsStatus()).toBe("");
    expect(profileSettingsError()).toBe(
      `About must be 0-${PROFILE_ABOUT_MAX_CHARS} characters.`,
    );
  });
});
