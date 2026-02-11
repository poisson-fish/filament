import { Match, Show, Suspense, Switch, lazy } from "solid-js";
import type { OverlayPanel } from "../../types";
import { ChannelCreatePanel, type ChannelCreatePanelProps } from "./ChannelCreatePanel";
import type { AttachmentsPanelProps } from "./AttachmentsPanel";
import type { FriendshipsPanelProps } from "./FriendshipsPanel";
import type { ModerationPanelProps } from "./ModerationPanel";
import type { PublicDirectoryPanelProps } from "./PublicDirectoryPanel";
import type { SearchPanelProps } from "./SearchPanel";
import type { SettingsPanelProps } from "./SettingsPanel";
import type { UtilityPanelProps } from "./UtilityPanel";
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
  hasModerationAccess: boolean;
  panelTitle: (panel: OverlayPanel) => string;
  panelClassName: (panel: OverlayPanel) => string;
  onClose: () => void;
  workspaceCreatePanelProps: WorkspaceCreatePanelProps;
  channelCreatePanelProps: ChannelCreatePanelProps;
  publicDirectoryPanelProps: PublicDirectoryPanelProps;
  settingsPanelProps: SettingsPanelProps;
  friendshipsPanelProps: FriendshipsPanelProps;
  searchPanelProps: SearchPanelProps;
  attachmentsPanelProps: AttachmentsPanelProps;
  moderationPanelProps: ModerationPanelProps;
  utilityPanelProps: UtilityPanelProps;
}

export function PanelHost(props: PanelHostProps) {
  return (
    <Show when={props.panel}>
      {(panelAccessor) => (
        <div
          class="panel-backdrop"
          role="presentation"
          onClick={(event) => {
            if (event.target === event.currentTarget) {
              props.onClose();
            }
          }}
        >
          <section
            class={props.panelClassName(panelAccessor())}
            role="dialog"
            aria-modal="true"
            aria-label={`${props.panelTitle(panelAccessor())} panel`}
          >
            <header class="panel-window-header">
              <h4>{props.panelTitle(panelAccessor())}</h4>
              <button type="button" onClick={props.onClose} disabled={!props.canCloseActivePanel}>
                Close
              </button>
            </header>
            <div class="panel-window-body">
              <Suspense fallback={<p class="panel-note">Loading panel...</p>}>
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

                  <Match when={panelAccessor() === "settings"}>
                    <SettingsPanelLazy {...props.settingsPanelProps} />
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

                  <Match when={panelAccessor() === "utility"}>
                    <UtilityPanelLazy {...props.utilityPanelProps} />
                  </Match>
                </Switch>
              </Suspense>
            </div>
          </section>
        </div>
      )}
    </Show>
  );
}
