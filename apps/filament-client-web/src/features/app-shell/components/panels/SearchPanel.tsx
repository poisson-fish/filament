import { For, Show } from "solid-js";
import type { SearchResults } from "../../../../domain/chat";
import { tokenizeToDisplayText } from "../../helpers";

export interface SearchPanelProps {
  searchQuery: string;
  isSearching: boolean;
  hasActiveWorkspace: boolean;
  canManageSearchMaintenance: boolean;
  isRunningSearchOps: boolean;
  searchOpsStatus: string;
  searchError: string;
  searchResults: SearchResults | null;
  onSubmitSearch: (event: SubmitEvent) => Promise<void> | void;
  onSearchQueryInput: (value: string) => void;
  onRebuildSearch: () => Promise<void> | void;
  onReconcileSearch: () => Promise<void> | void;
  displayUserLabel: (userId: string) => string;
}

export function SearchPanel(props: SearchPanelProps) {
  const presenceDotClass = "inline-block h-[0.58rem] w-[0.58rem] rounded-full";
  const onlinePresenceDotClass = `${presenceDotClass} bg-presence-online`;

  return (
    <section class="member-group">
      <form class="inline-form" onSubmit={props.onSubmitSearch}>
        <label>
          Query
          <input
            value={props.searchQuery}
            onInput={(event) => props.onSearchQueryInput(event.currentTarget.value)}
            maxlength="256"
            placeholder="needle"
          />
        </label>
        <button type="submit" disabled={props.isSearching || !props.hasActiveWorkspace}>
          {props.isSearching ? "Searching..." : "Search"}
        </button>
      </form>
      <Show when={props.canManageSearchMaintenance}>
        <div class="button-row">
          <button
            type="button"
            onClick={() => void props.onRebuildSearch()}
            disabled={props.isRunningSearchOps || !props.hasActiveWorkspace}
          >
            Rebuild Index
          </button>
          <button
            type="button"
            onClick={() => void props.onReconcileSearch()}
            disabled={props.isRunningSearchOps || !props.hasActiveWorkspace}
          >
            Reconcile Index
          </button>
        </div>
      </Show>
      <Show when={props.searchOpsStatus}>
        <p class="status ok">{props.searchOpsStatus}</p>
      </Show>
      <Show when={props.searchError}>
        <p class="status error">{props.searchError}</p>
      </Show>
      <Show when={props.searchResults}>
        {(resultsAccessor) => (
          <ul>
            <For each={resultsAccessor().messages}>
              {(message) => (
                <li>
                  <span class={onlinePresenceDotClass} />
                  {props.displayUserLabel(message.authorId)}:{" "}
                  {(tokenizeToDisplayText(message.markdownTokens) || message.content).slice(0, 40)}
                </li>
              )}
            </For>
          </ul>
        )}
      </Show>
    </section>
  );
}
