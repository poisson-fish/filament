import { Show } from "solid-js";

export interface ModerationPanelProps {
  moderationUserIdInput: string;
  moderationRoleInput: string;
  overrideRoleInput: string;
  overrideAllowCsv: string;
  overrideDenyCsv: string;
  isModerating: boolean;
  hasActiveWorkspace: boolean;
  hasActiveChannel: boolean;
  canManageRoles: boolean;
  canBanMembers: boolean;
  canManageChannelOverrides: boolean;
  moderationStatus: string;
  moderationError: string;
  onModerationUserIdInput: (value: string) => void;
  onModerationRoleChange: (value: string) => void;
  onRunMemberAction: (action: "add" | "role" | "kick" | "ban") => Promise<void> | void;
  onOverrideRoleChange: (value: string) => void;
  onOverrideAllowInput: (value: string) => void;
  onOverrideDenyInput: (value: string) => void;
  onApplyOverride: (event: SubmitEvent) => Promise<void> | void;
  onOpenRoleManagementPanel: () => void;
}

export function ModerationPanel(props: ModerationPanelProps) {
  const panelSectionClass = "grid gap-[0.5rem]";
  const formClass = "grid gap-[0.5rem]";
  const fieldLabelClass = "grid gap-[0.3rem] text-[0.84rem] text-ink-1";
  const fieldControlClass =
    "rounded-[0.56rem] border border-line-soft bg-bg-2 px-[0.55rem] py-[0.62rem] text-ink-0 disabled:cursor-default disabled:opacity-62";
  const buttonRowClass = "flex gap-[0.45rem]";
  const actionButtonClass =
    "min-h-[1.95rem] flex-1 rounded-[0.56rem] border border-line-soft bg-bg-3 px-[0.68rem] py-[0.44rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-4 disabled:cursor-default disabled:opacity-62";
  const submitButtonClass =
    "min-h-[1.95rem] rounded-[0.56rem] border border-line-soft bg-bg-3 px-[0.68rem] py-[0.44rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-4 disabled:cursor-default disabled:opacity-62";
  const statusOkClass = "mt-[0.92rem] text-[0.91rem] text-ok";
  const statusErrorClass = "mt-[0.92rem] text-[0.91rem] text-danger";

  return (
    <section class={panelSectionClass}>
      <form class={formClass}>
        <label class={fieldLabelClass}>
          Target user ULID
          <input
            class={fieldControlClass}
            value={props.moderationUserIdInput}
            onInput={(event) => props.onModerationUserIdInput(event.currentTarget.value)}
            maxlength="26"
            placeholder="01ARZ..."
          />
        </label>
        <label class={fieldLabelClass}>
          Role
          <select
            class={fieldControlClass}
            value={props.moderationRoleInput}
            onChange={(event) => props.onModerationRoleChange(event.currentTarget.value)}
          >
            <option value="member">member</option>
            <option value="moderator">moderator</option>
            <option value="owner">owner</option>
          </select>
        </label>
        <div class={buttonRowClass}>
          <Show when={props.canManageRoles}>
            <button
              class={actionButtonClass}
              type="button"
              disabled={props.isModerating || !props.hasActiveWorkspace}
              onClick={() => void props.onRunMemberAction("add")}
            >
              Add
            </button>
            <button
              class={actionButtonClass}
              type="button"
              disabled={props.isModerating || !props.hasActiveWorkspace}
              onClick={() => void props.onRunMemberAction("role")}
            >
              Set Role
            </button>
          </Show>
          <Show when={props.canBanMembers}>
            <button
              class={actionButtonClass}
              type="button"
              disabled={props.isModerating || !props.hasActiveWorkspace}
              onClick={() => void props.onRunMemberAction("kick")}
            >
              Kick
            </button>
            <button
              class={actionButtonClass}
              type="button"
              disabled={props.isModerating || !props.hasActiveWorkspace}
              onClick={() => void props.onRunMemberAction("ban")}
            >
              Ban
            </button>
          </Show>
        </div>
      </form>
      <Show when={props.canManageChannelOverrides}>
        <form class={formClass} onSubmit={props.onApplyOverride}>
          <label class={fieldLabelClass}>
            Override role
            <select
              class={fieldControlClass}
              value={props.overrideRoleInput}
              onChange={(event) => props.onOverrideRoleChange(event.currentTarget.value)}
            >
              <option value="member">member</option>
              <option value="moderator">moderator</option>
              <option value="owner">owner</option>
            </select>
          </label>
          <label class={fieldLabelClass}>
            Allow permissions (csv)
            <input
              class={fieldControlClass}
              value={props.overrideAllowCsv}
              onInput={(event) => props.onOverrideAllowInput(event.currentTarget.value)}
              placeholder="create_message,subscribe_streams"
            />
          </label>
          <label class={fieldLabelClass}>
            Deny permissions (csv)
            <input
              class={fieldControlClass}
              value={props.overrideDenyCsv}
              onInput={(event) => props.onOverrideDenyInput(event.currentTarget.value)}
              placeholder="delete_message"
            />
          </label>
          <button
            class={submitButtonClass}
            type="submit"
            disabled={props.isModerating || !props.hasActiveChannel}
          >
            Apply channel override
          </button>
        </form>
      </Show>
      <div class={buttonRowClass}>
        <button class={actionButtonClass} type="button" onClick={props.onOpenRoleManagementPanel}>
          Open role management panel
        </button>
      </div>
      <Show when={props.moderationStatus}>
        <p class={statusOkClass}>{props.moderationStatus}</p>
      </Show>
      <Show when={props.moderationError}>
        <p class={statusErrorClass}>{props.moderationError}</p>
      </Show>
    </section>
  );
}
