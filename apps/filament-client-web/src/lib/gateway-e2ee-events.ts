import {
  type DeviceId,
  type ConversationId,
  type GroupId,
  type MessageId,
  type ProposalId,
  type UserId,
  conversationIdFromInput,
  deviceIdFromInput,
  groupIdFromInput,
  messageIdFromInput,
  proposalIdFromInput,
  userIdFromInput,
} from "../domain/chat";

const MAX_DEVICE_COUNT = 100;
const MAX_KEYPACKAGE_COUNT = 100;

export interface DeviceListUpdatePayload {
  userId: UserId;
  deviceCount: number;
  createdAtUnix: number;
}

export interface KeyPackageLowPayload {
  deviceId: DeviceId;
  remainingCount: number;
  waterMark: number;
  createdAtUnix: number;
}

export interface MlsMessagePayload {
  groupId: GroupId;
  conversationId: ConversationId;
  messageId: MessageId;
  epoch: number;
  suiteId: number;
  senderDeviceId: DeviceId;
  createdAtUnix: number;
}

export interface MlsCommitPayload {
  groupId: GroupId;
  conversationId: ConversationId;
  epoch: number;
  priorEpoch: number;
  committerDeviceId: DeviceId;
  createdAtUnix: number;
}

export interface MlsWelcomePayload {
  groupId: GroupId;
  conversationId: ConversationId;
  epoch: number;
  suiteId: number;
  createdAtUnix: number;
}

export interface MlsProposalPayload {
  groupId: GroupId;
  conversationId: ConversationId;
  proposalId: ProposalId;
  epoch: number;
  proposerDeviceId: DeviceId | null;
  externalSenderIndex: 0 | null;
  reconciliationDeadlineUnix: number | null;
  createdAtUnix: number;
}

export interface MlsLeafRoutingPayload {
  leafIndex: number;
  userId: UserId;
  deviceId: DeviceId;
}

export type MlsMembershipChangePayload =
  | { kind: "add"; leaf: MlsLeafRoutingPayload }
  | { kind: "remove"; leaves: MlsLeafRoutingPayload[] };

export interface MlsMembershipChangeEventPayload {
  groupId: GroupId;
  conversationId: ConversationId;
  epoch: number;
  committerDeviceId: DeviceId;
  membershipChange: MlsMembershipChangePayload;
  createdAtUnix: number;
}

type E2eeGatewayEvent =
  | { type: "device_list_update"; payload: DeviceListUpdatePayload }
  | { type: "keypackage_low"; payload: KeyPackageLowPayload }
  | { type: "mls_message"; payload: MlsMessagePayload }
  | { type: "mls_commit"; payload: MlsCommitPayload }
  | { type: "mls_membership_change"; payload: MlsMembershipChangeEventPayload }
  | { type: "mls_proposal"; payload: MlsProposalPayload }
  | { type: "mls_welcome"; payload: MlsWelcomePayload };

type E2eeGatewayEventType = E2eeGatewayEvent["type"];
type E2eeEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

function hasExactKeys(value: Record<string, unknown>, expected: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const required = [...expected].sort();
  return actual.length === required.length
    && actual.every((key, index) => key === required[index]);
}

function isBoundedCount(value: unknown, maximum: number, allowZero: boolean): value is number {
  return typeof value === "number"
    && Number.isSafeInteger(value)
    && value >= (allowZero ? 0 : 1)
    && value <= maximum;
}

function isUnixTimestamp(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 1;
}

function isEpoch(value: unknown): value is number {
  return typeof value === "number" && Number.isSafeInteger(value) && value >= 0;
}

function isKnownMlsSuite(value: unknown): value is number {
  return typeof value === "number"
    && Number.isSafeInteger(value)
    && value >= 1
    && value <= 7;
}

function parseDeviceListUpdate(payload: unknown): DeviceListUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    !hasExactKeys(value, ["user_id", "device_count", "created_at_unix"])
    || typeof value.user_id !== "string"
    || !isBoundedCount(value.device_count, MAX_DEVICE_COUNT, true)
    || !isUnixTimestamp(value.created_at_unix)
  ) {
    return null;
  }

  try {
    return {
      userId: userIdFromInput(value.user_id),
      deviceCount: value.device_count,
      createdAtUnix: value.created_at_unix,
    };
  } catch {
    return null;
  }
}

function parseKeyPackageLow(payload: unknown): KeyPackageLowPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    !hasExactKeys(value, ["device_id", "remaining_count", "water_mark", "created_at_unix"])
    || typeof value.device_id !== "string"
    || !isBoundedCount(value.remaining_count, MAX_KEYPACKAGE_COUNT, true)
    || !isBoundedCount(value.water_mark, MAX_KEYPACKAGE_COUNT, false)
    || value.remaining_count >= value.water_mark
    || !isUnixTimestamp(value.created_at_unix)
  ) {
    return null;
  }

  try {
    return {
      deviceId: deviceIdFromInput(value.device_id),
      remainingCount: value.remaining_count,
      waterMark: value.water_mark,
      createdAtUnix: value.created_at_unix,
    };
  } catch {
    return null;
  }
}

