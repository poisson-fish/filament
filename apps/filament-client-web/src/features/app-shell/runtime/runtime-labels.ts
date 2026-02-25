import type { Accessor } from "solid-js";
import type {
  GuildId,
  GuildRoleRecord,
  WorkspaceRoleId,
} from "../../../domain/chat";
import {
  shortActor,
  userIdFromVoiceIdentity,
} from "../helpers";

export interface AppShellRuntimeLabels {
  actorLookupId: (actorId: string) => string;
  actorLabel: (actorId: string) => string;
  actorColor: (actorId: string) => string | null;
  displayUserLabel: (userId: string) => string;
  displayUserColor: (userId: string) => string | null;
  voiceParticipantLabel: (identity: string, isLocal: boolean) => string;
}

export interface CreateAppShellRuntimeLabelsOptions {
  resolvedUsernames: Accessor<Record<string, string>>;
  activeGuildId: Accessor<GuildId | null>;
  workspaceRolesByGuildId: Accessor<Record<string, GuildRoleRecord[]>>;
  workspaceUserRolesByGuildId: Accessor<Record<string, Record<string, WorkspaceRoleId[]>>>;
}

export function createAppShellRuntimeLabels(
  options: CreateAppShellRuntimeLabelsOptions,
): AppShellRuntimeLabels {
  const actorLookupId = (actorId: string): string =>
    userIdFromVoiceIdentity(actorId) ?? actorId;

  const actorLabel = (actorId: string): string => {
    const lookupId = actorLookupId(actorId);
    return options.resolvedUsernames()[lookupId] ?? shortActor(lookupId);
  };

  const displayUserColor = (userId: string): string | null => {
    const guildId = options.activeGuildId();
    if (!guildId) {
      return null;
    }
    const assignedRoleIds = options.workspaceUserRolesByGuildId()[guildId]?.[userId];
    if (!assignedRoleIds || assignedRoleIds.length === 0) {
      return null;
    }
    const assignedRoleSet = new Set(assignedRoleIds);
    const roles = options.workspaceRolesByGuildId()[guildId] ?? [];
    for (const role of roles) {
      if (!role.colorHex) {
        continue;
      }
      if (assignedRoleSet.has(role.roleId)) {
        return role.colorHex;
      }
    }
    return null;
  };

  return {
    actorLookupId,
    actorLabel,
    actorColor: (actorId: string) => displayUserColor(actorLookupId(actorId)),
    displayUserLabel: (userId: string) => actorLabel(userId),
    displayUserColor,
    voiceParticipantLabel: (identity: string, isLocal: boolean): string => {
      const label = actorLabel(identity);
      return isLocal ? `${label} (you)` : label;
    },
  };
}
