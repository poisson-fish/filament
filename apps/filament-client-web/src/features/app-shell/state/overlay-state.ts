import { createSignal } from "solid-js";
import {
  DEFAULT_SETTINGS_CATEGORY,
  DEFAULT_VOICE_SETTINGS_SUBMENU,
} from "../config/settings-menu";
import type {
  OverlayPanel,
  SettingsCategory,
  VoiceSettingsSubmenu,
} from "../types";

export function createOverlayState() {
  const [activeOverlayPanel, setActiveOverlayPanel] = createSignal<OverlayPanel | null>(null);
  const [activeSettingsCategory, setActiveSettingsCategory] =
    createSignal<SettingsCategory>(DEFAULT_SETTINGS_CATEGORY);
  const [activeVoiceSettingsSubmenu, setActiveVoiceSettingsSubmenu] =
    createSignal<VoiceSettingsSubmenu>(DEFAULT_VOICE_SETTINGS_SUBMENU);
  const [isChannelRailCollapsed, setChannelRailCollapsed] = createSignal(false);
  const [isMemberRailCollapsed, setMemberRailCollapsed] = createSignal(false);

  return {
    activeOverlayPanel,
    setActiveOverlayPanel,
    activeSettingsCategory,
    setActiveSettingsCategory,
    activeVoiceSettingsSubmenu,
    setActiveVoiceSettingsSubmenu,
    isChannelRailCollapsed,
    setChannelRailCollapsed,
    isMemberRailCollapsed,
    setMemberRailCollapsed,
  };
}
