import type {
  GuildVisibility,
  WorkspaceRecord,
} from "../../../domain/chat";
import { openOverlayPanelWithDefaults } from "../controllers/overlay-controller";
import type {
  OverlayPanel,
  SettingsCategory,
  VoiceSettingsSubmenu,
} from "../types";

export interface OverlayPanelActionsOptions {
  activeWorkspace: () => WorkspaceRecord | undefined;
  canCloseActivePanel: () => boolean;
  setWorkspaceSettingsName: (value: string) => string;
  setWorkspaceSettingsVisibility: (value: GuildVisibility) => GuildVisibility;
  setWorkspaceSettingsStatus: (value: string) => string;
  setWorkspaceSettingsError: (value: string) => string;
  setActiveOverlayPanel: (value: OverlayPanel | null) => OverlayPanel | null;
  setWorkspaceError: (value: string) => string;
  setChannelCreateError: (value: string) => string;
  setActiveSettingsCategory: (value: SettingsCategory) => SettingsCategory;
  setActiveVoiceSettingsSubmenu: (value: VoiceSettingsSubmenu) => VoiceSettingsSubmenu;
}

export function createOverlayPanelActions(options: OverlayPanelActionsOptions) {
  const openOverlayPanel = (panel: OverlayPanel): void => {
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

  const openWorkspaceSettingsPanel = (): void => {
    const activeWorkspace = options.activeWorkspace();
    if (activeWorkspace) {
      options.setWorkspaceSettingsName(activeWorkspace.guildName);
      options.setWorkspaceSettingsVisibility(activeWorkspace.visibility);
    }
    options.setWorkspaceSettingsStatus("");
    options.setWorkspaceSettingsError("");
    openOverlayPanel("workspace-settings");
  };

  return {
    openOverlayPanel,
    closeOverlayPanel,
    openSettingsCategory,
    openClientSettingsPanel,
    openWorkspaceSettingsPanel,
  };
}