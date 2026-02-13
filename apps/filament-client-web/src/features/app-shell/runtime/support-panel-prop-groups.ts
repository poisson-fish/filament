import type { BuildPanelHostPropGroupsOptions } from "../adapters/panel-host-props";
import {
  createClientSettingsPanelProps,
  type ClientSettingsPanelPropsOptions,
} from "./client-settings-panel-props";
import {
  createPublicDirectoryPanelProps,
  type PublicDirectoryPanelPropsOptions,
} from "./public-directory-panel-props";
import {
  createRoleManagementPanelProps,
  type RoleManagementPanelPropsOptions,
} from "./role-management-panel-props";
import {
  createUtilityPanelProps,
  type UtilityPanelPropsOptions,
} from "./utility-panel-props";
import {
  createWorkspaceSettingsPanelProps,
  type WorkspaceSettingsPanelPropsOptions,
} from "./workspace-settings-panel-props";

export interface SupportPanelPropGroupsOptions {
  publicDirectory: PublicDirectoryPanelPropsOptions;
  settings: ClientSettingsPanelPropsOptions;
  workspaceSettings: WorkspaceSettingsPanelPropsOptions;
  roleManagement: RoleManagementPanelPropsOptions;
  utility: UtilityPanelPropsOptions;
}

export function createSupportPanelPropGroups(
  options: SupportPanelPropGroupsOptions,
): Pick<
  BuildPanelHostPropGroupsOptions,
  | "publicDirectory"
  | "settings"
  | "workspaceSettings"
  | "roleManagement"
  | "utility"
> {
  return {
    publicDirectory: createPublicDirectoryPanelProps(options.publicDirectory),
    settings: createClientSettingsPanelProps(options.settings),
    workspaceSettings: {
      ...createWorkspaceSettingsPanelProps(options.workspaceSettings),
    },
    roleManagement: {
      ...createRoleManagementPanelProps(options.roleManagement),
    },
    utility: createUtilityPanelProps(options.utility),
  };
}