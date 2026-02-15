import type {
  ChannelCreatePayload,
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
  onWorkspaceIpBanSync?: (payload: WorkspaceIpBanSyncPayload) => void;
}

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

  if (workspaceEvent.type === "channel_create") {
    handlers.onChannelCreate?.(workspaceEvent.payload);
    return true;
  }
  if (workspaceEvent.type === "workspace_update") {
    handlers.onWorkspaceUpdate?.(workspaceEvent.payload);
    return true;
  }
  if (workspaceEvent.type === "workspace_member_add") {
    handlers.onWorkspaceMemberAdd?.(workspaceEvent.payload);
    return true;
  }
  if (workspaceEvent.type === "workspace_member_update") {
    handlers.onWorkspaceMemberUpdate?.(workspaceEvent.payload);
    return true;
  }
  if (workspaceEvent.type === "workspace_member_remove") {
    handlers.onWorkspaceMemberRemove?.(workspaceEvent.payload);
    return true;
  }
  if (workspaceEvent.type === "workspace_member_ban") {
    handlers.onWorkspaceMemberBan?.(workspaceEvent.payload);
    return true;
  }
  if (workspaceEvent.type === "workspace_role_create") {
    handlers.onWorkspaceRoleCreate?.(workspaceEvent.payload);
    return true;
  }
  if (workspaceEvent.type === "workspace_role_update") {
    handlers.onWorkspaceRoleUpdate?.(workspaceEvent.payload);
    return true;
  }
  if (workspaceEvent.type === "workspace_role_delete") {
    handlers.onWorkspaceRoleDelete?.(workspaceEvent.payload);
    return true;
  }
  if (workspaceEvent.type === "workspace_role_reorder") {
    handlers.onWorkspaceRoleReorder?.(workspaceEvent.payload);
    return true;
  }
  if (workspaceEvent.type === "workspace_role_assignment_add") {
    handlers.onWorkspaceRoleAssignmentAdd?.(workspaceEvent.payload);
    return true;
  }
  if (workspaceEvent.type === "workspace_role_assignment_remove") {
    handlers.onWorkspaceRoleAssignmentRemove?.(workspaceEvent.payload);
    return true;
  }
  if (workspaceEvent.type === "workspace_channel_override_update") {
    handlers.onWorkspaceChannelOverrideUpdate?.(workspaceEvent.payload);
    return true;
  }

  handlers.onWorkspaceIpBanSync?.(workspaceEvent.payload);
  return true;
}