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
}

export function AppShellLayout(props: AppShellLayoutProps) {
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

        <Show when={!props.isChannelRailCollapsed}>
          {props.channelRail}
        </Show>

        <Show when={!!props.streamColumn}>
          {props.streamColumn}
        </Show>

        {props.chatColumn}

        <Show when={!props.isMemberRailCollapsed}>
          {props.memberRail}
        </Show>
      </div>

      {props.children}
    </div>
  );
}
