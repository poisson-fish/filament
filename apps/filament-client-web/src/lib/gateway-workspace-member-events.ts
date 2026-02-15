import {
  decodeWorkspaceMemberBanGatewayEvent,
  isWorkspaceMemberBanGatewayEventType,
  type WorkspaceMemberBanGatewayEvent,
} from "./gateway-workspace-member-ban-events";
import {
  decodeWorkspaceMemberAddGatewayEvent,
  isWorkspaceMemberAddGatewayEventType,
  type WorkspaceMemberAddGatewayEvent,
} from "./gateway-workspace-member-add-events";
import {
  decodeWorkspaceMemberUpdateGatewayEvent,
  isWorkspaceMemberUpdateGatewayEventType,
  type WorkspaceMemberUpdateGatewayEvent,
} from "./gateway-workspace-member-update-events";
import {
  decodeWorkspaceMemberRemoveGatewayEvent,
  isWorkspaceMemberRemoveGatewayEventType,
  type WorkspaceMemberRemoveGatewayEvent,
} from "./gateway-workspace-member-remove-events";

export type WorkspaceMemberGatewayEvent =
  | WorkspaceMemberAddGatewayEvent
  | WorkspaceMemberUpdateGatewayEvent
  | WorkspaceMemberRemoveGatewayEvent
  | WorkspaceMemberBanGatewayEvent;

type WorkspaceMemberGatewayEventType = WorkspaceMemberGatewayEvent["type"];
type WorkspaceMemberEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

const WORKSPACE_MEMBER_EVENT_DECODERS: {
  [K in WorkspaceMemberGatewayEventType]: WorkspaceMemberEventDecoder<
    Extract<WorkspaceMemberGatewayEvent, { type: K }>["payload"]
  >;
} = {
  workspace_member_add: (payload) =>
    decodeWorkspaceMemberAddGatewayEvent("workspace_member_add", payload)?.payload ?? null,
  workspace_member_update: (payload) =>
    decodeWorkspaceMemberUpdateGatewayEvent("workspace_member_update", payload)?.payload ?? null,
  workspace_member_remove: (payload) =>
    decodeWorkspaceMemberRemoveGatewayEvent("workspace_member_remove", payload)?.payload ?? null,
  workspace_member_ban: (payload) =>
    decodeWorkspaceMemberBanGatewayEvent("workspace_member_ban", payload)?.payload ?? null,
};

export function isWorkspaceMemberGatewayEventType(
  value: string,
): value is WorkspaceMemberGatewayEventType {
  return (
    isWorkspaceMemberAddGatewayEventType(value) ||
    isWorkspaceMemberUpdateGatewayEventType(value) ||
    isWorkspaceMemberRemoveGatewayEventType(value) ||
    isWorkspaceMemberBanGatewayEventType(value) ||
    value in WORKSPACE_MEMBER_EVENT_DECODERS
  );
}

function decodeKnownWorkspaceMemberGatewayEvent<K extends WorkspaceMemberGatewayEventType>(
  type: K,
  payload: unknown,
): Extract<WorkspaceMemberGatewayEvent, { type: K }> | null {
  const parsedPayload = WORKSPACE_MEMBER_EVENT_DECODERS[type](payload);
  if (!parsedPayload) {
    return null;
  }

  return {
    type,
    payload: parsedPayload,
  } as Extract<WorkspaceMemberGatewayEvent, { type: K }>;
}

export function decodeWorkspaceMemberGatewayEvent(
  type: string,
  payload: unknown,
): WorkspaceMemberGatewayEvent | null {
  if (!isWorkspaceMemberGatewayEventType(type)) {
    return null;
  }

  return decodeKnownWorkspaceMemberGatewayEvent(type, payload);
}