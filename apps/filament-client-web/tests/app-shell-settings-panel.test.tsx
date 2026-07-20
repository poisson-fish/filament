import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import { PROFILE_ABOUT_MAX_CHARS, userIdFromInput, type ProfileRecord } from "../src/domain/chat";
import {
  ROTATE_IDENTITY_CONFIRMATION,
  SettingsPanel,
  type SettingsPanelProps,
} from "../src/features/app-shell/components/panels/SettingsPanel";
import { mediaDeviceIdFromInput } from "../src/lib/voice-device-settings";

const PROFILE_USER_ID = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAZ");

function profileFixture(): ProfileRecord {
  return {
    userId: PROFILE_USER_ID,
    username: "owner",
    aboutMarkdown: "hello\n\nteam",
    aboutMarkdownTokens: [
      { type: "paragraph_start" },
      { type: "text", text: "hello" },
      { type: "paragraph_end" },
      { type: "paragraph_start" },
      { type: "text", text: "team" },
      { type: "paragraph_end" },
    ],
    avatarVersion: 1,
    bannerVersion: 1,
  };
}

function settingsPanelPropsFixture(
  overrides: Partial<SettingsPanelProps> = {},
): SettingsPanelProps {
  return {
    settingsCategories: [
      { id: "voice", label: "Voice", summary: "Input, output, diagnostics" },
      { id: "profile", label: "Profile", summary: "Identity and avatar" },
    ],
    voiceSettingsSubmenu: [
      { id: "audio-devices", label: "Audio Devices", summary: "Hardware routing" },
    ],
    activeSettingsCategory: "voice",
    activeVoiceSettingsSubmenu: "audio-devices",
    voiceDevicePreferences: {
      audioInputDeviceId: mediaDeviceIdFromInput("mic-1"),
      audioOutputDeviceId: mediaDeviceIdFromInput("spk-1"),
    },
    audioInputDevices: [
      { kind: "audioinput", deviceId: mediaDeviceIdFromInput("mic-1"), label: "Desk Mic" },
    ],
    audioOutputDevices: [
      { kind: "audiooutput", deviceId: mediaDeviceIdFromInput("spk-1"), label: "Desk Speaker" },
    ],
    isRefreshingAudioDevices: false,
    audioDevicesStatus: "",
    audioDevicesError: "",
    profile: profileFixture(),
    profileDraftUsername: "owner",
    profileDraftAbout: "hello",
    profileAvatarUrl: null,
    profileBannerUrl: null,
    selectedAvatarFilename: "",
    selectedBannerFilename: "",
    isSavingProfile: false,
    isUploadingProfileAvatar: false,
    isUploadingProfileBanner: false,
    profileStatus: "",
    profileError: "",
    onOpenSettingsCategory: () => undefined,
    onOpenVoiceSettingsSubmenu: () => undefined,
    onSetVoiceDevicePreference: () => undefined,
    onRefreshAudioDeviceInventory: () => undefined,
    onProfileUsernameInput: () => undefined,
    onProfileAboutInput: () => undefined,
    onSelectProfileAvatarFile: () => undefined,
    onSelectProfileBannerFile: () => undefined,
    onSaveProfile: () => undefined,
    onUploadProfileAvatar: () => undefined,
    onUploadProfileBanner: () => undefined,
    ...overrides,
  };
}

