import { Show } from "solid-js";
import type { GuildVisibility } from "../../../../domain/chat";

export interface WorkspaceSettingsPanelProps {
  hasActiveWorkspace: boolean;
  canManageWorkspaceSettings: boolean;
  workspaceName: string;
  workspaceVisibility: GuildVisibility;
  isSavingWorkspaceSettings: boolean;
  workspaceSettingsStatus: string;
  workspaceSettingsError: string;
  onWorkspaceNameInput: (value: string) => void;
  onWorkspaceVisibilityChange: (value: GuildVisibility) => void;
  onSaveWorkspaceSettings: () => Promise<void> | void;
}

export function WorkspaceSettingsPanel(props: WorkspaceSettingsPanelProps) {
  const sectionLabelClassName =
    "m-0 text-[0.68rem] uppercase tracking-[0.08em] text-ink-2";

  return (
    <section aria-label="workspace settings">
      <p class={sectionLabelClassName}>WORKSPACE</p>
      <Show when={props.hasActiveWorkspace} fallback={<p class="muted">No active workspace selected.</p>}>
        <form
          class="inline-form"
          onSubmit={(event) => {
            event.preventDefault();
            void props.onSaveWorkspaceSettings();
          }}
        >
          <label>
            Workspace name
            <input
              aria-label="Workspace settings name"
              value={props.workspaceName}
              maxlength="64"
              onInput={(event) => props.onWorkspaceNameInput(event.currentTarget.value)}
              disabled={props.isSavingWorkspaceSettings || !props.canManageWorkspaceSettings}
            />
          </label>
          <label>
            Visibility
            <select
              aria-label="Workspace settings visibility"
              value={props.workspaceVisibility}
              onChange={(event) =>
                props.onWorkspaceVisibilityChange(
                  event.currentTarget.value === "public" ? "public" : "private",
                )}
              disabled={props.isSavingWorkspaceSettings || !props.canManageWorkspaceSettings}
            >
              <option value="private">private</option>
              <option value="public">public</option>
            </select>
          </label>
          <button type="submit" disabled={props.isSavingWorkspaceSettings || !props.canManageWorkspaceSettings}>
            {props.isSavingWorkspaceSettings ? "Saving..." : "Save workspace"}
          </button>
        </form>
        <Show when={!props.canManageWorkspaceSettings}>
          <p class="muted">You need workspace role-management permissions to update these settings.</p>
        </Show>
        <Show when={props.workspaceSettingsStatus}>
          <p class="status ok">{props.workspaceSettingsStatus}</p>
        </Show>
        <Show when={props.workspaceSettingsError}>
          <p class="status error">{props.workspaceSettingsError}</p>
        </Show>
      </Show>
    </section>
  );
}
