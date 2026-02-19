import { For } from "solid-js";
import type { ChannelId, GuildId, WorkspaceRecord } from "../../../domain/chat";
import type { OverlayPanel } from "../types";

export interface ServerRailProps {
  workspaces: WorkspaceRecord[];
  activeGuildId: GuildId | null;
  isCreatingWorkspace: boolean;
  onSelectWorkspace: (guildId: GuildId, firstChannelId: ChannelId | null) => void;
  onOpenPanel: (panel: OverlayPanel) => void;
}

export function ServerRail(props: ServerRailProps) {
  const railButtonClass =
    "inline-flex h-[3.05rem] w-[3.05rem] items-center justify-center rounded-[1rem] border border-line-soft bg-bg-2 text-ink-0 font-[800] leading-none transition-colors duration-[120ms] ease-out focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-[1px] focus-visible:outline-brand";

  return (
    <aside
      class="server-rail grid min-h-0 content-start gap-[0.52rem] bg-bg-1 px-[0.5rem] py-[0.7rem]"
      aria-label="servers"
    >
      <header class="m-0 text-center text-[0.58rem] tracking-[0.18em] text-ink-2">WS</header>
      <div class="grid content-start gap-[0.38rem]">
        <For each={props.workspaces}>
          {(workspace) => (
            <button
              title={`${workspace.guildName} (${workspace.visibility})`}
              class={`${railButtonClass} ${
                props.activeGuildId === workspace.guildId
                  ? "rounded-[0.9rem] border-brand bg-brand"
                  : "hover:bg-bg-3"
              }`}
              onClick={() =>
                props.onSelectWorkspace(workspace.guildId, workspace.channels[0]?.channelId ?? null)}
            >
              {workspace.guildName.slice(0, 1).toUpperCase()}
            </button>
          )}
        </For>
      </div>
      <div class="mt-auto grid gap-[0.38rem]">
        <button
          type="button"
          class={`${railButtonClass} text-[1rem] hover:bg-bg-3 disabled:cursor-default disabled:opacity-62`}
          aria-label="Open workspace create panel"
          title="Create workspace"
          onClick={() => props.onOpenPanel("workspace-create")}
          disabled={props.isCreatingWorkspace}
        >
          +
        </button>
        <button
          type="button"
          class={`${railButtonClass} text-[1rem] hover:bg-bg-3`}
          aria-label="Open public workspace directory panel"
          title="Public workspace directory"
          onClick={() => props.onOpenPanel("public-directory")}
        >
          D
        </button>
        <button
          type="button"
          class={`${railButtonClass} text-[1rem] hover:bg-bg-3`}
          aria-label="Open friendships panel"
          title="Friendships"
          onClick={() => props.onOpenPanel("friendships")}
        >
          F
        </button>
      </div>
    </aside>
  );
}
