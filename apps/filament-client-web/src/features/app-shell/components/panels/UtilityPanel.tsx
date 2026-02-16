import { Show } from "solid-js";
import type { DiagnosticsEventCounts } from "../../state/diagnostics-event-counters";

export interface UtilityPanelProps {
  echoInput: string;
  healthStatus: string;
  diagError: string;
  diagnosticsEventCounts: DiagnosticsEventCounts;
  showDiagnosticsCounters: boolean;
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
      <Show when={props.showDiagnosticsCounters}>
        <div>
          <p class="panel-note">Diagnostics counters (dev only)</p>
          <p class="panel-note">
            Session refresh: {props.diagnosticsEventCounts.session_refresh_succeeded} ok /{" "}
            {props.diagnosticsEventCounts.session_refresh_failed} failed
          </p>
          <p class="panel-note">
            Health checks: {props.diagnosticsEventCounts.health_check_succeeded} ok /{" "}
            {props.diagnosticsEventCounts.health_check_failed} failed
          </p>
          <p class="panel-note">
            Echo: {props.diagnosticsEventCounts.echo_succeeded} ok /{" "}
            {props.diagnosticsEventCounts.echo_failed} failed
          </p>
          <p class="panel-note">
            Logout requests: {props.diagnosticsEventCounts.logout_requested}
          </p>
          <p class="panel-note">
            Gateway connections: {props.diagnosticsEventCounts.gateway_connected} opened /{" "}
            {props.diagnosticsEventCounts.gateway_disconnected} closed
          </p>
        </div>
      </Show>
    </section>
  );
}
