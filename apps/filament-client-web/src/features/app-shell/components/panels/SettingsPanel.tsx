import { For, Match, Show, Switch } from "solid-js";
import type { SettingsCategory, SettingsCategoryItem, VoiceSettingsSubmenu, VoiceSettingsSubmenuItem } from "../../types";
import type {
  AudioDeviceOption,
  VoiceDevicePreferences,
} from "../../../../lib/voice-device-settings";
import type { ProfileRecord } from "../../../../domain/chat";
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
  selectedAvatarFilename: string;
  isSavingProfile: boolean;
  isUploadingProfileAvatar: boolean;
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
  onSaveProfile: () => Promise<void> | void;
  onUploadProfileAvatar: () => Promise<void> | void;
}

export function SettingsPanel(props: SettingsPanelProps) {
  const settingsNavButtonClass =
    "w-full cursor-pointer rounded-[0.62rem] border border-line bg-bg-2 px-[0.6rem] py-[0.52rem] text-left text-ink-1 transition-colors duration-[140ms] ease-out hover:bg-bg-3 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/60";

  return (
    <section
      class="grid min-h-[18rem] grid-cols-[minmax(10rem,12rem)_minmax(0,1fr)] gap-[0.88rem] max-[900px]:min-h-0 max-[900px]:grid-cols-1 max-[900px]:gap-[0.72rem]"
      aria-label="settings"
    >
      <aside
        class="grid content-start gap-[0.6rem] border-r border-line pr-[0.88rem] max-[900px]:border-r-0 max-[900px]:border-b max-[900px]:pr-0 max-[900px]:pb-[0.72rem]"
        aria-label="Settings category rail"
      >
        <p class="group-label">CATEGORIES</p>
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
                      "border-brand/85": isActive(),
                      "bg-brand/16": isActive(),
                    }}
                    onClick={() => props.onOpenSettingsCategory(category.id)}
                    aria-label={`Open ${category.label} settings category`}
                    aria-current={isActive() ? "page" : undefined}
                  >
                    <span class="text-[0.84rem] font-[700]">{category.label}</span>
                    <span class="muted text-[0.74rem]">{category.summary}</span>
                  </button>
                </li>
              );
            }}
          </For>
        </ul>
      </aside>
      <section class="grid content-start gap-[0.6rem]" aria-label="Settings content pane">
        <Switch>
          <Match when={props.activeSettingsCategory === "voice"}>
            <section
              class="grid grid-cols-[minmax(10rem,12rem)_minmax(0,1fr)] gap-[0.88rem] max-[900px]:grid-cols-1 max-[900px]:gap-[0.72rem]"
              aria-label="Voice settings submenu"
            >
              <aside
                class="grid content-start gap-[0.6rem] border-r border-line pr-[0.88rem] max-[900px]:border-r-0 max-[900px]:border-b max-[900px]:pr-0 max-[900px]:pb-[0.72rem]"
                aria-label="Voice settings submenu rail"
              >
                <p class="group-label">VOICE</p>
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
                              "border-brand/85": isActive(),
                              "bg-brand/16": isActive(),
                            }}
                            onClick={() => props.onOpenVoiceSettingsSubmenu(submenu.id)}
                            aria-label={`Open Voice ${submenu.label} submenu`}
                            aria-current={isActive() ? "page" : undefined}
                          >
                            <span class="text-[0.84rem] font-[700]">{submenu.label}</span>
                            <span class="muted text-[0.74rem]">{submenu.summary}</span>
                          </button>
                        </li>
                      );
                    }}
                  </For>
                </ul>
              </aside>
              <section class="grid content-start gap-[0.6rem]" aria-label="Voice settings submenu content">
                <Switch>
                  <Match when={props.activeVoiceSettingsSubmenu === "audio-devices"}>
                    <p class="group-label">AUDIO DEVICES</p>
                    <form class="inline-form" onSubmit={(event) => event.preventDefault()}>
                      <label>
                        Microphone
                        <select
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
                      <label>
                        Speaker
                        <select
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
                      <button
                        type="button"
                        onClick={() => void props.onRefreshAudioDeviceInventory()}
                        disabled={props.isRefreshingAudioDevices}
                      >
                        {props.isRefreshingAudioDevices ? "Refreshing..." : "Refresh devices"}
                      </button>
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
            <p class="group-label">PROFILE</p>
            <form
              class="inline-form"
              onSubmit={(event) => {
                event.preventDefault();
                void props.onSaveProfile();
              }}
            >
              <label>
                Username
                <input
                  aria-label="Profile username"
                  value={props.profileDraftUsername}
                  maxlength="32"
                  onInput={(event) => props.onProfileUsernameInput(event.currentTarget.value)}
                />
              </label>
              <label>
                About (Markdown)
                <textarea
                  aria-label="Profile about markdown"
                  value={props.profileDraftAbout}
                  maxlength="2048"
                  onInput={(event) => props.onProfileAboutInput(event.currentTarget.value)}
                  rows="6"
                />
              </label>
              <div class="flex gap-2">
                <button type="submit" disabled={props.isSavingProfile}>
                  {props.isSavingProfile ? "Saving..." : "Save profile"}
                </button>
              </div>
            </form>
            <div class="inline-form">
              <label>
                Avatar image
                <input
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
                <section class="grid gap-[0.55rem] rounded-[0.72rem] border border-line bg-bg-1 p-[0.7rem]">
                  <p class="group-label">PROFILE PREVIEW</p>
                  <div class="flex items-center gap-[0.6rem]">
                    <span
                      class="relative inline-flex h-[2.4rem] w-[2.4rem] items-center justify-center overflow-hidden rounded-full border border-line-soft bg-gradient-to-br from-bg-4 to-bg-3 text-[0.78rem] font-[780] text-ink-0"
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
                      <p class="m-0 font-[760] text-ink-0">{profile().username}</p>
                      <p class="mono">{profile().userId}</p>
                    </div>
                  </div>
                  <SafeMarkdown
                    class="leading-[1.4] text-ink-1 [&_ol]:m-[0.4rem_0_0.4rem_1.15rem] [&_ol]:p-0 [&_p+p]:mt-[0.45rem] [&_p]:m-0 [&_ul]:m-[0.4rem_0_0.4rem_1.15rem] [&_ul]:p-0"
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
