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
}

export function ModerationPanel(props: ModerationPanelProps) {
  return (
    <section class="member-group">
      <form class="inline-form">
        <label>
          Target user ULID
          <input
            value={props.moderationUserIdInput}
            onInput={(event) => props.onModerationUserIdInput(event.currentTarget.value)}
            maxlength="26"
            placeholder="01ARZ..."
          />
        </label>
        <label>
          Role
          <select
            value={props.moderationRoleInput}
            onChange={(event) => props.onModerationRoleChange(event.currentTarget.value)}
          >
            <option value="member">member</option>
            <option value="moderator">moderator</option>
            <option value="owner">owner</option>
          </select>
        </label>
        <div class="button-row">
          <Show when={props.canManageRoles}>
            <button
              type="button"
              disabled={props.isModerating || !props.hasActiveWorkspace}
              onClick={() => void props.onRunMemberAction("add")}
            >
              Add
            </button>
            <button
              type="button"
              disabled={props.isModerating || !props.hasActiveWorkspace}
              onClick={() => void props.onRunMemberAction("role")}
            >
              Set Role
            </button>
          </Show>
          <Show when={props.canBanMembers}>
            <button
              type="button"
              disabled={props.isModerating || !props.hasActiveWorkspace}
              onClick={() => void props.onRunMemberAction("kick")}
            >
              Kick
            </button>
            <button
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
        <form class="inline-form" onSubmit={props.onApplyOverride}>
          <label>
            Override role
            <select
              value={props.overrideRoleInput}
              onChange={(event) => props.onOverrideRoleChange(event.currentTarget.value)}
            >
              <option value="member">member</option>
              <option value="moderator">moderator</option>
              <option value="owner">owner</option>
            </select>
          </label>
          <label>
            Allow permissions (csv)
            <input
              value={props.overrideAllowCsv}
              onInput={(event) => props.onOverrideAllowInput(event.currentTarget.value)}
              placeholder="create_message,subscribe_streams"
            />
          </label>
          <label>
            Deny permissions (csv)
            <input
              value={props.overrideDenyCsv}
              onInput={(event) => props.onOverrideDenyInput(event.currentTarget.value)}
              placeholder="delete_message"
            />
          </label>
          <button type="submit" disabled={props.isModerating || !props.hasActiveChannel}>
            Apply channel override
          </button>
        </form>
      </Show>
      <Show when={props.moderationStatus}>
        <p class="status ok">{props.moderationStatus}</p>
      </Show>
      <Show when={props.moderationError}>
        <p class="status error">{props.moderationError}</p>
      </Show>
    </section>
  );
}
