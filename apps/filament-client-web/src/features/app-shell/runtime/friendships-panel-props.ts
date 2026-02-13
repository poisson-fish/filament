import type {
  FriendshipsPanelBuilderOptions,
} from "../adapters/panel-host-props";

export interface FriendshipsPanelPropsOptions {
  friendRecipientUserIdInput: string;
  friendRequests: FriendshipsPanelBuilderOptions["friendRequests"];
  friends: FriendshipsPanelBuilderOptions["friends"];
  isRunningFriendAction: boolean;
  friendStatus: string;
  friendError: string;
  onSubmitFriendRequest: (event: SubmitEvent) => Promise<void> | void;
  setFriendRecipientUserIdInput: (value: string) => void;
  onAcceptIncomingFriendRequest: (requestId: string) => Promise<void> | void;
  onDismissFriendRequest: (requestId: string) => Promise<void> | void;
  onRemoveFriendship: FriendshipsPanelBuilderOptions["onRemoveFriendship"];
}

export function createFriendshipsPanelProps(
  options: FriendshipsPanelPropsOptions,
): FriendshipsPanelBuilderOptions {
  return {
    friendRecipientUserIdInput: options.friendRecipientUserIdInput,
    friendRequests: options.friendRequests,
    friends: options.friends,
    isRunningFriendAction: options.isRunningFriendAction,
    friendStatus: options.friendStatus,
    friendError: options.friendError,
    onSubmitFriendRequest: options.onSubmitFriendRequest,
    setFriendRecipientUserIdInput: options.setFriendRecipientUserIdInput,
    onAcceptIncomingFriendRequest: (requestId) =>
      options.onAcceptIncomingFriendRequest(requestId),
    onDismissFriendRequest: (requestId) =>
      options.onDismissFriendRequest(requestId),
    onRemoveFriendship: (friendUserId) => options.onRemoveFriendship(friendUserId),
  };
}