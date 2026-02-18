import { Match, Show, Suspense, Switch, lazy } from "solid-js";
import type { OverlayPanel } from "../../types";
import { ChannelCreatePanel, type ChannelCreatePanelProps } from "./ChannelCreatePanel";
import type { AttachmentsPanelProps } from "./AttachmentsPanel";
import type { FriendshipsPanelProps } from "./FriendshipsPanel";
import type { ModerationPanelProps } from "./ModerationPanel";
import type { PublicDirectoryPanelProps } from "./PublicDirectoryPanel";
import type { RoleManagementPanelProps } from "./RoleManagementPanel";
import type { SearchPanelProps } from "./SearchPanel";
import type { SettingsPanelProps } from "./SettingsPanel";
import type { UtilityPanelProps } from "./UtilityPanel";
import type { WorkspaceSettingsPanelProps } from "./WorkspaceSettingsPanel";
import { WorkspaceCreatePanel, type WorkspaceCreatePanelProps } from "./WorkspaceCreatePanel";

const PublicDirectoryPanelLazy = lazy(() =>
  import("./lazy/PublicPanelGroup").then((module) => ({
    default: module.PublicDirectoryPanel,
  })),
);
const SettingsPanelLazy = lazy(() =>
  import("./lazy/PublicPanelGroup").then((module) => ({
    default: module.SettingsPanel,
  })),
);
const WorkspaceSettingsPanelLazy = lazy(() =>
  import("./lazy/PublicPanelGroup").then((module) => ({
    default: module.WorkspaceSettingsPanel,
  })),
);
const FriendshipsPanelLazy = lazy(() =>
  import("./lazy/PublicPanelGroup").then((module) => ({
    default: module.FriendshipsPanel,
  })),
);

const SearchPanelLazy = lazy(() =>
  import("./lazy/OperatorPanelGroup").then((module) => ({
    default: module.SearchPanel,
  })),
);
const AttachmentsPanelLazy = lazy(() =>
  import("./lazy/OperatorPanelGroup").then((module) => ({
    default: module.AttachmentsPanel,
  })),
);
const ModerationPanelLazy = lazy(() =>
  import("./lazy/OperatorPanelGroup").then((module) => ({
    default: module.ModerationPanel,
  })),
);
const RoleManagementPanelLazy = lazy(() =>
  import("./lazy/OperatorPanelGroup").then((module) => ({
    default: module.RoleManagementPanel,
  })),
);
const UtilityPanelLazy = lazy(() =>
  import("./lazy/OperatorPanelGroup").then((module) => ({
    default: module.UtilityPanel,
  })),
);

export interface PanelHostProps {
  panel: OverlayPanel | null;
  canCloseActivePanel: boolean;
  canManageWorkspaceChannels: boolean;
  canAccessActiveChannel: boolean;
  hasRoleManagementAccess: boolean;
  hasModerationAccess: boolean;
  panelTitle: (panel: OverlayPanel) => string;
  panelClassName: (panel: OverlayPanel) => string;
  onClose: () => void;
  workspaceCreatePanelProps: WorkspaceCreatePanelProps;
  channelCreatePanelProps: ChannelCreatePanelProps;
  publicDirectoryPanelProps: PublicDirectoryPanelProps;
  settingsPanelProps: SettingsPanelProps;
  workspaceSettingsPanelProps: WorkspaceSettingsPanelProps;
  friendshipsPanelProps: FriendshipsPanelProps;
  searchPanelProps: SearchPanelProps;
  attachmentsPanelProps: AttachmentsPanelProps;
  moderationPanelProps: ModerationPanelProps;
  roleManagementPanelProps: RoleManagementPanelProps;
  utilityPanelProps: UtilityPanelProps;
}

function panelWindowWidthClassName(panelClassName: string): string {
  if (panelClassName.includes("panel-window-compact")) {
    return "w-full md:w-[min(30rem,100%)]";
  }
  if (panelClassName.includes("panel-window-medium")) {
    return "w-full md:w-[min(42rem,100%)]";
  }
  return "w-full md:w-[min(52rem,100%)]";
}

