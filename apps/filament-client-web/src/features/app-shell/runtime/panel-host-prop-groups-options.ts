import type { PanelHostPropGroupsOptions } from "./panel-host-prop-groups";
import {
  createCollaborationPanelPropGroupsOptions,
  type CollaborationPanelPropGroupsStateOptions,
} from "./collaboration-panel-prop-groups-options";
import {
  createSupportPanelPropGroupsOptions,
  type SupportPanelPropGroupsStateOptions,
} from "./support-panel-prop-groups-options";
import {
  createWorkspaceChannelCreatePanelGroupsOptions,
  type WorkspaceChannelCreatePanelGroupsStateOptions,
} from "./workspace-channel-create-panel-groups-options";

export interface PanelHostPropGroupsStateOptions {
  workspaceChannelCreate: WorkspaceChannelCreatePanelGroupsStateOptions;
  support: SupportPanelPropGroupsStateOptions;
  collaboration: CollaborationPanelPropGroupsStateOptions;
}

export function createPanelHostPropGroupsOptions(
  options: PanelHostPropGroupsStateOptions,
): PanelHostPropGroupsOptions {
  return {
    workspaceChannelCreate: createWorkspaceChannelCreatePanelGroupsOptions(
      options.workspaceChannelCreate,
    ),
    support: createSupportPanelPropGroupsOptions(options.support),
    collaboration: createCollaborationPanelPropGroupsOptions(
      options.collaboration,
    ),
  };
}