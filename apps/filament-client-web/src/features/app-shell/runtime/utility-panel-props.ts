import type { UtilityPanelBuilderOptions } from "../adapters/panel-host-props";
import type { DiagnosticsEventCounts } from "../state/diagnostics-event-counters";

export interface UtilityPanelPropsOptions {
  echoInput: string;
  healthStatus: string;
  diagError: string;
  diagnosticsEventCounts: DiagnosticsEventCounts;
  showDiagnosticsCounters: boolean;
  isCheckingHealth: boolean;
  isEchoing: boolean;
  setEchoInput: (value: string) => void;
  onRunHealthCheck: () => Promise<void> | void;
  onRunEcho: (event: SubmitEvent) => Promise<void> | void;
}

export function createUtilityPanelProps(
  options: UtilityPanelPropsOptions,
): UtilityPanelBuilderOptions {
  return {
    echoInput: options.echoInput,
    healthStatus: options.healthStatus,
    diagError: options.diagError,
    diagnosticsEventCounts: options.diagnosticsEventCounts,
    showDiagnosticsCounters: options.showDiagnosticsCounters,
    isCheckingHealth: options.isCheckingHealth,
    isEchoing: options.isEchoing,
    setEchoInput: options.setEchoInput,
    onRunHealthCheck: options.onRunHealthCheck,
    onRunEcho: (event) => options.onRunEcho(event),
  };
}