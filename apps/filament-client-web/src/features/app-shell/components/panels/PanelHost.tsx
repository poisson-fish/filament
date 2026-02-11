import { Match, Show, Switch } from "solid-js";
import type { OverlayPanel } from "../../types";
import { AttachmentsPanel, type AttachmentsPanelProps } from "./AttachmentsPanel";
import { ChannelCreatePanel, type ChannelCreatePanelProps } from "./ChannelCreatePanel";
import { FriendshipsPanel, type FriendshipsPanelProps } from "./FriendshipsPanel";
import { ModerationPanel, type ModerationPanelProps } from "./ModerationPanel";
import { PublicDirectoryPanel, type PublicDirectoryPanelProps } from "./PublicDirectoryPanel";
import { SearchPanel, type SearchPanelProps } from "./SearchPanel";
import { SettingsPanel, type SettingsPanelProps } from "./SettingsPanel";
import { UtilityPanel, type UtilityPanelProps } from "./UtilityPanel";
import { WorkspaceCreatePanel, type WorkspaceCreatePanelProps } from "./WorkspaceCreatePanel";

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
              <Switch>
                <Match when={panelAccessor() === "workspace-create"}>
                  <WorkspaceCreatePanel {...props.workspaceCreatePanelProps} />
                </Match>

                <Match when={panelAccessor() === "channel-create" && props.canManageWorkspaceChannels}>
                  <ChannelCreatePanel {...props.channelCreatePanelProps} />
                </Match>

                <Match when={panelAccessor() === "public-directory"}>
                  <PublicDirectoryPanel {...props.publicDirectoryPanelProps} />
                </Match>

                <Match when={panelAccessor() === "settings"}>
                  <SettingsPanel {...props.settingsPanelProps} />
                </Match>

                <Match when={panelAccessor() === "friendships"}>
                  <FriendshipsPanel {...props.friendshipsPanelProps} />
                </Match>

                <Match when={panelAccessor() === "search" && props.canAccessActiveChannel}>
                  <SearchPanel {...props.searchPanelProps} />
                </Match>

                <Match when={panelAccessor() === "attachments" && props.canAccessActiveChannel}>
                  <AttachmentsPanel {...props.attachmentsPanelProps} />
                </Match>

                <Match when={panelAccessor() === "moderation" && props.hasModerationAccess}>
                  <ModerationPanel {...props.moderationPanelProps} />
                </Match>

                <Match when={panelAccessor() === "utility"}>
                  <UtilityPanel {...props.utilityPanelProps} />
                </Match>
              </Switch>
            </div>
          </section>
        </div>
      )}
    </Show>
  );
}
