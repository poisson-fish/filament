import { For, Match, Show, Switch } from "solid-js";
import type { SettingsCategory, SettingsCategoryItem, VoiceSettingsSubmenu, VoiceSettingsSubmenuItem } from "../../types";
import type {
  AudioDeviceOption,
  VoiceDevicePreferences,
} from "../../../../lib/voice-device-settings";
import { PROFILE_ABOUT_MAX_CHARS, type ProfileRecord } from "../../../../domain/chat";
import { SafeMarkdown } from "../SafeMarkdown";

export interface SettingsPanelProps {
  settingsCategories: SettingsCategoryItem[];
  voiceSettingsSubmenu: VoiceSettingsSubmenuItem[];
  activeSettingsCategory: SettingsCategory;
  activeVoiceSettingsSubmenu: VoiceSettingsSubmenu;
  voiceDevicePreferences: VoiceDevicePreferences;
  audioInputDevices: AudioDeviceOption[];
  audioOutputDevices: AudioDeviceOption[];
  isRefreshingAudioDevices: boolean;
  audioDevicesStatus: string;
  audioDevicesError: string;
  profile: ProfileRecord | null;
  profileDraftUsername: string;
  profileDraftAbout: string;
  profileAvatarUrl: string | null;
  profileBannerUrl: string | null;
  selectedAvatarFilename: string;
  selectedBannerFilename: string;
  isSavingProfile: boolean;
  isUploadingProfileAvatar: boolean;
  isUploadingProfileBanner: boolean;
  profileStatus: string;
  profileError: string;
  onOpenSettingsCategory: (category: SettingsCategory) => void;
  onOpenVoiceSettingsSubmenu: (submenu: VoiceSettingsSubmenu) => void;
  onSetVoiceDevicePreference: (
    kind: "audioinput" | "audiooutput",
    nextValue: string,
  ) => Promise<void> | void;
  onRefreshAudioDeviceInventory: () => Promise<void> | void;
  onProfileUsernameInput: (value: string) => void;
  onProfileAboutInput: (value: string) => void;
  onSelectProfileAvatarFile: (file: File | null) => void;
  onSelectProfileBannerFile: (file: File | null) => void;
  onSaveProfile: () => Promise<void> | void;
  onUploadProfileAvatar: () => Promise<void> | void;
  onUploadProfileBanner: () => Promise<void> | void;
}

