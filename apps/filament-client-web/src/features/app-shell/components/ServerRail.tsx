import { For } from "solid-js";
import type { ChannelId, GuildId, WorkspaceRecord } from "../../../domain/chat";
import type { OverlayPanel } from "../types";

interface ServerRailProps {
  workspaces: WorkspaceRecord[];
  activeGuildId: GuildId | null;
  isCreatingWorkspace: boolean;
  onSelectWorkspace: (guildId: GuildId, firstChannelId: ChannelId | null) => void;
  onOpenPanel: (panel: OverlayPanel) => void;
}

export function ServerRail(props: ServerRailProps) {
  return (
    <aside class="server-rail" aria-label="servers">
      <header class="rail-label">WS</header>
      <div class="server-list">
        <For each={props.workspaces}>
          {(workspace) => (
            <button
              title={`${workspace.guildName} (${workspace.visibility})`}
              classList={{ active: props.activeGuildId === workspace.guildId }}
              onClick={() =>
                props.onSelectWorkspace(workspace.guildId, workspace.channels[0]?.channelId ?? null)}
            >
              {workspace.guildName.slice(0, 1).toUpperCase()}
            </button>
          )}
        </For>
      </div>
      <div class="server-rail-footer">
        <button
          type="button"
          class="server-action"
          aria-label="Open workspace create panel"
          title="Create workspace"
          onClick={() => props.onOpenPanel("workspace-create")}
          disabled={props.isCreatingWorkspace}
        >
          +
        </button>
        <button
          type="button"
          class="server-action"
          aria-label="Open public workspace directory panel"
          title="Public workspace directory"
          onClick={() => props.onOpenPanel("public-directory")}
        >
          D
        </button>
        <button
          type="button"
          class="server-action"
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
