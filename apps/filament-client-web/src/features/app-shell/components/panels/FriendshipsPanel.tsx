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

const formControlClassName =
  "rounded-[0.62rem] border border-line-soft bg-bg-0 px-[0.62rem] py-[0.55rem] text-ink-1 placeholder:text-ink-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/60";
const actionButtonClassName =
  "inline-flex items-center justify-center rounded-[0.62rem] border border-brand/45 bg-brand/15 px-[0.72rem] py-[0.5rem] text-ink-0 transition-colors duration-[140ms] ease-out enabled:hover:bg-brand/24 disabled:cursor-not-allowed disabled:opacity-60";
const listClassName = "m-0 grid list-none gap-[0.35rem] p-0";
const listItemClassName =
  "flex items-start gap-[0.45rem] rounded-[0.6rem] border border-line-soft bg-bg-1 px-[0.5rem] py-[0.42rem]";

export function FriendshipsPanel(props: FriendshipsPanelProps) {
  return (
    <section class="public-directory grid content-start gap-[0.45rem]" aria-label="friendships">
      <form class="grid gap-[0.5rem]" onSubmit={props.onSubmitFriendRequest}>
        <label class="grid gap-[0.3rem] text-[0.84rem] text-ink-1">
          User ID
          <input
            class={formControlClassName}
            value={props.friendRecipientUserIdInput}
            onInput={(event) => props.onFriendRecipientInput(event.currentTarget.value)}
            maxlength="26"
            placeholder="01ARZ3NDEKTSV4RRFFQ69G5FAV"
          />
        </label>
        <button class={actionButtonClassName} type="submit" disabled={props.isRunningFriendAction}>
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
      <ul class={listClassName}>
        <For each={props.friendRequests.incoming}>
          {(request) => (
            <li class={listItemClassName}>
              <div class="stacked-meta">
                <span>{request.senderUsername}</span>
                <span class="muted mono">{request.senderUserId}</span>
              </div>
              <div class="ml-auto flex flex-1 gap-[0.45rem]">
                <button
                  class={`${actionButtonClassName} flex-1`}
                  type="button"
                  onClick={() => void props.onAcceptIncomingFriendRequest(request.requestId)}
                  disabled={props.isRunningFriendAction}
                >
                  Accept
                </button>
                <button
                  class={`${actionButtonClassName} flex-1`}
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
          <li class={`${listItemClassName} text-ink-2`}>no-incoming-requests</li>
        </Show>
      </ul>

      <p class="group-label">OUTGOING</p>
      <ul class={listClassName}>
        <For each={props.friendRequests.outgoing}>
          {(request) => (
            <li class={listItemClassName}>
              <div class="stacked-meta">
                <span>{request.recipientUsername}</span>
                <span class="muted mono">{request.recipientUserId}</span>
              </div>
              <button
                class={`${actionButtonClassName} ml-auto`}
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
          <li class={`${listItemClassName} text-ink-2`}>no-outgoing-requests</li>
        </Show>
      </ul>

      <p class="group-label">FRIEND LIST</p>
      <ul class={listClassName}>
        <For each={props.friends}>
          {(friend) => (
            <li class={listItemClassName}>
              <div class="stacked-meta">
                <span>{friend.username}</span>
                <span class="muted mono">{friend.userId}</span>
              </div>
              <button
                class={`${actionButtonClassName} ml-auto`}
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
          <li class={`${listItemClassName} text-ink-2`}>no-friends</li>
        </Show>
      </ul>
    </section>
  );
}