export function PanelHost(props: PanelHostProps) {
  return (
    <Show when={props.panel}>
      {(panelAccessor) => {
        const panel = panelAccessor();
        const panelClassName = props.panelClassName(panel);
        const panelWidthClassName = panelWindowWidthClassName(panelClassName);
        return (
          <div
            class="panel-backdrop fixed inset-0 z-20 grid place-items-center bg-bg-0/80 p-[0.55rem] md:p-4"
            role="presentation"
            onClick={(event) => {
              if (event.target === event.currentTarget) {
                props.onClose();
              }
            }}
          >
            <section
              class={`${panelClassName} ${panelWidthClassName} max-h-[94vh] overflow-hidden rounded-[0.9rem] border border-line bg-bg-2 shadow-panel md:max-h-[min(88vh,50rem)] grid grid-rows-[auto_minmax(0,1fr)]`}
              role="dialog"
              aria-modal="true"
              aria-label={`${props.panelTitle(panel)} panel`}
            >
              <header class="panel-window-header flex items-center justify-between gap-2 border-b border-line px-[0.92rem] py-[0.78rem]">
                <h4 class="m-0">{props.panelTitle(panel)}</h4>
                <button
                  class="inline-flex items-center justify-center rounded-[0.58rem] border border-line-soft bg-bg-3 px-[0.62rem] py-[0.38rem] text-ink-1 transition-colors duration-[140ms] ease-out hover:bg-bg-4 disabled:cursor-not-allowed disabled:opacity-60"
                  type="button"
                  onClick={props.onClose}
                  disabled={!props.canCloseActivePanel}
                >
                  Close
                </button>
              </header>
              <div class="panel-window-body grid content-start gap-[0.7rem] overflow-auto px-[0.92rem] py-[0.85rem]">
                <Suspense fallback={<p class="m-[0.5rem_1rem_0] text-ink-2">Loading panel...</p>}>
                  <Switch>
                    <Match when={panelAccessor() === "workspace-create"}>
                      <WorkspaceCreatePanel {...props.workspaceCreatePanelProps} />
                    </Match>

                    <Match when={panelAccessor() === "channel-create" && props.canManageWorkspaceChannels}>
                      <ChannelCreatePanel {...props.channelCreatePanelProps} />
                    </Match>

                    <Match when={panelAccessor() === "public-directory"}>
                      <PublicDirectoryPanelLazy {...props.publicDirectoryPanelProps} />
                    </Match>

                    <Match when={panelAccessor() === "client-settings"}>
                      <SettingsPanelLazy {...props.settingsPanelProps} />
                    </Match>

                    <Match when={panelAccessor() === "workspace-settings"}>
                      <WorkspaceSettingsPanelLazy {...props.workspaceSettingsPanelProps} />
                    </Match>

                    <Match when={panelAccessor() === "friendships"}>
                      <FriendshipsPanelLazy {...props.friendshipsPanelProps} />
                    </Match>

                    <Match when={panelAccessor() === "search" && props.canAccessActiveChannel}>
                      <SearchPanelLazy {...props.searchPanelProps} />
                    </Match>

                    <Match when={panelAccessor() === "attachments" && props.canAccessActiveChannel}>
                      <AttachmentsPanelLazy {...props.attachmentsPanelProps} />
                    </Match>

                    <Match when={panelAccessor() === "moderation" && props.hasModerationAccess}>
                      <ModerationPanelLazy {...props.moderationPanelProps} />
                    </Match>

                    <Match
                      when={panelAccessor() === "role-management" && props.hasRoleManagementAccess}
                    >
                      <RoleManagementPanelLazy {...props.roleManagementPanelProps} />
                    </Match>

                    <Match when={panelAccessor() === "utility"}>
                      <UtilityPanelLazy {...props.utilityPanelProps} />
                    </Match>
                  </Switch>
                </Suspense>
              </div>
            </section>
          </div>
        );
      }}
    </Show>
  );
}
