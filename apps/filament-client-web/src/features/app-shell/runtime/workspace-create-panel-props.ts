import type { WorkspaceCreatePanelBuilderOptions } from "../adapters/panel-host-props";

export interface WorkspaceCreatePanelPropsOptions {
  createGuildName: string;
  createGuildVisibility: WorkspaceCreatePanelBuilderOptions["createGuildVisibility"];
  createChannelName: string;
  createChannelKind: WorkspaceCreatePanelBuilderOptions["createChannelKind"];
  isCreatingWorkspace: boolean;
  canDismissWorkspaceCreateForm: boolean;
  workspaceError: string;
  onCreateWorkspaceSubmit:
    WorkspaceCreatePanelBuilderOptions["onCreateWorkspaceSubmit"];
  setCreateGuildName: WorkspaceCreatePanelBuilderOptions["setCreateGuildName"];
  setCreateGuildVisibility:
    WorkspaceCreatePanelBuilderOptions["setCreateGuildVisibility"];
  setCreateChannelName: WorkspaceCreatePanelBuilderOptions["setCreateChannelName"];
  setCreateChannelKind: WorkspaceCreatePanelBuilderOptions["setCreateChannelKind"];
  onCancelWorkspaceCreate:
    WorkspaceCreatePanelBuilderOptions["onCancelWorkspaceCreate"];
}

export function createWorkspaceCreatePanelProps(
  options: WorkspaceCreatePanelPropsOptions,
): WorkspaceCreatePanelBuilderOptions {
  return {
    createGuildName: options.createGuildName,
    createGuildVisibility: options.createGuildVisibility,
    createChannelName: options.createChannelName,
    createChannelKind: options.createChannelKind,
    isCreatingWorkspace: options.isCreatingWorkspace,
    canDismissWorkspaceCreateForm: options.canDismissWorkspaceCreateForm,
    workspaceError: options.workspaceError,
    onCreateWorkspaceSubmit: options.onCreateWorkspaceSubmit,
    setCreateGuildName: options.setCreateGuildName,
    setCreateGuildVisibility: options.setCreateGuildVisibility,
    setCreateChannelName: options.setCreateChannelName,
    setCreateChannelKind: options.setCreateChannelKind,
    onCancelWorkspaceCreate: options.onCancelWorkspaceCreate,
  };
}
