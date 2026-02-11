import { Show } from "solid-js";

export interface UtilityPanelProps {
  echoInput: string;
  healthStatus: string;
  diagError: string;
  isCheckingHealth: boolean;
  isEchoing: boolean;
  onEchoInput: (value: string) => void;
  onRunHealthCheck: () => Promise<void> | void;
  onRunEcho: (event: SubmitEvent) => Promise<void> | void;
}

export function UtilityPanel(props: UtilityPanelProps) {
  return (
    <section class="member-group">
      <div class="button-row">
        <button type="button" onClick={() => void props.onRunHealthCheck()} disabled={props.isCheckingHealth}>
          {props.isCheckingHealth ? "Checking..." : "Health"}
        </button>
      </div>
      <form class="inline-form" onSubmit={props.onRunEcho}>
        <label>
          Echo
          <input
            value={props.echoInput}
            onInput={(event) => props.onEchoInput(event.currentTarget.value)}
            maxlength="128"
          />
        </label>
        <button type="submit" disabled={props.isEchoing}>
          {props.isEchoing ? "Sending..." : "Echo"}
        </button>
      </form>
      <Show when={props.healthStatus}>
        <p class="status ok">{props.healthStatus}</p>
      </Show>
      <Show when={props.diagError}>
        <p class="status error">{props.diagError}</p>
      </Show>
    </section>
  );
}
