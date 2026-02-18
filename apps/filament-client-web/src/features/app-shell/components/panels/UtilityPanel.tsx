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
  const panelSectionClass = "grid gap-[0.5rem]";
  const buttonRowClass = "flex gap-[0.45rem]";
  const actionButtonClass =
    "min-h-[1.95rem] flex-1 rounded-[0.56rem] border border-line-soft bg-bg-3 px-[0.68rem] py-[0.44rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-4 disabled:cursor-default disabled:opacity-62";
  const formClass = "grid gap-[0.5rem]";
  const fieldLabelClass = "grid gap-[0.3rem] text-[0.84rem] text-ink-1";
  const fieldInputClass =
    "rounded-[0.56rem] border border-line-soft bg-bg-2 px-[0.55rem] py-[0.62rem] text-ink-0 disabled:cursor-default disabled:opacity-62";
  const statusOkClass = "mt-[0.92rem] text-[0.91rem] text-ok";
  const statusErrorClass = "mt-[0.92rem] text-[0.91rem] text-danger";

  return (
    <section class={panelSectionClass}>
      <div class={buttonRowClass}>
        <button
          class={actionButtonClass}
          type="button"
          onClick={() => void props.onRunHealthCheck()}
          disabled={props.isCheckingHealth}
        >
          {props.isCheckingHealth ? "Checking..." : "Health"}
        </button>
      </div>
      <form class={formClass} onSubmit={props.onRunEcho}>
        <label class={fieldLabelClass}>
          Echo
          <input
            class={fieldInputClass}
            value={props.echoInput}
            onInput={(event) => props.onEchoInput(event.currentTarget.value)}
            maxlength="128"
          />
        </label>
        <button class={actionButtonClass} type="submit" disabled={props.isEchoing}>
          {props.isEchoing ? "Sending..." : "Echo"}
        </button>
      </form>
      <Show when={props.healthStatus}>
        <p class={statusOkClass}>{props.healthStatus}</p>
      </Show>
      <Show when={props.diagError}>
        <p class={statusErrorClass}>{props.diagError}</p>
      </Show>
      <Show when={props.showDiagnosticsCounters}>
        <div class="grid gap-[0.5rem]">
          <p class="m-0">Diagnostics counters (dev only)</p>
          <p class="m-0">
            Session refresh: {props.diagnosticsEventCounts.session_refresh_succeeded} ok /{" "}
            {props.diagnosticsEventCounts.session_refresh_failed} failed
          </p>
          <p class="m-0">
            Health checks: {props.diagnosticsEventCounts.health_check_succeeded} ok /{" "}
            {props.diagnosticsEventCounts.health_check_failed} failed
          </p>
          <p class="m-0">
            Echo: {props.diagnosticsEventCounts.echo_succeeded} ok /{" "}
            {props.diagnosticsEventCounts.echo_failed} failed
          </p>
          <p class="m-0">
            Logout requests: {props.diagnosticsEventCounts.logout_requested}
          </p>
          <p class="m-0">
            Gateway connections: {props.diagnosticsEventCounts.gateway_connected} opened /{" "}
            {props.diagnosticsEventCounts.gateway_disconnected} closed
          </p>
        </div>
      </Show>
    </section>
  );
}
