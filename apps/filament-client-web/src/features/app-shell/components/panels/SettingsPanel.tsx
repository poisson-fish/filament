import { For, Match, Show, Switch } from "solid-js";
import type { SettingsCategory, SettingsCategoryItem, VoiceSettingsSubmenu, VoiceSettingsSubmenuItem } from "../../types";
import type {
  AudioDeviceOption,
  VoiceDevicePreferences,
} from "../../../../lib/voice-device-settings";

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
  onOpenSettingsCategory: (category: SettingsCategory) => void;
  onOpenVoiceSettingsSubmenu: (submenu: VoiceSettingsSubmenu) => void;
  onSetVoiceDevicePreference: (
    kind: "audioinput" | "audiooutput",
    nextValue: string,
  ) => Promise<void> | void;
  onRefreshAudioDeviceInventory: () => Promise<void> | void;
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
            <p class="muted">
              Profile settings remain a non-functional placeholder for a future
              plan phase.
            </p>
          </Match>
        </Switch>
      </section>
    </section>
  );
}
