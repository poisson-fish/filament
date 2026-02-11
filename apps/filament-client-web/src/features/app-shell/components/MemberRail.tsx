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
  onlineMembers: string[];
  hasModerationAccess: boolean;
  displayUserLabel: (userId: string) => string;
  onOpenPanel: (panel: OverlayPanel) => void;
}

export function MemberRail(props: MemberRailProps) {
  return (
    <aside class="member-rail">
      <header>
        <h4>Workspace Tools</h4>
      </header>

      <Show when={props.profileLoading}>
        <p class="muted">Loading profile...</p>
      </Show>
      <Show when={props.profileErrorText}>
        <p class="status error">{props.profileErrorText}</p>
      </Show>
      <Show when={props.profile}>
        {(value) => (
          <div class="profile-card">
            <p class="label">Username</p>
            <p>{value().username}</p>
            <p class="label">User ID</p>
            <p class="mono">{value().userId}</p>
          </div>
        )}
      </Show>

      <Show when={props.showUnauthorizedWorkspaceNote}>
        <p class="muted">No authorized workspace/channel selected for operator actions.</p>
      </Show>

      <Show when={props.canAccessActiveChannel}>
        <section class="member-group">
          <p class="group-label">ONLINE ({props.onlineMembers.length})</p>
          <ul>
            <For each={props.onlineMembers}>
              {(memberId) => (
                <li>
                  <span class="presence online" />
                  {props.displayUserLabel(memberId)}
                </li>
              )}
            </For>
            <Show when={props.onlineMembers.length === 0}>
              <li>
                <span class="presence idle" />
                no-presence-yet
              </li>
            </Show>
          </ul>
        </section>
      </Show>

      <section class="member-group">
        <p class="group-label">PANELS</p>
        <div class="ops-launch-grid">
          <button type="button" onClick={() => props.onOpenPanel("public-directory")}>
            Open directory panel
          </button>
          <button type="button" onClick={() => props.onOpenPanel("friendships")}>
            Open friendships panel
          </button>
          <Show when={props.canAccessActiveChannel}>
            <button type="button" onClick={() => props.onOpenPanel("search")}>
              Open search panel
            </button>
            <button type="button" onClick={() => props.onOpenPanel("attachments")}>
              Open attachments panel
            </button>
          </Show>
          <Show when={props.hasModerationAccess}>
            <button type="button" onClick={() => props.onOpenPanel("moderation")}>
              Open moderation panel
            </button>
          </Show>
          <button type="button" onClick={() => props.onOpenPanel("utility")}>
            Open utility panel
          </button>
        </div>
      </section>
    </aside>
  );
}