export function SettingsPanel(props: SettingsPanelProps) {
  const settingsNavButtonClass =
    "w-full cursor-pointer rounded-[0.68rem] border border-line-soft bg-bg-3 px-[0.68rem] py-[0.58rem] text-left text-ink-1 transition-colors duration-[140ms] ease-out hover:bg-bg-4 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/60";
  const sectionLabelClassName =
    "m-0 text-[0.68rem] uppercase tracking-[0.08em] text-ink-2";

  const formLabelClass = "grid gap-[0.3rem] text-ink-1 text-[0.84rem] font-medium";
  const formInputClass =
    "bg-bg-1 border border-line-soft rounded-[0.62rem] text-ink-0 px-[0.62rem] py-[0.55rem] focus:outline-none focus:border-brand-strong placeholder-ink-2";
  const formButtonClass =
    "bg-bg-3 border border-line-soft rounded-[0.62rem] text-ink-1 px-[0.72rem] py-[0.46rem] font-medium hover:bg-bg-4 active:bg-bg-2 disabled:opacity-50 disabled:pointer-events-none transition-colors";
  const profileAboutRemainingChars = () => PROFILE_ABOUT_MAX_CHARS - props.profileDraftAbout.length;

  return (
    <section
      class="grid grid-cols-1 items-start gap-[0.95rem] md:grid-cols-[14.5rem_minmax(0,1fr)] md:gap-[1.05rem]"
      aria-label="settings"
    >
      <aside
        class="grid content-start gap-[0.6rem] rounded-[0.78rem] border border-line bg-bg-2 p-[0.74rem]"
        aria-label="Settings category rail"
      >
        <p class={sectionLabelClassName}>CATEGORIES</p>
        <ul class="m-0 grid list-none gap-[0.45rem] p-0">
          <For each={props.settingsCategories}>
            {(category) => {
              const isActive = () => props.activeSettingsCategory === category.id;
              return (
                <li>
                  <button
                    type="button"
                    class={settingsNavButtonClass}
                    classList={{
                      "border-brand/85 bg-brand/20 text-ink-0": isActive(),
                    }}
                    onClick={() => props.onOpenSettingsCategory(category.id)}
                    aria-label={`Open ${category.label} settings category`}
                    aria-current={isActive() ? "page" : undefined}
                  >
                    <div class="text-[0.88rem] font-[700] text-ink-0">{category.label}</div>
                    <div class="muted mt-[0.15rem] text-[0.76rem] font-normal leading-[1.3]">{category.summary}</div>
                  </button>
                </li>
              );
            }}
          </For>
        </ul>
      </aside>
      <section
        class="grid min-w-0 content-start gap-[0.7rem] rounded-[0.78rem] border border-line bg-bg-2 p-[0.8rem]"
        aria-label="Settings content pane"
      >
        <Switch>
          <Match when={props.activeSettingsCategory === "voice"}>
            <section
              class="grid grid-cols-1 items-start gap-[0.8rem] xl:grid-cols-[14.2rem_minmax(0,1fr)] xl:gap-[0.8rem]"
              aria-label="Voice settings submenu"
            >
              <aside
                class="grid content-start gap-[0.6rem] rounded-[0.72rem] border border-line-soft bg-bg-1 p-[0.66rem]"
                aria-label="Voice settings submenu rail"
              >
                <p class={sectionLabelClassName}>VOICE</p>
                <ul class="m-0 grid list-none gap-[0.45rem] p-0">
                  <For each={props.voiceSettingsSubmenu}>
                    {(submenu) => {
                      const isActive = () => props.activeVoiceSettingsSubmenu === submenu.id;
                      return (
                        <li>
                          <button
                            type="button"
                            class={settingsNavButtonClass}
                            classList={{
                              "border-brand/85 bg-brand/20 text-ink-0": isActive(),
                            }}
                            onClick={() => props.onOpenVoiceSettingsSubmenu(submenu.id)}
                            aria-label={`Open Voice ${submenu.label} submenu`}
                            aria-current={isActive() ? "page" : undefined}
                          >
                            <div class="text-[0.88rem] font-[700] text-ink-0">{submenu.label}</div>
                            <div class="muted mt-[0.15rem] text-[0.76rem] font-normal leading-[1.3]">{submenu.summary}</div>
                          </button>
                        </li>
                      );
                    }}
                  </For>
                </ul>
              </aside>
              <section
                class="grid min-w-0 content-start gap-[0.6rem] rounded-[0.72rem] border border-line-soft bg-bg-1 p-[0.72rem]"
                aria-label="Voice settings submenu content"
              >
                <Switch>
                  <Match when={props.activeVoiceSettingsSubmenu === "audio-devices"}>
                    <p class={sectionLabelClassName}>AUDIO DEVICES</p>
                    <form class="grid gap-[0.5rem]" onSubmit={(event) => event.preventDefault()}>
                      <label class={formLabelClass}>
                        Microphone
                        <select
                          class={formInputClass}
                          aria-label="Select microphone device"
                          value={props.voiceDevicePreferences.audioInputDeviceId ?? ""}
                          onChange={(event) =>
                            void props.onSetVoiceDevicePreference(
                              "audioinput",
                              event.currentTarget.value,
                            )
                          }
                          disabled={props.isRefreshingAudioDevices}
                        >
                          <option value="">System default</option>
                          <For each={props.audioInputDevices}>
                            {(device) => <option value={device.deviceId}>{device.label}</option>}
                          </For>
                        </select>
                      </label>
                      <label class={formLabelClass}>
                        Speaker
                        <select
                          class={formInputClass}
                          aria-label="Select speaker device"
                          value={props.voiceDevicePreferences.audioOutputDeviceId ?? ""}
                          onChange={(event) =>
                            void props.onSetVoiceDevicePreference(
                              "audiooutput",
                              event.currentTarget.value,
                            )
                          }
                          disabled={props.isRefreshingAudioDevices}
                        >
                          <option value="">System default</option>
                          <For each={props.audioOutputDevices}>
                            {(device) => <option value={device.deviceId}>{device.label}</option>}
                          </For>
                        </select>
                      </label>
                      <div class="flex pt-2">
                        <button
                          type="button"
                          class={formButtonClass}
                          onClick={() => void props.onRefreshAudioDeviceInventory()}
                          disabled={props.isRefreshingAudioDevices}
                        >
                          {props.isRefreshingAudioDevices ? "Refreshing..." : "Refresh devices"}
                        </button>
                      </div>
                    </form>
                    <Show when={props.audioDevicesStatus}>
                      <p class="status ok">{props.audioDevicesStatus}</p>
                    </Show>
                    <Show when={props.audioDevicesError}>
                      <p class="status error">{props.audioDevicesError}</p>
                    </Show>
                    <Show
                      when={
                        !props.isRefreshingAudioDevices &&
                        props.audioInputDevices.length === 0 &&
                        props.audioOutputDevices.length === 0 &&
                        !props.audioDevicesError
                      }
                    >
                      <p class="muted">
                        No audio devices were detected yet. Select Refresh devices to request
                        microphone access, then retry.
                      </p>
                    </Show>
                  </Match>
                </Switch>
              </section>
            </section>
          </Match>
          <Match when={props.activeSettingsCategory === "profile"}>
            <p class={sectionLabelClassName}>PROFILE</p>
            <form
              class="grid gap-[0.5rem]"
              onSubmit={(event) => {
                event.preventDefault();
                void props.onSaveProfile();
              }}
            >
              <label class={formLabelClass}>
                Username
                <input
                  class={formInputClass}
                  aria-label="Profile username"
                  value={props.profileDraftUsername}
                  maxlength="32"
                  onInput={(event) => props.onProfileUsernameInput(event.currentTarget.value)}
                />
              </label>
              <label class={formLabelClass}>
                About (Markdown)
                <textarea
                  class={`${formInputClass} resize-y min-h-[6rem]`}
                  aria-label="Profile about markdown"
                  value={props.profileDraftAbout}
                  maxlength={PROFILE_ABOUT_MAX_CHARS}
                  onInput={(event) => props.onProfileAboutInput(event.currentTarget.value)}
                  rows="6"
                />
              </label>
              <p
                class="m-0 text-[0.76rem] text-ink-2"
                classList={{
                  "text-danger": profileAboutRemainingChars() <= 128,
                }}
                aria-live="polite"
                role="status"
              >
                {profileAboutRemainingChars()} characters remaining ({props.profileDraftAbout.length}/
                {PROFILE_ABOUT_MAX_CHARS})
              </p>
              <div class="flex gap-2">
                <button
                  type="submit"
                  class={formButtonClass}
                  disabled={props.isSavingProfile}
                >
                  {props.isSavingProfile ? "Saving..." : "Save profile"}
                </button>
              </div>
            </form>
            <div class="grid gap-[0.5rem]">
              <label class={formLabelClass}>
                Banner image
                <input
                  class={formInputClass}
                  type="file"
                  accept="image/png,image/jpeg,image/webp,image/gif,image/avif"
                  aria-label="Profile banner file input"
                  onInput={(event) =>
                    props.onSelectProfileBannerFile(event.currentTarget.files?.[0] ?? null)}
                />
              </label>
              <Show when={props.selectedBannerFilename}>
                <p class="muted">Selected: {props.selectedBannerFilename}</p>
              </Show>
              <div class="flex gap-2">
                <button
                  type="button"
                  class={formButtonClass}
                  onClick={() => void props.onUploadProfileBanner()}
                  disabled={props.isUploadingProfileBanner || props.selectedBannerFilename.length === 0}
                >
                  {props.isUploadingProfileBanner ? "Uploading..." : "Upload banner"}
                </button>
              </div>
            </div>
            <div class="grid gap-[0.5rem]">
              <label class={formLabelClass}>
                Avatar image
                <input
                  class={formInputClass}
                  type="file"
                  accept="image/png,image/jpeg,image/webp,image/gif,image/avif"
                  aria-label="Profile avatar file input"
                  onInput={(event) =>
                    props.onSelectProfileAvatarFile(event.currentTarget.files?.[0] ?? null)}
                />
              </label>
              <Show when={props.selectedAvatarFilename}>
                <p class="muted">Selected: {props.selectedAvatarFilename}</p>
              </Show>
              <div class="flex gap-2">
                <button
                  type="button"
                  class={formButtonClass}
                  onClick={() => void props.onUploadProfileAvatar()}
                  disabled={props.isUploadingProfileAvatar || props.selectedAvatarFilename.length === 0}
                >
                  {props.isUploadingProfileAvatar ? "Uploading..." : "Upload avatar"}
                </button>
              </div>
            </div>
            <Show when={props.profileStatus}>
              <p class="status ok">{props.profileStatus}</p>
            </Show>
            <Show when={props.profileError}>
              <p class="status error">{props.profileError}</p>
            </Show>
            <Show when={props.profile}>
              {(profile) => (
                <section class="grid gap-[0.75rem] rounded-[0.8rem] border border-line bg-bg-2 p-[1rem] shadow-sm">
                  <p class={sectionLabelClassName}>PROFILE PREVIEW</p>
                  <Show when={props.profileBannerUrl}>
                    <img
                      class="h-[6rem] w-full rounded-[0.62rem] border border-line-soft object-cover"
                      src={props.profileBannerUrl!}
                      alt={`${profile().username} banner`}
                      loading="lazy"
                      decoding="async"
                      referrerPolicy="no-referrer"
                      onError={(event) => {
                        event.currentTarget.style.display = "none";
                      }}
                    />
                  </Show>
                  <div class="flex items-center gap-[0.8rem]">
                    <span
                      class="relative inline-flex h-[3.2rem] w-[3.2rem] items-center justify-center overflow-hidden rounded-full border border-line-soft bg-gradient-to-br from-bg-4 to-bg-3 text-[1rem] font-[780] text-ink-0 shadow-sm"
                      aria-hidden="true"
                    >
                      <span class="z-[1]">{profile().username.slice(0, 1).toUpperCase()}</span>
                      <Show when={props.profileAvatarUrl}>
                        <img
                          class="absolute inset-0 z-[2] h-full w-full rounded-full object-cover"
                          src={props.profileAvatarUrl!}
                          alt={`${profile().username} avatar`}
                          loading="lazy"
                          decoding="async"
                          referrerPolicy="no-referrer"
                          onError={(event) => {
                            event.currentTarget.style.display = "none";
                          }}
                        />
                      </Show>
                    </span>
                    <div>
                      <p class="m-0 text-lg font-[760] text-ink-0 leading-tight">{profile().username}</p>
                      <p class="m-0 mt-0.5 text-[0.82rem] font-code text-ink-2">{profile().userId}</p>
                    </div>
                  </div>
                  <SafeMarkdown
                    class="leading-[1.5] text-ink-1 [&_ol]:m-[0.5rem_0_0.5rem_1.2rem] [&_ol]:p-0 [&_p+p]:mt-[0.6rem] [&_p]:m-0 [&_ul]:m-[0.5rem_0_0.5rem_1.2rem] [&_ul]:p-0 rounded-[0.5rem] bg-bg-3 p-[0.75rem]"
                    tokens={profile().aboutMarkdownTokens}
                  />
                </section>
              )}
            </Show>
          </Match>
        </Switch>
      </section>
    </section>
  );
}
