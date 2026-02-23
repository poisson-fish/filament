import type { Setter } from "solid-js";
import type {
  GuildVisibility,
  WorkspaceRecord,
} from "../../../domain/chat";
import { openOverlayPanelWithDefaults } from "../controllers/overlay-controller";
import type {
  OverlayPanel,
  SettingsCategory,
  VoiceSettingsSubmenu,
  WorkspaceSettingsSection,
} from "../types";

export interface OverlayPanelActionsOptions {
  activeWorkspace: () => WorkspaceRecord | null | undefined;
  canCloseActivePanel: () => boolean;
  setWorkspaceSettingsName: Setter<string>;
  setWorkspaceSettingsVisibility: Setter<GuildVisibility>;
  setWorkspaceSettingsStatus: Setter<string>;
  setWorkspaceSettingsError: Setter<string>;
  setActiveOverlayPanel: Setter<OverlayPanel | null>;
  setWorkspaceError: Setter<string>;
  setChannelCreateError: Setter<string>;
  setActiveSettingsCategory: Setter<SettingsCategory>;
  setActiveVoiceSettingsSubmenu: Setter<VoiceSettingsSubmenu>;
  setActiveWorkspaceSettingsSection: Setter<WorkspaceSettingsSection>;
}

export function createOverlayPanelActions(options: OverlayPanelActionsOptions) {
  const openOverlayPanel = (panel: OverlayPanel): void => {
    if (panel === "workspace-settings") {
      options.setActiveWorkspaceSettingsSection("profile");
    }
    openOverlayPanelWithDefaults(panel, {
      setPanel: options.setActiveOverlayPanel,
      setWorkspaceError: options.setWorkspaceError,
      setChannelCreateError: options.setChannelCreateError,
      setActiveSettingsCategory: options.setActiveSettingsCategory,
      setActiveVoiceSettingsSubmenu: options.setActiveVoiceSettingsSubmenu,
    });
  };

  const closeOverlayPanel = (): void => {
    if (!options.canCloseActivePanel()) {
      return;
    }
    options.setActiveOverlayPanel(null);
  };

  const openSettingsCategory = (category: SettingsCategory): void => {
    options.setActiveSettingsCategory(category);
    if (category === "voice") {
      options.setActiveVoiceSettingsSubmenu("audio-devices");
    }
  };

  const openClientSettingsPanel = (): void => {
    openOverlayPanel("client-settings");
  };

  const openWorkspaceSettingsPanel = (
    section: WorkspaceSettingsSection = "profile",
  ): void => {
    const activeWorkspace = options.activeWorkspace();
    if (activeWorkspace) {
      options.setWorkspaceSettingsName(activeWorkspace.guildName);
      options.setWorkspaceSettingsVisibility(activeWorkspace.visibility);
    }
    options.setActiveWorkspaceSettingsSection(section);
    options.setWorkspaceSettingsStatus("");
    options.setWorkspaceSettingsError("");
    openOverlayPanelWithDefaults("workspace-settings", {
      setPanel: options.setActiveOverlayPanel,
      setWorkspaceError: options.setWorkspaceError,
      setChannelCreateError: options.setChannelCreateError,
      setActiveSettingsCategory: options.setActiveSettingsCategory,
      setActiveVoiceSettingsSubmenu: options.setActiveVoiceSettingsSubmenu,
    });
  };

  return {
    openOverlayPanel,
    closeOverlayPanel,
    openSettingsCategory,
    openClientSettingsPanel,
    openWorkspaceSettingsPanel,
  };
}
