import {
  channelKindFromInput,
  type ChannelId,
  type GuildId,
} from "../../../domain/chat";
import type { OverlayPanel } from "../types";

export interface WorkspaceSelectionActionsOptions {
  setNewChannelKind: (value: ReturnType<typeof channelKindFromInput>) => ReturnType<typeof channelKindFromInput>;
  openOverlayPanel: (panel: OverlayPanel) => void;
  setActiveGuildId: (value: GuildId) => GuildId;
  setActiveChannelId: (value: ChannelId | null) => ChannelId | null;
}

export function createWorkspaceSelectionActions(
  options: WorkspaceSelectionActionsOptions,
) {
  const openTextChannelCreatePanel = (): void => {
    options.setNewChannelKind(channelKindFromInput("text"));
    options.openOverlayPanel("channel-create");
  };

  const openVoiceChannelCreatePanel = (): void => {
    options.setNewChannelKind(channelKindFromInput("voice"));
    options.openOverlayPanel("channel-create");
  };

  const onSelectWorkspace = (
    guildId: GuildId,
    firstChannelId: ChannelId | null,
  ): void => {
    options.setActiveGuildId(guildId);
    options.setActiveChannelId(firstChannelId);
  };

  return {
    openTextChannelCreatePanel,
    openVoiceChannelCreatePanel,
    onSelectWorkspace,
  };
}