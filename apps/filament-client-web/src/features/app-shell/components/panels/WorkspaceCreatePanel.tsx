import { Show } from "solid-js";

export interface WorkspaceCreatePanelProps {
  createGuildName: string;
  createGuildVisibility: string;
  createChannelName: string;
  createChannelKind: string;
  isCreatingWorkspace: boolean;
  canDismissWorkspaceCreateForm: boolean;
  workspaceError: string;
  onSubmit: (event: SubmitEvent) => Promise<void> | void;
  onCreateGuildNameInput: (value: string) => void;
  onCreateGuildVisibilityChange: (value: string) => void;
  onCreateChannelNameInput: (value: string) => void;
  onCreateChannelKindChange: (value: string) => void;
  onCancel: () => void;
}

export function WorkspaceCreatePanel(props: WorkspaceCreatePanelProps) {
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
          Workspace name
          <input
            class={fieldInputClass}
            value={props.createGuildName}
            onInput={(event) => props.onCreateGuildNameInput(event.currentTarget.value)}
            maxlength="64"
          />
        </label>
        <label class={fieldLabelClass}>
          Visibility
          <select
            class={fieldInputClass}
            value={props.createGuildVisibility}
            onChange={(event) => props.onCreateGuildVisibilityChange(event.currentTarget.value)}
          >
            <option value="private">private</option>
            <option value="public">public</option>
          </select>
        </label>
        <label class={fieldLabelClass}>
          First channel
          <input
            class={fieldInputClass}
            value={props.createChannelName}
            onInput={(event) => props.onCreateChannelNameInput(event.currentTarget.value)}
            maxlength="64"
          />
        </label>
        <label class={fieldLabelClass}>
          Channel type
          <select
            class={fieldInputClass}
            value={props.createChannelKind}
            onChange={(event) => props.onCreateChannelKindChange(event.currentTarget.value)}
          >
            <option value="text">text</option>
            <option value="voice">voice</option>
          </select>
        </label>
        <div class={buttonRowClass}>
          <button class={actionButtonClass} type="submit" disabled={props.isCreatingWorkspace}>
            {props.isCreatingWorkspace ? "Creating..." : "Create workspace"}
          </button>
          <Show when={props.canDismissWorkspaceCreateForm}>
            <button class={actionButtonClass} type="button" onClick={props.onCancel}>
              Cancel
            </button>
          </Show>
        </div>
      </form>
      <Show when={props.workspaceError}>
        <p class={statusErrorClass}>{props.workspaceError}</p>
      </Show>
    </section>
  );
}
