import { Show, type ParentProps } from "solid-js";
import type { JSX } from "solid-js";

export interface AppShellLayoutProps extends ParentProps {
  isChannelRailCollapsed: boolean;
  isMemberRailCollapsed: boolean;
  serverRail: JSX.Element;
  channelRail: JSX.Element;
  streamColumn?: JSX.Element;
  chatColumn: JSX.Element;
  memberRail: JSX.Element;
  onCloseChannelRail?: () => void;
  onCloseMemberRail?: () => void;
}

export function AppShellLayout(props: AppShellLayoutProps) {
  const isAnyRailOpenOnMobile = () => !props.isChannelRailCollapsed || !props.isMemberRailCollapsed;

  return (
    <div class="app-shell-scaffold">
      <div
        classList={{
          "app-shell": true,
          "channel-rail-collapsed": props.isChannelRailCollapsed,
          "member-rail-collapsed": props.isMemberRailCollapsed,
          "with-stream": !!props.streamColumn,
        }}
      >
        {props.serverRail}

        {props.channelRail}

        <Show when={!!props.streamColumn}>
          {props.streamColumn}
        </Show>

        {props.chatColumn}

        {props.memberRail}

        <div
          class="mobile-scrim"
          classList={{
            "active": isAnyRailOpenOnMobile()
          }}
          onClick={() => {
            if (!props.isChannelRailCollapsed && props.onCloseChannelRail) {
              props.onCloseChannelRail();
            }
            if (!props.isMemberRailCollapsed && props.onCloseMemberRail) {
              props.onCloseMemberRail();
            }
          }}
        />
      </div>

      {props.children}
    </div>
  );
}
