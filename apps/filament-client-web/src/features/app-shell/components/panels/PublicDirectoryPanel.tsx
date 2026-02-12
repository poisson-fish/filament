import { For, Show } from "solid-js";
import type { GuildId, GuildRecord } from "../../../../domain/chat";
import type { PublicDirectoryJoinStatus } from "../../types";

export interface PublicDirectoryPanelProps {
  searchQuery: string;
  isSearching: boolean;
  searchError: string;
  guilds: GuildRecord[];
  joinStatusByGuildId: Record<string, PublicDirectoryJoinStatus>;
  joinErrorByGuildId: Record<string, string>;
  onSubmitSearch: (event: SubmitEvent) => Promise<void> | void;
  onJoinGuild: (guildId: GuildId) => Promise<void> | void;
  onSearchInput: (value: string) => void;
}

export function PublicDirectoryPanel(props: PublicDirectoryPanelProps) {
  const joinLabel = (status: PublicDirectoryJoinStatus): string => {
    if (status === "joining") {
      return "Joining...";
    }
    if (status === "joined") {
      return "Joined";
    }
    if (status === "banned") {
      return "Join blocked";
    }
    return "Join";
  };

  const shouldDisableJoin = (status: PublicDirectoryJoinStatus): boolean =>
    status === "joining" || status === "joined";

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
          {(guild) => {
            const status = (): PublicDirectoryJoinStatus =>
              props.joinStatusByGuildId[guild.guildId] ?? "idle";
            const joinError = (): string => props.joinErrorByGuildId[guild.guildId] ?? "";
            return (
              <li class="public-directory-row">
                <span class="presence online" />
                <div class="public-directory-row-main">
                  <div class="stacked-meta">
                    <span>{guild.name}</span>
                    <span class="muted mono">{guild.visibility}</span>
                  </div>
                  <div class="public-directory-row-actions">
                    <Show when={status() !== "idle"}>
                      <span
                        classList={{
                          "directory-status-chip": true,
                          joined: status() === "joined",
                          banned: status() === "banned",
                          failed: status() === "join_failed",
                          pending: status() === "joining",
                        }}
                      >
                        {status()}
                      </span>
                    </Show>
                    <button
                      type="button"
                      disabled={shouldDisableJoin(status())}
                      onClick={() => void props.onJoinGuild(guild.guildId)}
                    >
                      {joinLabel(status())}
                    </button>
                  </div>
                  <Show when={joinError()}>
                    <p class="status error public-directory-row-error">{joinError()}</p>
                  </Show>
                </div>
              </li>
            );
          }}
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