describe("app shell settings panel", () => {
  it("renders with Uno utility classes and without legacy settings hooks", () => {
    render(() => <SettingsPanel {...settingsPanelPropsFixture()} />);

    const layout = screen.getByLabelText("settings");
    expect(layout).toHaveClass("grid");
    expect(layout).toHaveClass("items-start");

    const categoryRail = screen.getByLabelText("Settings category rail");
    expect(categoryRail).toHaveClass("rounded-[0.78rem]");
    expect(categoryRail).toHaveClass("bg-bg-2");

    const voiceCategoryButton = screen.getByRole("button", {
      name: "Open Voice settings category",
    });
    expect(voiceCategoryButton).toHaveClass("rounded-[0.68rem]");
    expect(voiceCategoryButton).toHaveClass("border-brand/85");
    expect(voiceCategoryButton).toHaveClass("bg-brand/20");

    const voiceSubmenuButton = screen.getByRole("button", {
      name: "Open Voice Audio Devices submenu",
    });
    expect(voiceSubmenuButton).toHaveClass("bg-brand/20");

    expect(document.querySelector(".settings-panel-layout")).toBeNull();
    expect(document.querySelector(".settings-category-button")).toBeNull();
    expect(document.querySelector(".settings-submenu-button")).toBeNull();
    expect(document.querySelector(".group-label")).toBeNull();
    expect(document.querySelector(".inline-form")).toBeNull();
  });

  it("keeps callbacks and profile-preview behavior intact", async () => {
    const onOpenSettingsCategory = vi.fn();
    const onProfileUsernameInput = vi.fn();
    const onProfileAboutInput = vi.fn();
    const onSelectProfileAvatarFile = vi.fn();
    const onSelectProfileBannerFile = vi.fn();
    const onSaveProfile = vi.fn();
    const onUploadProfileAvatar = vi.fn();
    const onUploadProfileBanner = vi.fn();

    render(() => (
      <SettingsPanel
        {...settingsPanelPropsFixture({
          activeSettingsCategory: "profile",
          selectedAvatarFilename: "avatar.png",
          selectedBannerFilename: "banner.png",
          profileAvatarUrl: "https://example.test/avatar.png",
          profileBannerUrl: "https://example.test/banner.png",
          onOpenSettingsCategory,
          onProfileUsernameInput,
          onProfileAboutInput,
          onSelectProfileAvatarFile,
          onSelectProfileBannerFile,
          onSaveProfile,
          onUploadProfileAvatar,
          onUploadProfileBanner,
        })}
      />
    ));

    await fireEvent.click(screen.getByRole("button", { name: "Open Voice settings category" }));
    expect(onOpenSettingsCategory).toHaveBeenCalledWith("voice");

    await fireEvent.input(screen.getByLabelText("Profile username"), {
      target: { value: "owner-updated" },
    });
    expect(onProfileUsernameInput).toHaveBeenCalledWith("owner-updated");

    await fireEvent.input(screen.getByLabelText("Profile about markdown"), {
      target: { value: "updated" },
    });
    expect(onProfileAboutInput).toHaveBeenCalledWith("updated");

    const bannerFileInput = screen.getByLabelText("Profile banner file input") as HTMLInputElement;
    const bannerFile = new File(["banner-bytes"], "banner.png", { type: "image/png" });
    Object.defineProperty(bannerFileInput, "files", {
      configurable: true,
      value: [bannerFile],
    });
    await fireEvent.input(bannerFileInput);
    expect(onSelectProfileBannerFile).toHaveBeenCalledWith(bannerFile);

    const avatarFileInput = screen.getByLabelText("Profile avatar file input") as HTMLInputElement;
    const avatarFile = new File(["avatar-bytes"], "avatar.png", { type: "image/png" });
    Object.defineProperty(avatarFileInput, "files", {
      configurable: true,
      value: [avatarFile],
    });
    await fireEvent.input(avatarFileInput);
    expect(onSelectProfileAvatarFile).toHaveBeenCalledWith(avatarFile);

    await fireEvent.click(screen.getByRole("button", { name: "Save profile" }));
    await fireEvent.click(screen.getByRole("button", { name: "Upload banner" }));
    await fireEvent.click(screen.getByRole("button", { name: "Upload avatar" }));
    expect(onSaveProfile).toHaveBeenCalledOnce();
    expect(onUploadProfileBanner).toHaveBeenCalledOnce();
    expect(onUploadProfileAvatar).toHaveBeenCalledOnce();

    const bannerImage = screen.getByAltText("owner banner");
    await fireEvent.error(bannerImage);
    expect(bannerImage).toHaveStyle({ display: "none" });

    const avatarImage = screen.getByAltText("owner avatar");
    await fireEvent.error(avatarImage);
    expect(avatarImage).toHaveStyle({ display: "none" });
    expect(screen.getByText("hello").parentElement).toHaveClass("text-ink-1");
    expect(screen.getByText(PROFILE_USER_ID)).toHaveClass("font-code");
    expect(screen.getByText(PROFILE_USER_ID)).toHaveClass("text-[0.82rem]");
    expect(document.querySelector(".settings-profile-preview")).toBeNull();
    expect(document.querySelector(".settings-profile-markdown")).toBeNull();
    expect(document.querySelector(".mono")).toBeNull();
  });

  it("shows profile about remaining-character feedback at the 2048 cap", () => {
    render(() => (
      <SettingsPanel
        {...settingsPanelPropsFixture({
          activeSettingsCategory: "profile",
          profileDraftAbout: "A".repeat(PROFILE_ABOUT_MAX_CHARS),
        })}
      />
    ));

    const counter = screen.getByRole("status");
    expect(counter).toHaveTextContent(`0 characters remaining (${PROFILE_ABOUT_MAX_CHARS}/${PROFILE_ABOUT_MAX_CHARS})`);
    expect(counter).toHaveClass("text-danger");
  });

  it("renders redacted encryption state and gates destructive rotation on exact confirmation", async () => {
    const onRotationConfirmationInput = vi.fn();
    const onRotateIdentity = vi.fn();
    const { unmount } = render(() => (
      <SettingsPanel
        {...settingsPanelPropsFixture({
          settingsCategories: [
            { id: "voice", label: "Voice", summary: "Input and output" },
            { id: "profile", label: "Profile", summary: "Identity and avatar" },
            { id: "encryption", label: "Encryption", summary: "Device-bound keys" },
          ],
          activeSettingsCategory: "encryption",
          encryptionSettings: {
            ready: true,
            safetyNumber: "0123456789abcdef0123456789abcdef",
            rotationSequence: 2,
            devices: [{
              deviceId: "01ARZ3NDEKTSV4RRFFQ69G5FAW",
              addedAtUnix: 1_700_000_000,
              isCurrentDevice: true,
              verification: "verified",
            }],
            backupEnrolled: false,
          },
          rotationConfirmation: "ROTATE",
          onRotationConfirmationInput,
          onRotateIdentity,
        })}
      />
    ));

    expect(screen.getByText("0123456789abcdef0123456789abcdef")).toBeInTheDocument();
    expect(screen.getByText("01ARZ3NDEKTSV4RRFFQ69G5FAW")).toBeInTheDocument();
    const rotate = screen.getByRole("button", { name: "Rotate identity and revoke other devices" });
    expect(rotate).toBeDisabled();
    await fireEvent.input(screen.getByLabelText("Identity rotation confirmation"), {
      target: { value: ROTATE_IDENTITY_CONFIRMATION },
    });
    expect(onRotationConfirmationInput).toHaveBeenCalledWith(ROTATE_IDENTITY_CONFIRMATION);
    unmount();

    render(() => (
      <SettingsPanel
        {...settingsPanelPropsFixture({
          activeSettingsCategory: "encryption",
          encryptionSettings: {
            ready: true,
            safetyNumber: "0123456789abcdef0123456789abcdef",
            rotationSequence: 2,
            devices: [],
            backupEnrolled: false,
          },
          rotationConfirmation: ROTATE_IDENTITY_CONFIRMATION,
          onRotateIdentity,
        })}
      />
    ));
    const enabledRotate = screen.getByRole("button", { name: "Rotate identity and revoke other devices" });
    expect(enabledRotate).toBeEnabled();
    await fireEvent.click(enabledRotate);
    expect(onRotateIdentity).toHaveBeenCalledOnce();
  });
});
