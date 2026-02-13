import type { BuildPanelHostPropGroupsOptions } from "../adapters/panel-host-props";
import {
  createAttachmentsPanelProps,
  type AttachmentsPanelPropsOptions,
} from "./attachments-panel-props";
import {
  createFriendshipsPanelProps,
  type FriendshipsPanelPropsOptions,
} from "./friendships-panel-props";
import {
  createModerationPanelProps,
  type ModerationPanelPropsOptions,
} from "./moderation-panel-props";
import {
  createSearchPanelProps,
  type SearchPanelPropsOptions,
} from "./search-panel-props";

export interface CollaborationPanelPropGroupsOptions {
  friendships: FriendshipsPanelPropsOptions;
  search: SearchPanelPropsOptions;
  attachments: AttachmentsPanelPropsOptions;
  moderation: ModerationPanelPropsOptions;
}

export function createCollaborationPanelPropGroups(
  options: CollaborationPanelPropGroupsOptions,
): Pick<
  BuildPanelHostPropGroupsOptions,
  "friendships" | "search" | "attachments" | "moderation"
> {
  return {
    friendships: createFriendshipsPanelProps(options.friendships),
    search: createSearchPanelProps(options.search),
    attachments: createAttachmentsPanelProps(options.attachments),
    moderation: createModerationPanelProps(options.moderation),
  };
}