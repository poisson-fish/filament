import { Show } from "solid-js";

export interface ChannelCreatePanelProps {
  newChannelName: string;
  newChannelKind: string;
  isCreatingChannel: boolean;
  channelCreateError: string;
  onSubmit: (event: SubmitEvent) => Promise<void> | void;
  onNewChannelNameInput: (value: string) => void;
  onNewChannelKindChange: (value: string) => void;
  onCancel: () => void;
}

export function ChannelCreatePanel(props: ChannelCreatePanelProps) {
  const panelSectionClass = "grid gap-[0.5rem]";
  const formClass = "grid gap-[0.5rem]";
  const fieldLabelClass = "grid gap-[0.3rem] text-[0.84rem] text-ink-1";
  const fieldInputClass =
    "rounded-[0.56rem] border border-line-soft bg-bg-2 px-[0.55rem] py-[0.62rem] text-ink-0";
  const buttonRowClass = "flex gap-[0.45rem]";
  const actionButtonClass =
    "min-h-[1.95rem] flex-1 rounded-[0.56rem] border border-line-soft bg-bg-3 px-[0.68rem] py-[0.44rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-4 disabled:cursor-default disabled:opacity-62";
  const statusErrorClass = "mt-[0.92rem] text-[0.91rem] text-danger";

  return (
    <section class={panelSectionClass}>
      <form class={formClass} onSubmit={props.onSubmit}>
        <label class={fieldLabelClass}>
          Channel name
          <input
            class={fieldInputClass}
            value={props.newChannelName}
            onInput={(event) => props.onNewChannelNameInput(event.currentTarget.value)}
            maxlength="64"
          />
        </label>
        <label class={fieldLabelClass}>
          Channel type
          <select
            class={fieldInputClass}
            value={props.newChannelKind}
            onChange={(event) => props.onNewChannelKindChange(event.currentTarget.value)}
          >
            <option value="text">text</option>
            <option value="voice">voice</option>
          </select>
        </label>
        <div class={buttonRowClass}>
          <button class={actionButtonClass} type="submit" disabled={props.isCreatingChannel}>
            {props.isCreatingChannel ? "Creating..." : "Create channel"}
          </button>
          <button class={actionButtonClass} type="button" onClick={props.onCancel}>
            Cancel
          </button>
        </div>
      </form>
      <Show when={props.channelCreateError}>
        <p class={statusErrorClass}>{props.channelCreateError}</p>
      </Show>
    </section>
  );
}
