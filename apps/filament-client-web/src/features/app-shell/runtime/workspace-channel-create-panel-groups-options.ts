import type { ChannelKindName, GuildVisibility } from "../../../domain/chat";
import type { WorkspaceChannelCreatePanelGroupsOptions } from "./workspace-channel-create-panel-groups";

export interface WorkspaceChannelCreatePanelGroupsStateOptions {
  createGuildName: () => string;
  createGuildVisibility: () => GuildVisibility;
  createChannelName: () => string;
  createChannelKind: () => ChannelKindName;
  isCreatingWorkspace: () => boolean;
  canDismissWorkspaceCreateForm: () => boolean;
  workspaceError: () => string;
  onCreateWorkspaceSubmit:
    WorkspaceChannelCreatePanelGroupsOptions["workspaceCreate"]["onCreateWorkspaceSubmit"];
  setCreateGuildName:
    WorkspaceChannelCreatePanelGroupsOptions["workspaceCreate"]["setCreateGuildName"];
  setCreateGuildVisibility:
    WorkspaceChannelCreatePanelGroupsOptions["workspaceCreate"]["setCreateGuildVisibility"];
  setCreateChannelName:
    WorkspaceChannelCreatePanelGroupsOptions["workspaceCreate"]["setCreateChannelName"];
  setCreateChannelKind:
    WorkspaceChannelCreatePanelGroupsOptions["workspaceCreate"]["setCreateChannelKind"];
  newChannelName: () => string;
  newChannelKind: () => ChannelKindName;
  isCreatingChannel: () => boolean;
  channelCreateError: () => string;
  onCreateChannelSubmit:
    WorkspaceChannelCreatePanelGroupsOptions["channelCreate"]["onCreateChannelSubmit"];
  setNewChannelName:
    WorkspaceChannelCreatePanelGroupsOptions["channelCreate"]["setNewChannelName"];
  setNewChannelKind:
    WorkspaceChannelCreatePanelGroupsOptions["channelCreate"]["setNewChannelKind"];
  closeOverlayPanel: () => void;
}

export function createWorkspaceChannelCreatePanelGroupsOptions(
  options: WorkspaceChannelCreatePanelGroupsStateOptions,
): WorkspaceChannelCreatePanelGroupsOptions {
  return {
    workspaceCreate: {
      createGuildName: options.createGuildName(),
      createGuildVisibility: options.createGuildVisibility(),
      createChannelName: options.createChannelName(),
      createChannelKind: options.createChannelKind(),
      isCreatingWorkspace: options.isCreatingWorkspace(),
      canDismissWorkspaceCreateForm: options.canDismissWorkspaceCreateForm(),
      workspaceError: options.workspaceError(),
      onCreateWorkspaceSubmit: options.onCreateWorkspaceSubmit,
      setCreateGuildName: options.setCreateGuildName,
      setCreateGuildVisibility: options.setCreateGuildVisibility,
      setCreateChannelName: options.setCreateChannelName,
      setCreateChannelKind: options.setCreateChannelKind,
      onCancelWorkspaceCreate: options.closeOverlayPanel,
    },
    channelCreate: {
      newChannelName: options.newChannelName(),
      newChannelKind: options.newChannelKind(),
      isCreatingChannel: options.isCreatingChannel(),
      channelCreateError: options.channelCreateError(),
      onCreateChannelSubmit: options.onCreateChannelSubmit,
      setNewChannelName: options.setNewChannelName,
      setNewChannelKind: options.setNewChannelKind,
      onCancelChannelCreate: options.closeOverlayPanel,
    },
  };
}