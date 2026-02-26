import type {
  ChannelCreatePayload,
  WorkspaceChannelPermissionOverrideUpdatePayload,
  WorkspaceChannelOverrideUpdatePayload,
  WorkspaceIpBanSyncPayload,
  WorkspaceMemberAddPayload,
  WorkspaceMemberBanPayload,
  WorkspaceMemberRemovePayload,
  WorkspaceMemberUpdatePayload,
  WorkspaceRoleAssignmentAddPayload,
  WorkspaceRoleAssignmentRemovePayload,
  WorkspaceRoleCreatePayload,
  WorkspaceRoleDeletePayload,
  WorkspaceRoleReorderPayload,
  WorkspaceRoleUpdatePayload,
  WorkspaceUpdatePayload,
} from "./gateway-contracts";
import {
  decodeWorkspaceGatewayEvent,
  isWorkspaceGatewayEventType,
} from "./gateway-workspace-events";
import {
  dispatchDecodedGatewayEvent,
  type GatewayDispatchTable,
} from "./gateway-dispatch-table";

export interface WorkspaceGatewayDispatchHandlers {
  onChannelCreate?: (payload: ChannelCreatePayload) => void;
  onWorkspaceUpdate?: (payload: WorkspaceUpdatePayload) => void;
  onWorkspaceMemberAdd?: (payload: WorkspaceMemberAddPayload) => void;
  onWorkspaceMemberUpdate?: (payload: WorkspaceMemberUpdatePayload) => void;
  onWorkspaceMemberRemove?: (payload: WorkspaceMemberRemovePayload) => void;
  onWorkspaceMemberBan?: (payload: WorkspaceMemberBanPayload) => void;
  onWorkspaceRoleCreate?: (payload: WorkspaceRoleCreatePayload) => void;
  onWorkspaceRoleUpdate?: (payload: WorkspaceRoleUpdatePayload) => void;
  onWorkspaceRoleDelete?: (payload: WorkspaceRoleDeletePayload) => void;
  onWorkspaceRoleReorder?: (payload: WorkspaceRoleReorderPayload) => void;
  onWorkspaceRoleAssignmentAdd?: (payload: WorkspaceRoleAssignmentAddPayload) => void;
  onWorkspaceRoleAssignmentRemove?: (
    payload: WorkspaceRoleAssignmentRemovePayload,
  ) => void;
  onWorkspaceChannelOverrideUpdate?: (
    payload: WorkspaceChannelOverrideUpdatePayload,
  ) => void;
  onWorkspaceChannelPermissionOverrideUpdate?: (
    payload: WorkspaceChannelPermissionOverrideUpdatePayload,
  ) => void;
  onWorkspaceIpBanSync?: (payload: WorkspaceIpBanSyncPayload) => void;
}

export const WORKSPACE_GATEWAY_DISPATCH_EVENT_TYPES: readonly string[] = [
  "channel_create",
  "workspace_update",
  "workspace_member_add",
  "workspace_member_update",
  "workspace_member_remove",
  "workspace_member_ban",
  "workspace_role_create",
  "workspace_role_update",
  "workspace_role_delete",
  "workspace_role_reorder",
  "workspace_role_assignment_add",
  "workspace_role_assignment_remove",
  "workspace_channel_role_override_update",
  "workspace_channel_permission_override_update",
  "workspace_ip_ban_sync",
];

type WorkspaceGatewayEvent = NonNullable<
  ReturnType<typeof decodeWorkspaceGatewayEvent>
>;

const WORKSPACE_DISPATCH_TABLE: GatewayDispatchTable<
  WorkspaceGatewayEvent,
  WorkspaceGatewayDispatchHandlers
> = {
  channel_create: (eventPayload, eventHandlers) => {
    eventHandlers.onChannelCreate?.(eventPayload);
  },
  workspace_update: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceUpdate?.(eventPayload);
  },
  workspace_member_add: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceMemberAdd?.(eventPayload);
  },
  workspace_member_update: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceMemberUpdate?.(eventPayload);
  },
  workspace_member_remove: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceMemberRemove?.(eventPayload);
  },
  workspace_member_ban: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceMemberBan?.(eventPayload);
  },
  workspace_role_create: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceRoleCreate?.(eventPayload);
  },
  workspace_role_update: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceRoleUpdate?.(eventPayload);
  },
  workspace_role_delete: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceRoleDelete?.(eventPayload);
  },
  workspace_role_reorder: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceRoleReorder?.(eventPayload);
  },
  workspace_role_assignment_add: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceRoleAssignmentAdd?.(eventPayload);
  },
  workspace_role_assignment_remove: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceRoleAssignmentRemove?.(eventPayload);
  },
  workspace_channel_role_override_update: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceChannelOverrideUpdate?.(eventPayload);
  },
  workspace_channel_permission_override_update: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceChannelPermissionOverrideUpdate?.(eventPayload);
  },
  workspace_ip_ban_sync: (eventPayload, eventHandlers) => {
    eventHandlers.onWorkspaceIpBanSync?.(eventPayload);
  },
};

export function dispatchWorkspaceGatewayEvent(
  type: string,
  payload: unknown,
  handlers: WorkspaceGatewayDispatchHandlers,
): boolean {
  if (!isWorkspaceGatewayEventType(type)) {
    return false;
  }

  const workspaceEvent = decodeWorkspaceGatewayEvent(type, payload);
  if (!workspaceEvent) {
    return true;
  }

  dispatchDecodedGatewayEvent(
    workspaceEvent,
    handlers,
    WORKSPACE_DISPATCH_TABLE,
  );
  return true;
}
