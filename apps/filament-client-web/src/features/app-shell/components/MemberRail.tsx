import { For, Show } from "solid-js";
import type { OverlayPanel } from "../types";

interface ProfileSummary {
  userId: string;
  username: string;
}

interface MemberRailProps {
  profileLoading: boolean;
  profileErrorText: string;
  profile: ProfileSummary | null;
  showUnauthorizedWorkspaceNote: boolean;
  canAccessActiveChannel: boolean;
  hasRoleManagementAccess: boolean;
  onlineMembers: string[];
  hasModerationAccess: boolean;
  displayUserLabel: (userId: string) => string;
  onOpenPanel: (panel: OverlayPanel) => void;
}

export function MemberRail(props: MemberRailProps) {
  const sectionClass = "grid gap-[0.5rem]";
  const sectionLabelClass =
    "m-0 text-[0.68rem] text-ink-2 tracking-[0.08em] uppercase";
  const memberListClass = "m-0 grid list-none gap-[0.42rem] p-0";
  const memberListRowClass =
    "flex items-center gap-[0.45rem] overflow-hidden rounded-[0.6rem] border border-line-soft bg-bg-2 px-[0.55rem] py-[0.5rem]";
  const presenceDotClass = "inline-block h-[0.58rem] w-[0.58rem] rounded-full";
  const onlinePresenceDotClass = `${presenceDotClass} bg-presence-online`;
  const idlePresenceDotClass = `${presenceDotClass} bg-presence-idle`;
  const panelButtonClass =
    "w-full rounded-[0.62rem] border border-line-soft bg-bg-3 px-[0.6rem] py-[0.48rem] text-left text-[0.82rem] text-ink-0 transition-colors duration-[120ms] ease-out enabled:hover:bg-bg-4 enabled:cursor-pointer";

  return (
    <aside class="member-rail grid min-h-0 content-start gap-[0.66rem] overflow-auto bg-bg-0 px-[0.78rem] py-[0.78rem]">
      <header>
        <h4 class="m-0 text-[0.94rem] text-ink-0 font-[700] tracking-[0.01em]">
          Workspace Tools
        </h4>
      </header>

      <Show when={props.profileLoading}>
        <p class="muted">Loading profile...</p>
      </Show>
      <Show when={props.profileErrorText}>
        <p class="status error">{props.profileErrorText}</p>
      </Show>
      <Show when={props.profile}>
        {(value) => (
          <div class="grid gap-[0.3rem] rounded-[0.64rem] border border-line-soft bg-bg-2 p-[0.72rem]">
            <p class="m-0 text-[0.72rem] text-ink-2 tracking-[0.07em] uppercase">
              Username
            </p>
            <p class="m-0 break-words text-[0.86rem] text-ink-0">{value().username}</p>
            <p class="m-0 pt-[0.1rem] text-[0.72rem] text-ink-2 tracking-[0.07em] uppercase">
              User ID
            </p>
            <p class="m-0 break-all text-[0.82rem] text-ink-1 font-code">
              {value().userId}
            </p>
          </div>
        )}
      </Show>

      <Show when={props.showUnauthorizedWorkspaceNote}>
        <p class="muted">No authorized workspace/channel selected for operator actions.</p>
      </Show>

      <Show when={props.canAccessActiveChannel}>
        <section class={sectionClass}>
          <p class={sectionLabelClass}>ONLINE ({props.onlineMembers.length})</p>
          <ul class={memberListClass}>
            <For each={props.onlineMembers}>
              {(memberId) => (
                <li class={memberListRowClass}>
                  <span class={onlinePresenceDotClass} />
                  <span class="min-w-0 break-words">{props.displayUserLabel(memberId)}</span>
                </li>
              )}
            </For>
            <Show when={props.onlineMembers.length === 0}>
              <li class={memberListRowClass}>
                <span class={idlePresenceDotClass} />
                <span class="min-w-0 break-words">no-presence-yet</span>
              </li>
            </Show>
          </ul>
        </section>
      </Show>

      <section class={sectionClass}>
        <p class={sectionLabelClass}>PANELS</p>
        <div class="grid gap-[0.44rem]">
          <button
            type="button"
            class={panelButtonClass}
            onClick={() => props.onOpenPanel("public-directory")}
          >
            Open directory panel
          </button>
          <button
            type="button"
            class={panelButtonClass}
            onClick={() => props.onOpenPanel("friendships")}
          >
            Open friendships panel
          </button>
          <Show when={props.canAccessActiveChannel}>
            <button
              type="button"
              class={panelButtonClass}
              onClick={() => props.onOpenPanel("search")}
            >
              Open search panel
            </button>
            <button
              type="button"
              class={panelButtonClass}
              onClick={() => props.onOpenPanel("attachments")}
            >
              Open attachments panel
            </button>
          </Show>
          <Show when={props.hasModerationAccess}>
            <button
              type="button"
              class={panelButtonClass}
              onClick={() => props.onOpenPanel("moderation")}
            >
              Open moderation panel
            </button>
          </Show>
          <Show when={props.hasRoleManagementAccess}>
            <button
              type="button"
              class={panelButtonClass}
              onClick={() => props.onOpenPanel("role-management")}
            >
              Open role management panel
            </button>
          </Show>
          <button
            type="button"
            class={panelButtonClass}
            onClick={() => props.onOpenPanel("utility")}
          >
            Open utility panel
          </button>
        </div>
      </section>
    </aside>
  );
}
