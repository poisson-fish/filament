import { For, Show, createMemo } from "solid-js";
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

type FloatingAlertTone = "info" | "ok" | "error";

interface FloatingAlert {
  id: string;
  message: string;
  tone: FloatingAlertTone;
}

export function ChatColumn(props: ChatColumnProps) {
  const floatingAlerts = createMemo<FloatingAlert[]>(() => {
    const alerts: FloatingAlert[] = [];
    if (!props.workspaceBootstrapDone) {
      alerts.push({
        id: "workspace-validating",
        message: "Validating workspace access...",
        tone: "info",
      });
      return alerts;
    }
    if (props.isLoadingMessages) {
      alerts.push({
        id: "messages-loading",
        message: "Loading messages...",
        tone: "info",
      });
    }
    if (props.messageError) {
      alerts.push({
        id: "messages-error",
        message: props.messageError,
        tone: "error",
      });
    }
    if (props.sessionStatus) {
      alerts.push({
        id: "session-status",
        message: props.sessionStatus,
        tone: "ok",
      });
    }
    if (props.sessionError) {
      alerts.push({
        id: "session-error",
        message: props.sessionError,
        tone: "error",
      });
    }
    if (props.voiceStatus && (props.canShowVoiceHeaderControls || props.isVoiceSessionActive)) {
      alerts.push({
        id: "voice-status",
        message: props.voiceStatus,
        tone: "ok",
      });
    }
    if (props.voiceError && (props.canShowVoiceHeaderControls || props.isVoiceSessionActive)) {
      alerts.push({
        id: "voice-error",
        message: props.voiceError,
        tone: "error",
      });
    }
    if (props.activeChannel && !props.canAccessActiveChannel) {
      alerts.push({
        id: "channel-visibility",
        message: "Channel is not visible with your current default permissions.",
        tone: "error",
      });
    }
    if (props.messageStatus) {
      alerts.push({
        id: "message-status",
        message: props.messageStatus,
        tone: "ok",
      });
    }
    return alerts;
  });

  return (
    <main class="chat-panel">
      {props.chatHeader}

      <Show
        when={props.workspaceBootstrapDone && props.workspaceCount === 0}
        fallback={(
          <>
            <Show when={floatingAlerts().length > 0}>
              <div class="pointer-events-none fixed right-[0.9rem] top-[4.4rem] z-[70] grid max-w-[28rem] gap-[0.5rem] max-sm:left-[0.9rem] max-sm:right-[0.9rem]">
                <For each={floatingAlerts()}>
                  {(alert) => (
                    <p
                      class="m-0 rounded-[0.72rem] border px-[0.78rem] py-[0.64rem] text-[0.83rem] leading-snug shadow-panel backdrop-blur-[4px]"
                      classList={{
                        "border-line-soft bg-bg-2/96 text-ink-1": alert.tone === "info",
                        "border-ok/40 bg-ok-soft/92 text-ok": alert.tone === "ok",
                        "border-danger/45 bg-danger-soft/92 text-danger": alert.tone === "error",
                      }}
                      role={alert.tone === "error" ? "alert" : "status"}
                      aria-live={alert.tone === "error" ? "assertive" : "polite"}
                      aria-atomic="true"
                    >
                      {alert.message}
                    </p>
                  )}
                </For>
              </div>
            </Show>
            <section class="chat-body">
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
