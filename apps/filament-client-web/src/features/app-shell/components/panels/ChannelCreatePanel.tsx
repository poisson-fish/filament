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
  return (
    <section class="member-group">
      <form class="inline-form" onSubmit={props.onSubmit}>
        <label>
          Channel name
          <input
            value={props.newChannelName}
            onInput={(event) => props.onNewChannelNameInput(event.currentTarget.value)}
            maxlength="64"
          />
        </label>
        <label>
          Channel type
          <select
            value={props.newChannelKind}
            onChange={(event) => props.onNewChannelKindChange(event.currentTarget.value)}
          >
            <option value="text">text</option>
            <option value="voice">voice</option>
          </select>
        </label>
        <div class="button-row">
          <button type="submit" disabled={props.isCreatingChannel}>
            {props.isCreatingChannel ? "Creating..." : "Create channel"}
          </button>
          <button type="button" onClick={props.onCancel}>
            Cancel
          </button>
        </div>
      </form>
      <Show when={props.channelCreateError}>
        <p class="status error">{props.channelCreateError}</p>
      </Show>
    </section>
  );
}
