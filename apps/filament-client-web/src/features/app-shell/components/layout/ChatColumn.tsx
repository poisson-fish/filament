import { Show } from "solid-js";
import type { JSX } from "solid-js";
import type { ChannelRecord } from "../../../../domain/chat";

export interface ChatColumnProps {
  chatHeader: JSX.Element;
  workspaceBootstrapDone: boolean;
  workspaceCount: number;
  isLoadingMessages: boolean;
  messageError: string;
  sessionStatus: string;
  sessionError: string;
  voiceStatus: string;
  voiceError: string;
  canShowVoiceHeaderControls: boolean;
  isVoiceSessionActive: boolean;
  activeChannel: ChannelRecord | null;
  canAccessActiveChannel: boolean;
  messageList: JSX.Element;
  messageComposer: JSX.Element;
  reactionPicker: JSX.Element;
  messageStatus: string;
}

export function ChatColumn(props: ChatColumnProps) {
  return (
    <main class="chat-panel">
      {props.chatHeader}

      <Show
        when={props.workspaceBootstrapDone && props.workspaceCount === 0}
        fallback={(
          <>
            <section class="chat-body">
              <div class="chat-transient-notes">
                <Show when={!props.workspaceBootstrapDone}>
                  <p class="m-[0.5rem_1rem_0]">Validating workspace access...</p>
                </Show>
                <Show when={props.workspaceBootstrapDone}>
                  <Show when={props.isLoadingMessages}>
                    <p class="m-[0.5rem_1rem_0]">Loading messages...</p>
                  </Show>
                  <Show when={props.messageError}>
                    <p class="status error m-[0.5rem_1rem_0]">{props.messageError}</p>
                  </Show>
                  <Show when={props.sessionStatus}>
                    <p class="status ok m-[0.5rem_1rem_0]">{props.sessionStatus}</p>
                  </Show>
                  <Show when={props.sessionError}>
                    <p class="status error m-[0.5rem_1rem_0]">{props.sessionError}</p>
                  </Show>
                  <Show when={props.voiceStatus && (props.canShowVoiceHeaderControls || props.isVoiceSessionActive)}>
                    <p class="status ok m-[0.5rem_1rem_0]">{props.voiceStatus}</p>
                  </Show>
                  <Show when={props.voiceError && (props.canShowVoiceHeaderControls || props.isVoiceSessionActive)}>
                    <p class="status error m-[0.5rem_1rem_0]">{props.voiceError}</p>
                  </Show>
                  <Show when={props.activeChannel && !props.canAccessActiveChannel}>
                    <p class="status error m-[0.5rem_1rem_0]">
                      Channel is not visible with your current default permissions.
                    </p>
                  </Show>
                  <Show when={props.messageStatus}>
                    <p class="status ok m-[0.5rem_1rem_0]">{props.messageStatus}</p>
                  </Show>
                </Show>
              </div>

              {props.messageList}
            </section>

            {props.messageComposer}
          </>
        )}
      >
        <section class="grid gap-[0.72rem] p-[1rem]">
          <h3>Create your first workspace</h3>
          <p class="muted">Use the + button in the workspace rail to create your first guild and channel.</p>
        </section>
      </Show>

      {props.reactionPicker}
    </main>
  );
}
