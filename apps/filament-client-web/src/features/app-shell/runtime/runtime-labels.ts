import type { Accessor } from "solid-js";
import {
  shortActor,
  userIdFromVoiceIdentity,
} from "../helpers";

export interface AppShellRuntimeLabels {
  actorLookupId: (actorId: string) => string;
  actorLabel: (actorId: string) => string;
  displayUserLabel: (userId: string) => string;
  voiceParticipantLabel: (identity: string, isLocal: boolean) => string;
}

export interface CreateAppShellRuntimeLabelsOptions {
  resolvedUsernames: Accessor<Record<string, string>>;
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

  return {
    actorLookupId,
    actorLabel,
    displayUserLabel: (userId: string) => actorLabel(userId),
    voiceParticipantLabel: (identity: string, isLocal: boolean): string => {
      const label = actorLabel(identity);
      return isLocal ? `${label} (you)` : label;
    },
  };
}
