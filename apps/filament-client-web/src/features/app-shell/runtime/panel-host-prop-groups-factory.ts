import { createCollaborationPanelHostStateOptions } from "./collaboration-panel-host-state-options";
import { createPanelHostPropGroupsOptions } from "./panel-host-prop-groups-options";
import { createPanelHostPropGroups } from "./panel-host-prop-groups";
import { createSupportPanelHostStateOptions } from "./support-panel-host-state-options";
import { createWorkspaceChannelCreatePanelHostStateOptions } from "./workspace-channel-create-panel-host-state-options";
import type { CollaborationPanelHostStateOptions } from "./collaboration-panel-host-state-options";
import type { SupportPanelHostStateOptions } from "./support-panel-host-state-options";
import type { WorkspaceChannelCreatePanelHostStateOptions } from "./workspace-channel-create-panel-host-state-options";

export interface PanelHostPropGroupsFactoryOptions {
  workspaceChannelCreate: WorkspaceChannelCreatePanelHostStateOptions;
  support: SupportPanelHostStateOptions;
  collaboration: CollaborationPanelHostStateOptions;
}

export function createPanelHostPropGroupsFactory(
  options: PanelHostPropGroupsFactoryOptions,
): () => ReturnType<typeof createPanelHostPropGroups> {
  return () =>
    createPanelHostPropGroups(
      createPanelHostPropGroupsOptions({
        workspaceChannelCreate:
          createWorkspaceChannelCreatePanelHostStateOptions(
            options.workspaceChannelCreate,
          ),
        support: createSupportPanelHostStateOptions(options.support),
        collaboration: createCollaborationPanelHostStateOptions(
          options.collaboration,
        ),
      }),
    );
}