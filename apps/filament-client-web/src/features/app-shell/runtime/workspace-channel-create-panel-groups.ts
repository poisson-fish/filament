import type { BuildPanelHostPropGroupsOptions } from "../adapters/panel-host-props";
import {
  createChannelCreatePanelProps,
  type ChannelCreatePanelPropsOptions,
} from "./channel-create-panel-props";
import {
  createWorkspaceCreatePanelProps,
  type WorkspaceCreatePanelPropsOptions,
} from "./workspace-create-panel-props";

export interface WorkspaceChannelCreatePanelGroupsOptions {
  workspaceCreate: WorkspaceCreatePanelPropsOptions;
  channelCreate: ChannelCreatePanelPropsOptions;
}

export function createWorkspaceChannelCreatePanelGroups(
  options: WorkspaceChannelCreatePanelGroupsOptions,
): Pick<BuildPanelHostPropGroupsOptions, "workspaceCreate" | "channelCreate"> {
  return {
    workspaceCreate: {
      ...createWorkspaceCreatePanelProps(options.workspaceCreate),
    },
    channelCreate: {
      ...createChannelCreatePanelProps(options.channelCreate),
    },
  };
}
