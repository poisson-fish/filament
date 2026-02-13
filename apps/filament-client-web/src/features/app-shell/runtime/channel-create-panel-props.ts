import type { ChannelCreatePanelBuilderOptions } from "../adapters/panel-host-props";

export interface ChannelCreatePanelPropsOptions {
  newChannelName: string;
  newChannelKind: ChannelCreatePanelBuilderOptions["newChannelKind"];
  isCreatingChannel: boolean;
  channelCreateError: string;
  onCreateChannelSubmit: ChannelCreatePanelBuilderOptions["onCreateChannelSubmit"];
  setNewChannelName: ChannelCreatePanelBuilderOptions["setNewChannelName"];
  setNewChannelKind: ChannelCreatePanelBuilderOptions["setNewChannelKind"];
  onCancelChannelCreate: ChannelCreatePanelBuilderOptions["onCancelChannelCreate"];
}

export function createChannelCreatePanelProps(
  options: ChannelCreatePanelPropsOptions,
): ChannelCreatePanelBuilderOptions {
  return {
    newChannelName: options.newChannelName,
    newChannelKind: options.newChannelKind,
    isCreatingChannel: options.isCreatingChannel,
    channelCreateError: options.channelCreateError,
    onCreateChannelSubmit: options.onCreateChannelSubmit,
    setNewChannelName: options.setNewChannelName,
    setNewChannelKind: options.setNewChannelKind,
    onCancelChannelCreate: options.onCancelChannelCreate,
  };
}