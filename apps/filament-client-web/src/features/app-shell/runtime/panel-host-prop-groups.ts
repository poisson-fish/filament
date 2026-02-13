import {
  buildPanelHostPropGroups,
  type PanelHostPropGroups,
} from "../adapters/panel-host-props";
import {
  createCollaborationPanelPropGroups,
  type CollaborationPanelPropGroupsOptions,
} from "./collaboration-panel-prop-groups";
import {
  createSupportPanelPropGroups,
  type SupportPanelPropGroupsOptions,
} from "./support-panel-prop-groups";
import {
  createWorkspaceChannelCreatePanelGroups,
  type WorkspaceChannelCreatePanelGroupsOptions,
} from "./workspace-channel-create-panel-groups";

export interface PanelHostPropGroupsOptions {
  workspaceChannelCreate: WorkspaceChannelCreatePanelGroupsOptions;
  support: SupportPanelPropGroupsOptions;
  collaboration: CollaborationPanelPropGroupsOptions;
}

export function createPanelHostPropGroups(
  options: PanelHostPropGroupsOptions,
): PanelHostPropGroups {
  return buildPanelHostPropGroups({
    ...createWorkspaceChannelCreatePanelGroups(options.workspaceChannelCreate),
    ...createSupportPanelPropGroups(options.support),
    ...createCollaborationPanelPropGroups(options.collaboration),
  });
}