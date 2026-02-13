import { createEffect, untrack, type Accessor } from "solid-js";
import { saveWorkspaceCache } from "../../../lib/workspace-cache";
import type { WorkspaceRecord } from "../../../domain/chat";
import type {
  OverlayPanel,
  SettingsCategory,
  VoiceSettingsSubmenu,
} from "../types";

export interface RuntimeEffectsOptions {
  workspaceBootstrapDone: Accessor<boolean>;
  workspaces: Accessor<WorkspaceRecord[]>;
  setActiveOverlayPanel: (value: OverlayPanel | null) => OverlayPanel | null;
  activeOverlayPanel: Accessor<OverlayPanel | null>;
  activeSettingsCategory: Accessor<SettingsCategory>;
  activeVoiceSettingsSubmenu: Accessor<VoiceSettingsSubmenu>;
  refreshAudioDeviceInventory: (requestPermissionPrompt?: boolean) => Promise<void>;
}

export function createRuntimeEffects(options: RuntimeEffectsOptions): void {
  createEffect(() => {
    if (!options.workspaceBootstrapDone()) {
      return;
    }
    saveWorkspaceCache(options.workspaces());
  });

  createEffect(() => {
    if (!options.workspaceBootstrapDone()) {
      return;
    }
    if (options.workspaces().length === 0) {
      options.setActiveOverlayPanel("workspace-create");
    }
  });

  createEffect(() => {
    const isVoiceAudioSettingsOpen =
      options.activeOverlayPanel() === "client-settings" &&
      options.activeSettingsCategory() === "voice" &&
      options.activeVoiceSettingsSubmenu() === "audio-devices";
    if (!isVoiceAudioSettingsOpen) {
      return;
    }
    void untrack(() => options.refreshAudioDeviceInventory(false));
  });
}