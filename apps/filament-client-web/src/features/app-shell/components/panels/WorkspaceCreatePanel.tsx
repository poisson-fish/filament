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
  return (
    <section class="member-group">
      <form class="inline-form" onSubmit={props.onSubmit}>
        <label>
          Workspace name
          <input
            value={props.createGuildName}
            onInput={(event) => props.onCreateGuildNameInput(event.currentTarget.value)}
            maxlength="64"
          />
        </label>
        <label>
          Visibility
          <select
            value={props.createGuildVisibility}
            onChange={(event) => props.onCreateGuildVisibilityChange(event.currentTarget.value)}
          >
            <option value="private">private</option>
            <option value="public">public</option>
          </select>
        </label>
        <label>
          First channel
          <input
            value={props.createChannelName}
            onInput={(event) => props.onCreateChannelNameInput(event.currentTarget.value)}
            maxlength="64"
          />
        </label>
        <label>
          Channel type
          <select
            value={props.createChannelKind}
            onChange={(event) => props.onCreateChannelKindChange(event.currentTarget.value)}
          >
            <option value="text">text</option>
            <option value="voice">voice</option>
          </select>
        </label>
        <div class="button-row">
          <button type="submit" disabled={props.isCreatingWorkspace}>
            {props.isCreatingWorkspace ? "Creating..." : "Create workspace"}
          </button>
          <Show when={props.canDismissWorkspaceCreateForm}>
            <button type="button" onClick={props.onCancel}>
              Cancel
            </button>
          </Show>
        </div>
      </form>
      <Show when={props.workspaceError}>
        <p class="status error">{props.workspaceError}</p>
      </Show>
    </section>
  );
}
