import {
  guildIdFromInput,
  userIdFromInput,
  type GuildId,
} from "../domain/chat";
import type {
  WorkspaceMemberBanPayload,
} from "./gateway-contracts";
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
  | {
      type: "workspace_member_ban";
      payload: WorkspaceMemberBanPayload;
    };

type WorkspaceMemberGatewayEventType = WorkspaceMemberGatewayEvent["type"];
type WorkspaceMemberEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

function parseWorkspaceMemberBanPayload(payload: unknown): WorkspaceMemberBanPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.guild_id !== "string" ||
    typeof value.user_id !== "string" ||
    typeof value.banned_at_unix !== "number" ||
    !Number.isSafeInteger(value.banned_at_unix) ||
    value.banned_at_unix < 1
  ) {
    return null;
  }

  let guildId: GuildId;
  let userId: string;
  try {
    guildId = guildIdFromInput(value.guild_id);
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }

  return {
    guildId,
    userId,
    bannedAtUnix: value.banned_at_unix,
  };
}

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
  workspace_member_ban: parseWorkspaceMemberBanPayload,
};

export function isWorkspaceMemberGatewayEventType(
  value: string,
): value is WorkspaceMemberGatewayEventType {
  return (
    isWorkspaceMemberAddGatewayEventType(value) ||
    isWorkspaceMemberUpdateGatewayEventType(value) ||
    isWorkspaceMemberRemoveGatewayEventType(value) ||
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