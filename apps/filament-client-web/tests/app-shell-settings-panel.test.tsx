import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import { userIdFromInput, type ProfileRecord } from "../src/domain/chat";
import { SettingsPanel, type SettingsPanelProps } from "../src/features/app-shell/components/panels/SettingsPanel";
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
    selectedAvatarFilename: "",
    isSavingProfile: false,
    isUploadingProfileAvatar: false,
    profileStatus: "",
    profileError: "",
    onOpenSettingsCategory: () => undefined,
    onOpenVoiceSettingsSubmenu: () => undefined,
    onSetVoiceDevicePreference: () => undefined,
    onRefreshAudioDeviceInventory: () => undefined,
    onProfileUsernameInput: () => undefined,
    onProfileAboutInput: () => undefined,
    onSelectProfileAvatarFile: () => undefined,
    onSaveProfile: () => undefined,
    onUploadProfileAvatar: () => undefined,
    ...overrides,
  };
}

describe("app shell settings panel", () => {
  it("renders with Uno utility classes and without legacy settings hooks", () => {
    render(() => <SettingsPanel {...settingsPanelPropsFixture()} />);

    const layout = screen.getByLabelText("settings");
    expect(layout).toHaveClass("grid");
    expect(layout).toHaveClass("min-h-[24rem]");

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
    const onSaveProfile = vi.fn();
    const onUploadProfileAvatar = vi.fn();

    render(() => (
      <SettingsPanel
        {...settingsPanelPropsFixture({
          activeSettingsCategory: "profile",
          selectedAvatarFilename: "avatar.png",
          profileAvatarUrl: "https://example.test/avatar.png",
          onOpenSettingsCategory,
          onProfileUsernameInput,
          onProfileAboutInput,
          onSelectProfileAvatarFile,
          onSaveProfile,
          onUploadProfileAvatar,
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

    const avatarFileInput = screen.getByLabelText("Profile avatar file input") as HTMLInputElement;
    const avatarFile = new File(["avatar-bytes"], "avatar.png", { type: "image/png" });
    Object.defineProperty(avatarFileInput, "files", {
      configurable: true,
      value: [avatarFile],
    });
    await fireEvent.input(avatarFileInput);
    expect(onSelectProfileAvatarFile).toHaveBeenCalledWith(avatarFile);

    await fireEvent.click(screen.getByRole("button", { name: "Save profile" }));
    await fireEvent.click(screen.getByRole("button", { name: "Upload avatar" }));
    expect(onSaveProfile).toHaveBeenCalledOnce();
    expect(onUploadProfileAvatar).toHaveBeenCalledOnce();

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
});
