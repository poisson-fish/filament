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

const formControlClassName =
  "rounded-[0.62rem] border border-line-soft bg-bg-0 px-[0.62rem] py-[0.55rem] text-ink-1 placeholder:text-ink-2 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-brand/60";
const actionButtonClassName =
  "inline-flex items-center justify-center rounded-[0.62rem] border border-brand/45 bg-brand/15 px-[0.72rem] py-[0.5rem] text-ink-0 transition-colors duration-[140ms] ease-out enabled:hover:bg-brand/24 disabled:cursor-not-allowed disabled:opacity-60";
const directoryItemClassName =
  "flex items-start gap-[0.45rem] rounded-[0.6rem] border border-line-soft bg-bg-1 px-[0.5rem] py-[0.42rem]";
const joinStatusBaseClassName =
  "inline-flex items-center rounded-full border px-[0.5rem] py-[0.08rem] text-[0.7rem] tracking-[0.06em] lowercase";

function joinStatusClassName(status: PublicDirectoryJoinStatus): string {
  if (status === "joined") {
    return "border-ok/80 bg-ok/18 text-ok";
  }
  if (status === "banned" || status === "join_failed") {
    return "border-danger/80 bg-danger/16 text-danger";
  }
  if (status === "joining") {
    return "border-brand/80 bg-brand/20 text-ink-1";
  }
  return "border-line-soft bg-bg-3 text-ink-1";
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
    <section
      class="public-directory grid content-start gap-[0.45rem]"
      aria-label="public-workspace-directory"
    >
      <form class="grid gap-[0.5rem]" onSubmit={props.onSubmitSearch}>
        <label class="grid gap-[0.3rem] text-[0.84rem] text-ink-1">
          Search
          <input
            class={formControlClassName}
            value={props.searchQuery}
            onInput={(event) => props.onSearchInput(event.currentTarget.value)}
            maxlength="64"
            placeholder="workspace name"
          />
        </label>
        <button class={actionButtonClassName} type="submit" disabled={props.isSearching}>
          {props.isSearching ? "Searching..." : "Find public"}
        </button>
      </form>
      <Show when={props.searchError}>
        <p class="status error">{props.searchError}</p>
      </Show>
      <ul class="m-0 grid list-none gap-[0.35rem] p-0">
        <For each={props.guilds}>
          {(guild) => {
            const status = (): PublicDirectoryJoinStatus =>
              props.joinStatusByGuildId[guild.guildId] ?? "idle";
            const joinError = (): string => props.joinErrorByGuildId[guild.guildId] ?? "";
            return (
              <li class={directoryItemClassName}>
                <span class="presence online" />
                <div class="grid w-full gap-[0.35rem]">
                  <div class="grid min-w-0 gap-[0.16rem]">
                    <span>{guild.name}</span>
                    <span class="muted text-[0.78rem] font-code">{guild.visibility}</span>
                  </div>
                  <div class="flex items-center justify-between gap-[0.4rem]">
                    <Show when={status() !== "idle"}>
                      <span class={`${joinStatusBaseClassName} ${joinStatusClassName(status())}`}>
                        {status()}
                      </span>
                    </Show>
                    <button
                      class={actionButtonClassName}
                      type="button"
                      disabled={shouldDisableJoin(status())}
                      onClick={() => void props.onJoinGuild(guild.guildId)}
                    >
                      {joinLabel(status())}
                    </button>
                  </div>
                  <Show when={joinError()}>
                    <p class="m-0 text-[0.91rem] text-danger">{joinError()}</p>
                  </Show>
                </div>
              </li>
            );
          }}
        </For>
        <Show when={!props.isSearching && props.guilds.length === 0}>
          <li class={directoryItemClassName}>
            <span class="presence idle" />
            no-public-workspaces
          </li>
        </Show>
      </ul>
    </section>
  );
}
