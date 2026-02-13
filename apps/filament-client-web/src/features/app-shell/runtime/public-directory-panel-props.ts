import type { PublicDirectoryPanelBuilderOptions } from "../adapters/panel-host-props";

export interface PublicDirectoryPanelPropsOptions {
  publicGuildSearchQuery: string;
  isSearchingPublicGuilds: boolean;
  publicGuildSearchError: string;
  publicGuildDirectory: PublicDirectoryPanelBuilderOptions["publicGuildDirectory"];
  publicGuildJoinStatusByGuildId:
    PublicDirectoryPanelBuilderOptions["publicGuildJoinStatusByGuildId"];
  publicGuildJoinErrorByGuildId:
    PublicDirectoryPanelBuilderOptions["publicGuildJoinErrorByGuildId"];
  onSubmitPublicGuildSearch:
    PublicDirectoryPanelBuilderOptions["onSubmitPublicGuildSearch"];
  onJoinGuildFromDirectory:
    PublicDirectoryPanelBuilderOptions["onJoinGuildFromDirectory"];
  setPublicGuildSearchQuery:
    PublicDirectoryPanelBuilderOptions["setPublicGuildSearchQuery"];
}

export function createPublicDirectoryPanelProps(
  options: PublicDirectoryPanelPropsOptions,
): PublicDirectoryPanelBuilderOptions {
  return {
    publicGuildSearchQuery: options.publicGuildSearchQuery,
    isSearchingPublicGuilds: options.isSearchingPublicGuilds,
    publicGuildSearchError: options.publicGuildSearchError,
    publicGuildDirectory: options.publicGuildDirectory,
    publicGuildJoinStatusByGuildId: options.publicGuildJoinStatusByGuildId,
    publicGuildJoinErrorByGuildId: options.publicGuildJoinErrorByGuildId,
    onSubmitPublicGuildSearch: options.onSubmitPublicGuildSearch,
    onJoinGuildFromDirectory: options.onJoinGuildFromDirectory,
    setPublicGuildSearchQuery: options.setPublicGuildSearchQuery,
  };
}