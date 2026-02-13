import type {
  ModerationPanelBuilderOptions,
} from "../adapters/panel-host-props";

export interface ModerationPanelPropsOptions {
  moderationUserIdInput: string;
  moderationRoleInput: ModerationPanelBuilderOptions["moderationRoleInput"];
  overrideRoleInput: ModerationPanelBuilderOptions["overrideRoleInput"];
  overrideAllowCsv: string;
  overrideDenyCsv: string;
  isModerating: boolean;
  hasActiveWorkspace: boolean;
  hasActiveChannel: boolean;
  canManageRoles: boolean;
  canBanMembers: boolean;
  canManageChannelOverrides: boolean;
  moderationStatus: string;
  moderationError: string;
  setModerationUserIdInput: (value: string) => void;
  setModerationRoleInput: (
    value: ModerationPanelBuilderOptions["moderationRoleInput"],
  ) => void;
  onRunMemberAction: ModerationPanelBuilderOptions["onRunMemberAction"];
  setOverrideRoleInput: (
    value: ModerationPanelBuilderOptions["overrideRoleInput"],
  ) => void;
  setOverrideAllowCsv: (value: string) => void;
  setOverrideDenyCsv: (value: string) => void;
  onApplyOverride: ModerationPanelBuilderOptions["onApplyOverride"];
  onOpenRoleManagementPanel: () => void;
}

export function createModerationPanelProps(
  options: ModerationPanelPropsOptions,
): ModerationPanelBuilderOptions {
  return {
    moderationUserIdInput: options.moderationUserIdInput,
    moderationRoleInput: options.moderationRoleInput,
    overrideRoleInput: options.overrideRoleInput,
    overrideAllowCsv: options.overrideAllowCsv,
    overrideDenyCsv: options.overrideDenyCsv,
    isModerating: options.isModerating,
    hasActiveWorkspace: options.hasActiveWorkspace,
    hasActiveChannel: options.hasActiveChannel,
    canManageRoles: options.canManageRoles,
    canBanMembers: options.canBanMembers,
    canManageChannelOverrides: options.canManageChannelOverrides,
    moderationStatus: options.moderationStatus,
    moderationError: options.moderationError,
    setModerationUserIdInput: options.setModerationUserIdInput,
    setModerationRoleInput: options.setModerationRoleInput,
    onRunMemberAction: (action) => options.onRunMemberAction(action),
    setOverrideRoleInput: options.setOverrideRoleInput,
    setOverrideAllowCsv: options.setOverrideAllowCsv,
    setOverrideDenyCsv: options.setOverrideDenyCsv,
    onApplyOverride: (event) => options.onApplyOverride(event),
    onOpenRoleManagementPanel: options.onOpenRoleManagementPanel,
  };
}