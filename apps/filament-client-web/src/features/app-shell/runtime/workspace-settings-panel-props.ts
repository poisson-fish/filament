import type { GuildVisibility } from "../../../domain/chat";
import type { WorkspaceSettingsPanelBuilderOptions } from "../adapters/panel-host-props";

export interface WorkspaceSettingsPanelPropsOptions {
  hasActiveWorkspace: boolean;
  canManageWorkspaceSettings: boolean;
  workspaceName: string;
  workspaceVisibility: GuildVisibility;
  isSavingWorkspaceSettings: boolean;
  workspaceSettingsStatus: string;
  workspaceSettingsError: string;
  setWorkspaceSettingsName: (value: string) => void;
  setWorkspaceSettingsVisibility: (value: GuildVisibility) => void;
  setWorkspaceSettingsStatus: (value: string) => void;
  setWorkspaceSettingsError: (value: string) => void;
  onSaveWorkspaceSettings: () => Promise<void> | void;
}

export function createWorkspaceSettingsPanelProps(
  options: WorkspaceSettingsPanelPropsOptions,
): WorkspaceSettingsPanelBuilderOptions {
  return {
    hasActiveWorkspace: options.hasActiveWorkspace,
    canManageWorkspaceSettings: options.canManageWorkspaceSettings,
    workspaceName: options.workspaceName,
    workspaceVisibility: options.workspaceVisibility,
    isSavingWorkspaceSettings: options.isSavingWorkspaceSettings,
    workspaceSettingsStatus: options.workspaceSettingsStatus,
    workspaceSettingsError: options.workspaceSettingsError,
    setWorkspaceSettingsName: (value) => {
      options.setWorkspaceSettingsName(value);
      options.setWorkspaceSettingsStatus("");
      options.setWorkspaceSettingsError("");
    },
    setWorkspaceSettingsVisibility: (value) => {
      options.setWorkspaceSettingsVisibility(value);
      options.setWorkspaceSettingsStatus("");
      options.setWorkspaceSettingsError("");
    },
    onSaveWorkspaceSettings: options.onSaveWorkspaceSettings,
  };
}