function parseMlsMessage(payload: unknown): MlsMessagePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    !hasExactKeys(value, [
      "group_id",
      "conversation_id",
      "message_id",
      "epoch",
      "suite_id",
      "sender_device_id",
      "created_at_unix",
    ])
    || typeof value.group_id !== "string"
    || typeof value.conversation_id !== "string"
    || typeof value.message_id !== "string"
    || typeof value.sender_device_id !== "string"
    || !isEpoch(value.epoch)
    || !isKnownMlsSuite(value.suite_id)
    || !isUnixTimestamp(value.created_at_unix)
  ) {
    return null;
  }
  try {
    return {
      groupId: groupIdFromInput(value.group_id),
      conversationId: conversationIdFromInput(value.conversation_id),
      messageId: messageIdFromInput(value.message_id),
      epoch: value.epoch,
      suiteId: value.suite_id,
      senderDeviceId: deviceIdFromInput(value.sender_device_id),
      createdAtUnix: value.created_at_unix,
    };
  } catch {
    return null;
  }
}

function parseMlsCommit(payload: unknown): MlsCommitPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    !hasExactKeys(value, [
      "group_id",
      "conversation_id",
      "epoch",
      "prior_epoch",
      "committer_device_id",
      "created_at_unix",
    ])
    || typeof value.group_id !== "string"
    || typeof value.conversation_id !== "string"
    || typeof value.committer_device_id !== "string"
    || !isEpoch(value.epoch)
    || !isEpoch(value.prior_epoch)
    || value.prior_epoch + 1 !== value.epoch
    || !isUnixTimestamp(value.created_at_unix)
  ) {
    return null;
  }
  try {
    return {
      groupId: groupIdFromInput(value.group_id),
      conversationId: conversationIdFromInput(value.conversation_id),
      epoch: value.epoch,
      priorEpoch: value.prior_epoch,
      committerDeviceId: deviceIdFromInput(value.committer_device_id),
      createdAtUnix: value.created_at_unix,
    };
  } catch {
    return null;
  }
}

function parseMlsWelcome(payload: unknown): MlsWelcomePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    !hasExactKeys(value, [
      "group_id",
      "conversation_id",
      "epoch",
      "suite_id",
      "created_at_unix",
    ])
    || typeof value.group_id !== "string"
    || typeof value.conversation_id !== "string"
    || !isEpoch(value.epoch)
    || !isKnownMlsSuite(value.suite_id)
    || !isUnixTimestamp(value.created_at_unix)
  ) {
    return null;
  }
  try {
    return {
      groupId: groupIdFromInput(value.group_id),
      conversationId: conversationIdFromInput(value.conversation_id),
      epoch: value.epoch,
      suiteId: value.suite_id,
      createdAtUnix: value.created_at_unix,
    };
  } catch {
    return null;
  }
}

function parseMlsProposal(payload: unknown): MlsProposalPayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  const baseKeys = ["group_id", "conversation_id", "proposal_id", "epoch", "created_at_unix"];
  const isMember = hasExactKeys(value, [...baseKeys, "proposer_device_id"]);
  const isExternal = hasExactKeys(value, [
    ...baseKeys,
    "external_sender_index",
    "reconciliation_deadline_unix",
  ]);
  if (
    (!isMember && !isExternal)
    || typeof value.group_id !== "string"
    || typeof value.conversation_id !== "string"
    || typeof value.proposal_id !== "string"
    || (isMember && typeof value.proposer_device_id !== "string")
    || (isExternal && value.external_sender_index !== 0)
    || (isExternal && !isUnixTimestamp(value.reconciliation_deadline_unix))
    || !isEpoch(value.epoch)
    || !isUnixTimestamp(value.created_at_unix)
  ) {
    return null;
  }
  try {
    return {
      groupId: groupIdFromInput(value.group_id),
      conversationId: conversationIdFromInput(value.conversation_id),
      proposalId: proposalIdFromInput(value.proposal_id),
      epoch: value.epoch,
      proposerDeviceId: isMember
        ? deviceIdFromInput(value.proposer_device_id as string)
        : null,
      externalSenderIndex: isExternal ? 0 : null,
      reconciliationDeadlineUnix: isExternal
        ? value.reconciliation_deadline_unix as number
        : null,
      createdAtUnix: value.created_at_unix,
    };
  } catch {
    return null;
  }
}

