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
  const panelSectionClass =
    "grid gap-[0.64rem] rounded-[0.78rem] border border-line bg-bg-2 p-[0.82rem]";
  const sectionLabelClassName =
    "m-0 text-[0.68rem] uppercase tracking-[0.08em] text-ink-2";
  const mutedTextClass = "m-0 text-[0.91rem] text-ink-2";
  const formClass = "grid gap-[0.55rem]";
  const fieldLabelClass = "grid gap-[0.3rem] text-[0.84rem] text-ink-1";
  const fieldControlClass =
    "rounded-[0.62rem] border border-line-soft bg-bg-1 px-[0.6rem] py-[0.62rem] text-ink-0 disabled:cursor-default disabled:opacity-62";
  const submitButtonClass =
    "min-h-[2rem] rounded-[0.62rem] border border-line-soft bg-bg-3 px-[0.72rem] py-[0.46rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-4 disabled:cursor-default disabled:opacity-62";
  const statusOkClass = "mt-[0.92rem] text-[0.91rem] text-ok";
  const statusErrorClass = "mt-[0.92rem] text-[0.91rem] text-danger";

  return (
    <section class={panelSectionClass} aria-label="workspace settings">
      <p class={sectionLabelClassName}>WORKSPACE</p>
      <Show
        when={props.hasActiveWorkspace}
        fallback={<p class={mutedTextClass}>No active workspace selected.</p>}
      >
        <form
          class={formClass}
          onSubmit={(event) => {
            event.preventDefault();
            void props.onSaveWorkspaceSettings();
          }}
        >
          <label class={fieldLabelClass}>
            Workspace name
            <input
              class={fieldControlClass}
              aria-label="Workspace settings name"
              value={props.workspaceName}
              maxlength="64"
              onInput={(event) => props.onWorkspaceNameInput(event.currentTarget.value)}
              disabled={props.isSavingWorkspaceSettings || !props.canManageWorkspaceSettings}
            />
          </label>
          <label class={fieldLabelClass}>
            Visibility
            <select
              class={fieldControlClass}
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
          <button
            class={submitButtonClass}
            type="submit"
            disabled={props.isSavingWorkspaceSettings || !props.canManageWorkspaceSettings}
          >
            {props.isSavingWorkspaceSettings ? "Saving..." : "Save workspace"}
          </button>
        </form>
        <Show when={!props.canManageWorkspaceSettings}>
          <p class={mutedTextClass}>You need workspace role-management permissions to update these settings.</p>
        </Show>
        <Show when={props.workspaceSettingsStatus}>
          <p class={statusOkClass}>{props.workspaceSettingsStatus}</p>
        </Show>
        <Show when={props.workspaceSettingsError}>
          <p class={statusErrorClass}>{props.workspaceSettingsError}</p>
        </Show>
      </Show>
    </section>
  );
}
