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
  return (
    <section class="settings-panel-layout" aria-label="settings">
      <aside class="settings-panel-rail" aria-label="Settings category rail">
        <p class="group-label">CATEGORIES</p>
        <ul class="settings-category-list">
          <For each={props.settingsCategories}>
            {(category) => {
              const isActive = () => props.activeSettingsCategory === category.id;
              return (
                <li>
                  <button
                    type="button"
                    class="settings-category-button"
                    classList={{ "settings-category-button-active": isActive() }}
                    onClick={() => props.onOpenSettingsCategory(category.id)}
                    aria-label={`Open ${category.label} settings category`}
                    aria-current={isActive() ? "page" : undefined}
                  >
                    <span class="settings-category-name">{category.label}</span>
                    <span class="settings-category-summary muted">{category.summary}</span>
                  </button>
                </li>
              );
            }}
          </For>
        </ul>
      </aside>
      <section class="settings-panel-content" aria-label="Settings content pane">
        <Switch>
          <Match when={props.activeSettingsCategory === "voice"}>
            <section class="settings-submenu-layout" aria-label="Voice settings submenu">
              <aside class="settings-submenu-rail" aria-label="Voice settings submenu rail">
                <p class="group-label">VOICE</p>
                <ul class="settings-submenu-list">
                  <For each={props.voiceSettingsSubmenu}>
                    {(submenu) => {
                      const isActive = () => props.activeVoiceSettingsSubmenu === submenu.id;
                      return (
                        <li>
                          <button
                            type="button"
                            class="settings-submenu-button"
                            classList={{
                              "settings-submenu-button-active": isActive(),
                            }}
                            onClick={() => props.onOpenVoiceSettingsSubmenu(submenu.id)}
                            aria-label={`Open Voice ${submenu.label} submenu`}
                            aria-current={isActive() ? "page" : undefined}
                          >
                            <span class="settings-category-name">{submenu.label}</span>
                            <span class="settings-category-summary muted">{submenu.summary}</span>
                          </button>
                        </li>
                      );
                    }}
                  </For>
                </ul>
              </aside>
              <section
                class="settings-submenu-content"
                aria-label="Voice settings submenu content"
              >
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
                        No audio devices were detected yet. Refresh after granting
                        media permissions.
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
              <div class="settings-profile-actions">
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
              <div class="settings-profile-actions">
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
                <section class="settings-profile-preview">
                  <p class="group-label">PROFILE PREVIEW</p>
                  <div class="settings-profile-preview-head">
                    <span class="settings-avatar-shell" aria-hidden="true">
                      <span class="settings-avatar-fallback">{profile().username.slice(0, 1).toUpperCase()}</span>
                      <Show when={props.profileAvatarUrl}>
                        <img
                          class="settings-avatar-image"
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
                      <p class="settings-profile-name">{profile().username}</p>
                      <p class="mono">{profile().userId}</p>
                    </div>
                  </div>
                  <SafeMarkdown class="settings-profile-markdown" tokens={profile().aboutMarkdownTokens} />
                </section>
              )}
            </Show>
          </Match>
        </Switch>
      </section>
    </section>
  );
}
