import type { ChannelId, GuildId, WorkspaceRecord } from "../../../domain/chat";

export interface WorkspaceSelection {
  guildId: GuildId | null;
  channelId: ChannelId | null;
}

export function filterAccessibleWorkspaces(
  workspaces: Array<WorkspaceRecord | null>,
): WorkspaceRecord[] {
  return workspaces.filter(
    (workspace): workspace is WorkspaceRecord =>
      workspace !== null && workspace.channels.length > 0,
  );
}

export function resolveWorkspaceSelection(
  workspaces: WorkspaceRecord[],
  selectedGuildId: GuildId | null,
  selectedChannelId: ChannelId | null,
): WorkspaceSelection {
  const selectedWorkspace =
    (selectedGuildId &&
      workspaces.find((workspace) => workspace.guildId === selectedGuildId)) ??
    workspaces[0] ??
    null;

  if (!selectedWorkspace) {
    return {
      guildId: null,
      channelId: null,
    };
  }

  const selectedChannel =
    (selectedChannelId &&
      selectedWorkspace.channels.find((channel) => channel.channelId === selectedChannelId)) ??
    selectedWorkspace.channels[0] ??
    null;

  return {
    guildId: selectedWorkspace.guildId,
    channelId: selectedChannel?.channelId ?? null,
  };
}
