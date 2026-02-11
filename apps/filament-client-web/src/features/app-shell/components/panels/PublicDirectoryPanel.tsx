import { For, Show } from "solid-js";
import type { GuildRecord } from "../../../../domain/chat";

export interface PublicDirectoryPanelProps {
  searchQuery: string;
  isSearching: boolean;
  searchError: string;
  guilds: GuildRecord[];
  onSubmitSearch: (event: SubmitEvent) => Promise<void> | void;
  onSearchInput: (value: string) => void;
}

export function PublicDirectoryPanel(props: PublicDirectoryPanelProps) {
  return (
    <section class="public-directory" aria-label="public-workspace-directory">
      <form class="inline-form" onSubmit={props.onSubmitSearch}>
        <label>
          Search
          <input
            value={props.searchQuery}
            onInput={(event) => props.onSearchInput(event.currentTarget.value)}
            maxlength="64"
            placeholder="workspace name"
          />
        </label>
        <button type="submit" disabled={props.isSearching}>
          {props.isSearching ? "Searching..." : "Find public"}
        </button>
      </form>
      <Show when={props.searchError}>
        <p class="status error">{props.searchError}</p>
      </Show>
      <ul>
        <For each={props.guilds}>
          {(guild) => (
            <li>
              <span class="presence online" />
              <div class="stacked-meta">
                <span>{guild.name}</span>
                <span class="muted mono">{guild.visibility}</span>
              </div>
            </li>
          )}
        </For>
        <Show when={!props.isSearching && props.guilds.length === 0}>
          <li>
            <span class="presence idle" />
            no-public-workspaces
          </li>
        </Show>
      </ul>
    </section>
  );
}
