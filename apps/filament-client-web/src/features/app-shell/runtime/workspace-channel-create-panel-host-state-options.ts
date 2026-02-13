import type { CreateAppShellSelectorsResult } from "../selectors/create-app-shell-selectors";
import type { createWorkspaceState } from "../state/workspace-state";
import type { WorkspaceChannelOperationsController } from "./workspace-channel-operations-controller";
import type { WorkspaceChannelCreatePanelGroupsStateOptions } from "./workspace-channel-create-panel-groups-options";

export interface WorkspaceChannelCreatePanelHostStateOptions {
  workspaceChannelState: ReturnType<typeof createWorkspaceState>["workspaceChannel"];
  selectors: CreateAppShellSelectorsResult;
  workspaceChannelOperations: WorkspaceChannelOperationsController;
  closeOverlayPanel: () => void;
}

export function createWorkspaceChannelCreatePanelHostStateOptions(
  options: WorkspaceChannelCreatePanelHostStateOptions,
): WorkspaceChannelCreatePanelGroupsStateOptions {
  return {
    createGuildName: options.workspaceChannelState.createGuildName,
    createGuildVisibility: options.workspaceChannelState.createGuildVisibility,
    createChannelName: options.workspaceChannelState.createChannelName,
    createChannelKind: options.workspaceChannelState.createChannelKind,
    isCreatingWorkspace: options.workspaceChannelState.isCreatingWorkspace,
    canDismissWorkspaceCreateForm: options.selectors.canDismissWorkspaceCreateForm,
    workspaceError: options.workspaceChannelState.workspaceError,
    onCreateWorkspaceSubmit: options.workspaceChannelOperations.createWorkspace,
    setCreateGuildName: options.workspaceChannelState.setCreateGuildName,
    setCreateGuildVisibility: options.workspaceChannelState.setCreateGuildVisibility,
    setCreateChannelName: options.workspaceChannelState.setCreateChannelName,
    setCreateChannelKind: options.workspaceChannelState.setCreateChannelKind,
    newChannelName: options.workspaceChannelState.newChannelName,
    newChannelKind: options.workspaceChannelState.newChannelKind,
    isCreatingChannel: options.workspaceChannelState.isCreatingChannel,
    channelCreateError: options.workspaceChannelState.channelCreateError,
    onCreateChannelSubmit: options.workspaceChannelOperations.createNewChannel,
    setNewChannelName: options.workspaceChannelState.setNewChannelName,
    setNewChannelKind: options.workspaceChannelState.setNewChannelKind,
    closeOverlayPanel: options.closeOverlayPanel,
  };
}