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
  resolveUserNameColor?: (userId: string) => string | null;
}

export function SearchPanel(props: SearchPanelProps) {
  const panelSectionClass = "grid gap-[0.5rem]";
  const formClass = "grid gap-[0.5rem]";
  const fieldLabelClass = "grid gap-[0.3rem] text-[0.84rem] text-ink-1";
  const fieldControlClass =
    "rounded-[0.56rem] border border-line-soft bg-bg-2 px-[0.55rem] py-[0.62rem] text-ink-0 disabled:cursor-default disabled:opacity-62";
  const buttonRowClass = "flex gap-[0.45rem]";
  const actionButtonClass =
    "min-h-[1.95rem] flex-1 rounded-[0.56rem] border border-line-soft bg-bg-3 px-[0.68rem] py-[0.44rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-4 disabled:cursor-default disabled:opacity-62";
  const submitButtonClass =
    "min-h-[1.95rem] rounded-[0.56rem] border border-line-soft bg-bg-3 px-[0.68rem] py-[0.44rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-4 disabled:cursor-default disabled:opacity-62";
  const statusOkClass = "mt-[0.92rem] text-[0.91rem] text-ok";
  const statusErrorClass = "mt-[0.92rem] text-[0.91rem] text-danger";
  const presenceDotClass = "inline-block h-[0.58rem] w-[0.58rem] rounded-full";
  const onlinePresenceDotClass = `${presenceDotClass} bg-presence-online`;
  const resultsListClass = "m-0 grid list-none gap-[0.42rem] p-0";
  const resultsListItemClass =
    "flex items-start gap-[0.45rem] overflow-hidden rounded-[0.6rem] border border-line-soft bg-bg-2 px-[0.55rem] py-[0.5rem]";
  const resultsListTextClass = "min-w-0 break-words text-[0.84rem] text-ink-0";

  return (
    <section class={panelSectionClass}>
      <form class={formClass} onSubmit={props.onSubmitSearch}>
        <label class={fieldLabelClass}>
          Query
          <input
            class={fieldControlClass}
            value={props.searchQuery}
            onInput={(event) => props.onSearchQueryInput(event.currentTarget.value)}
            maxlength="256"
            placeholder="needle"
          />
        </label>
        <button
          class={submitButtonClass}
          type="submit"
          disabled={props.isSearching || !props.hasActiveWorkspace}
        >
          {props.isSearching ? "Searching..." : "Search"}
        </button>
      </form>
      <Show when={props.canManageSearchMaintenance}>
        <div class={buttonRowClass}>
          <button
            class={actionButtonClass}
            type="button"
            onClick={() => void props.onRebuildSearch()}
            disabled={props.isRunningSearchOps || !props.hasActiveWorkspace}
          >
            Rebuild Index
          </button>
          <button
            class={actionButtonClass}
            type="button"
            onClick={() => void props.onReconcileSearch()}
            disabled={props.isRunningSearchOps || !props.hasActiveWorkspace}
          >
            Reconcile Index
          </button>
        </div>
      </Show>
      <Show when={props.searchOpsStatus}>
        <p class={statusOkClass}>{props.searchOpsStatus}</p>
      </Show>
      <Show when={props.searchError}>
        <p class={statusErrorClass}>{props.searchError}</p>
      </Show>
      <Show when={props.searchResults}>
        {(resultsAccessor) => (
          <ul class={resultsListClass}>
            <For each={resultsAccessor().messages}>
              {(message) => (
                <li class={resultsListItemClass}>
                  <span class={onlinePresenceDotClass} />
                  <span class={resultsListTextClass}>
                    <strong
                      style={
                        props.resolveUserNameColor?.(message.authorId)
                          ? { color: props.resolveUserNameColor?.(message.authorId) ?? undefined }
                          : undefined
                      }
                    >
                      {props.displayUserLabel(message.authorId)}
                    </strong>
                    :{" "}
                    {(tokenizeToDisplayText(message.markdownTokens) || message.content).slice(0, 40)}
                  </span>
                </li>
              )}
            </For>
          </ul>
        )}
      </Show>
    </section>
  );
}