function parseMlsLeaf(value: unknown): MlsLeafRoutingPayload | null {
  if (!value || typeof value !== "object") return null;
  const leaf = value as Record<string, unknown>;
  if (
    !hasExactKeys(leaf, ["leaf_index", "user_id", "device_id"])
    || !isBoundedCount(leaf.leaf_index, 199, true)
    || typeof leaf.user_id !== "string"
    || typeof leaf.device_id !== "string"
  ) return null;
  try {
    return {
      leafIndex: leaf.leaf_index,
      userId: userIdFromInput(leaf.user_id),
      deviceId: deviceIdFromInput(leaf.device_id),
    };
  } catch {
    return null;
  }
}

function parseMlsMembershipChange(payload: unknown): MlsMembershipChangeEventPayload | null {
  if (!payload || typeof payload !== "object") return null;
  const value = payload as Record<string, unknown>;
  if (
    !hasExactKeys(value, [
      "group_id",
      "conversation_id",
      "epoch",
      "committer_device_id",
      "membership_change",
      "created_at_unix",
    ])
    || typeof value.group_id !== "string"
    || typeof value.conversation_id !== "string"
    || typeof value.committer_device_id !== "string"
    || !isEpoch(value.epoch)
    || !isUnixTimestamp(value.created_at_unix)
    || !value.membership_change
    || typeof value.membership_change !== "object"
  ) return null;
  const rawChange = value.membership_change as Record<string, unknown>;
  let membershipChange: MlsMembershipChangePayload;
  if (rawChange.kind === "add" && hasExactKeys(rawChange, ["kind", "leaf"])) {
    const leaf = parseMlsLeaf(rawChange.leaf);
    if (!leaf) return null;
    membershipChange = { kind: "add", leaf };
  } else if (
    rawChange.kind === "remove"
    && hasExactKeys(rawChange, ["kind", "leaves"])
    && Array.isArray(rawChange.leaves)
    && rawChange.leaves.length >= 1
    && rawChange.leaves.length <= 100
  ) {
    const leaves = rawChange.leaves.map(parseMlsLeaf);
    if (leaves.some((leaf) => leaf === null)) return null;
    const parsedLeaves = leaves as MlsLeafRoutingPayload[];
    if (new Set(parsedLeaves.map((leaf) => leaf.leafIndex)).size !== parsedLeaves.length) return null;
    membershipChange = { kind: "remove", leaves: parsedLeaves };
  } else {
    return null;
  }
  try {
    return {
      groupId: groupIdFromInput(value.group_id),
      conversationId: conversationIdFromInput(value.conversation_id),
      epoch: value.epoch,
      committerDeviceId: deviceIdFromInput(value.committer_device_id),
      membershipChange,
      createdAtUnix: value.created_at_unix,
    };
  } catch {
    return null;
  }
}

const E2EE_EVENT_DECODERS: {
  [K in E2eeGatewayEventType]: E2eeEventDecoder<
    Extract<E2eeGatewayEvent, { type: K }>["payload"]
  >;
} = {
  device_list_update: parseDeviceListUpdate,
  keypackage_low: parseKeyPackageLow,
  mls_message: parseMlsMessage,
  mls_commit: parseMlsCommit,
  mls_membership_change: parseMlsMembershipChange,
  mls_proposal: parseMlsProposal,
  mls_welcome: parseMlsWelcome,
};

export function decodeE2eeGatewayEvent(
  type: string,
  payload: unknown,
): E2eeGatewayEvent | null {
  if (type === "device_list_update") {
    const parsed = E2EE_EVENT_DECODERS.device_list_update(payload);
    return parsed ? { type, payload: parsed } : null;
  }
  if (type === "keypackage_low") {
    const parsed = E2EE_EVENT_DECODERS.keypackage_low(payload);
    return parsed ? { type, payload: parsed } : null;
  }
  if (type === "mls_message") {
    const parsed = E2EE_EVENT_DECODERS.mls_message(payload);
    return parsed ? { type, payload: parsed } : null;
  }
  if (type === "mls_commit") {
    const parsed = E2EE_EVENT_DECODERS.mls_commit(payload);
    return parsed ? { type, payload: parsed } : null;
  }
  if (type === "mls_membership_change") {
    const parsed = E2EE_EVENT_DECODERS.mls_membership_change(payload);
    return parsed ? { type, payload: parsed } : null;
  }
  if (type === "mls_proposal") {
    const parsed = E2EE_EVENT_DECODERS.mls_proposal(payload);
    return parsed ? { type, payload: parsed } : null;
  }
  if (type === "mls_welcome") {
    const parsed = E2EE_EVENT_DECODERS.mls_welcome(payload);
    return parsed ? { type, payload: parsed } : null;
  }
  return null;
}
