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
        class="fixed inset-0 z-20 grid place-items-center bg-black/72 p-4 max-[900px]:p-[0.55rem]"
        role="presentation"
        onClick={(event) => {
          if (event.target === event.currentTarget) {
            props.onClose();
          }
        }}
      >
        <section
          class="grid max-h-[min(88vh,50rem)] w-full max-w-[28rem] grid-rows-[auto_1fr] overflow-hidden rounded-[0.9rem] border border-line-soft bg-bg-1 shadow-panel max-[900px]:max-h-[94vh]"
          role="dialog"
          aria-modal="true"
          aria-label="User profile panel"
        >
          <header class="flex items-center justify-between gap-2 border-b border-line px-[0.92rem] py-[0.78rem]">
            <h4 class="m-0 text-[0.98rem] font-[720] text-ink-0">User profile</h4>
            <button
              class="rounded-[0.58rem] border border-line-soft bg-bg-3 px-[0.62rem] py-[0.38rem] text-[0.84rem] text-ink-1 transition-colors enabled:hover:bg-bg-4"
              type="button"
              onClick={props.onClose}
            >
              Close
            </button>
          </header>
          <div class="grid content-start gap-[0.7rem] overflow-auto px-[0.92rem] py-[0.85rem]">
            <Show when={props.selectedProfileLoading}>
              <p class="m-0 text-[0.88rem] text-ink-2">Loading profile...</p>
            </Show>
            <Show when={props.selectedProfileError}>
              <p class="m-0 text-[0.91rem] text-danger">{props.selectedProfileError}</p>
            </Show>
            <Show when={props.selectedProfile}>
              {(profile) => (
                <section class="grid gap-[0.7rem]">
                  <div class="flex items-center gap-[0.68rem]">
                    <span
                      class="relative inline-flex h-[3rem] w-[3rem] shrink-0 items-center justify-center overflow-hidden rounded-full border border-line-soft bg-gradient-to-br from-bg-4 to-bg-3 text-[0.98rem] font-[780] text-ink-0"
                      aria-hidden="true"
                    >
                      <span class="z-[1]">
                        {profile().username.slice(0, 1).toUpperCase()}
                      </span>
                      <Show when={props.avatarUrlForUser(profile().userId)}>
                        <img
                          class="absolute inset-0 z-[2] h-full w-full rounded-full object-cover"
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
                    <div class="grid min-w-0 gap-[0.18rem]">
                      <p class="m-0 text-[1rem] font-[780] text-ink-0">{profile().username}</p>
                      <p class="m-0 break-all font-code text-[0.78rem] text-ink-2">
                        {profile().userId}
                      </p>
                    </div>
                  </div>
                  <SafeMarkdown
                    class="leading-[1.44] text-ink-1 [&_ol]:m-[0.4rem_0_0.4rem_1.15rem] [&_ol]:p-0 [&_p+p]:mt-[0.45rem] [&_p]:m-0 [&_ul]:m-[0.4rem_0_0.4rem_1.15rem] [&_ul]:p-0"
                    tokens={profile().aboutMarkdownTokens}
                  />
                </section>
              )}
            </Show>
          </div>
        </section>
      </div>
    </Show>
  );
}
