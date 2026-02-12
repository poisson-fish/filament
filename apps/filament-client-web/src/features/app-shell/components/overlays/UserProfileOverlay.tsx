import { Show } from "solid-js";
import type { ProfileRecord, UserId } from "../../../../domain/chat";
import { SafeMarkdown } from "../SafeMarkdown";

export interface UserProfileOverlayProps {
  selectedProfileUserId: UserId | null;
  selectedProfileLoading: boolean;
  selectedProfileError: string;
  selectedProfile: ProfileRecord | null;
  avatarUrlForUser: (rawUserId: string) => string | null;
  onClose: () => void;
}

export function UserProfileOverlay(props: UserProfileOverlayProps) {
  return (
    <Show when={props.selectedProfileUserId}>
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
          class="panel-window panel-window-compact profile-view-panel"
          role="dialog"
          aria-modal="true"
          aria-label="User profile panel"
        >
          <header class="panel-window-header">
            <h4>User profile</h4>
            <button type="button" onClick={props.onClose}>
              Close
            </button>
          </header>
          <div class="panel-window-body">
            <Show when={props.selectedProfileLoading}>
              <p class="panel-note">Loading profile...</p>
            </Show>
            <Show when={props.selectedProfileError}>
              <p class="status error">{props.selectedProfileError}</p>
            </Show>
            <Show when={props.selectedProfile}>
              {(profile) => (
                <section class="profile-view-body">
                  <div class="profile-view-header">
                    <span class="profile-view-avatar" aria-hidden="true">
                      <span class="profile-view-avatar-fallback">
                        {profile().username.slice(0, 1).toUpperCase()}
                      </span>
                      <Show when={props.avatarUrlForUser(profile().userId)}>
                        <img
                          class="profile-view-avatar-image"
                          src={props.avatarUrlForUser(profile().userId)!}
                          alt={`${profile().username} avatar`}
                          loading="lazy"
                          decoding="async"
                          referrerPolicy="no-referrer"
                          onError={(event) => {
                            event.currentTarget.style.display = "none";
                          }}
                        />
                      </Show>
                    </span>
                    <div>
                      <p class="profile-view-name">{profile().username}</p>
                      <p class="mono">{profile().userId}</p>
                    </div>
                  </div>
                  <SafeMarkdown class="profile-view-markdown" tokens={profile().aboutMarkdownTokens} />
                </section>
              )}
            </Show>
          </div>
        </section>
      </div>
    </Show>
  );
}
