import { For, Show } from "solid-js";
import type { FriendRecord, FriendRequestList, UserId } from "../../../../domain/chat";

export interface FriendshipsPanelProps {
  friendRecipientUserIdInput: string;
  friendRequests: FriendRequestList;
  friends: FriendRecord[];
  isRunningFriendAction: boolean;
  friendStatus: string;
  friendError: string;
  onSubmitFriendRequest: (event: SubmitEvent) => Promise<void> | void;
  onFriendRecipientInput: (value: string) => void;
  onAcceptIncomingFriendRequest: (requestId: string) => Promise<void> | void;
  onDismissFriendRequest: (requestId: string) => Promise<void> | void;
  onRemoveFriendship: (friendUserId: UserId) => Promise<void> | void;
}

export function FriendshipsPanel(props: FriendshipsPanelProps) {
  return (
    <section class="public-directory" aria-label="friendships">
      <form class="inline-form" onSubmit={props.onSubmitFriendRequest}>
        <label>
          User ID
          <input
            value={props.friendRecipientUserIdInput}
            onInput={(event) => props.onFriendRecipientInput(event.currentTarget.value)}
            maxlength="26"
            placeholder="01ARZ3NDEKTSV4RRFFQ69G5FAV"
          />
        </label>
        <button type="submit" disabled={props.isRunningFriendAction}>
          {props.isRunningFriendAction ? "Submitting..." : "Send request"}
        </button>
      </form>
      <Show when={props.friendStatus}>
        <p class="status ok">{props.friendStatus}</p>
      </Show>
      <Show when={props.friendError}>
        <p class="status error">{props.friendError}</p>
      </Show>

      <p class="group-label">INCOMING</p>
      <ul>
        <For each={props.friendRequests.incoming}>
          {(request) => (
            <li>
              <div class="stacked-meta">
                <span>{request.senderUsername}</span>
                <span class="muted mono">{request.senderUserId}</span>
              </div>
              <div class="button-row">
                <button
                  type="button"
                  onClick={() => void props.onAcceptIncomingFriendRequest(request.requestId)}
                  disabled={props.isRunningFriendAction}
                >
                  Accept
                </button>
                <button
                  type="button"
                  onClick={() => void props.onDismissFriendRequest(request.requestId)}
                  disabled={props.isRunningFriendAction}
                >
                  Ignore
                </button>
              </div>
            </li>
          )}
        </For>
        <Show when={props.friendRequests.incoming.length === 0}>
          <li class="muted">no-incoming-requests</li>
        </Show>
      </ul>

      <p class="group-label">OUTGOING</p>
      <ul>
        <For each={props.friendRequests.outgoing}>
          {(request) => (
            <li>
              <div class="stacked-meta">
                <span>{request.recipientUsername}</span>
                <span class="muted mono">{request.recipientUserId}</span>
              </div>
              <button
                type="button"
                onClick={() => void props.onDismissFriendRequest(request.requestId)}
                disabled={props.isRunningFriendAction}
              >
                Cancel
              </button>
            </li>
          )}
        </For>
        <Show when={props.friendRequests.outgoing.length === 0}>
          <li class="muted">no-outgoing-requests</li>
        </Show>
      </ul>

      <p class="group-label">FRIEND LIST</p>
      <ul>
        <For each={props.friends}>
          {(friend) => (
            <li>
              <div class="stacked-meta">
                <span>{friend.username}</span>
                <span class="muted mono">{friend.userId}</span>
              </div>
              <button
                type="button"
                onClick={() => void props.onRemoveFriendship(friend.userId)}
                disabled={props.isRunningFriendAction}
              >
                Remove
              </button>
            </li>
          )}
        </For>
        <Show when={props.friends.length === 0}>
          <li class="muted">no-friends</li>
        </Show>
      </ul>
    </section>
  );
}
