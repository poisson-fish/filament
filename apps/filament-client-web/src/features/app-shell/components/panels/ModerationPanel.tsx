import { For, Show } from "solid-js";
import type { PermissionName, RoleName } from "../../../../domain/chat";

interface ChannelOverrideEntity {
  role: RoleName;
  label: string;
  hasExplicitOverride: boolean;
  allow: PermissionName[];
  deny: PermissionName[];
  updatedAtUnix: number | null;
}

export interface ModerationPanelProps {
  moderationUserIdInput: string;
  moderationRoleInput: string;
  overrideRoleInput: string;
  overrideAllowCsv: string;
  overrideDenyCsv: string;
  channelOverrideEntities: ChannelOverrideEntity[];
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
  const selectorSidebarClass =
    "grid gap-[0.38rem] rounded-[0.62rem] border border-line-soft bg-bg-2 p-[0.5rem]";
  const selectorButtonClass =
    "grid gap-[0.1rem] rounded-[0.56rem] border border-line-soft bg-bg-1 px-[0.52rem] py-[0.44rem] text-left text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-3 disabled:cursor-default disabled:opacity-62";
  const statusOkClass = "mt-[0.92rem] text-[0.91rem] text-ok";
  const statusErrorClass = "mt-[0.92rem] text-[0.91rem] text-danger";

  const applyOverrideEntitySelection = (entity: ChannelOverrideEntity): void => {
    props.onOverrideRoleChange(entity.role);
    props.onOverrideAllowInput(entity.allow.join(","));
    props.onOverrideDenyInput(entity.deny.join(","));
  };

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
          <div class="grid gap-[0.55rem] md:grid-cols-[minmax(11rem,0.45fr)_minmax(0,1fr)]">
            <section class={selectorSidebarClass} aria-label="Channel override entities">
              <strong class="text-[0.82rem] uppercase tracking-[0.06em] text-ink-2">Entities</strong>
              <Show
                when={props.channelOverrideEntities.length > 0}
                fallback={<p class="m-0 text-[0.82rem] text-ink-2">No override entities available.</p>}
              >
                <For each={props.channelOverrideEntities}>
                  {(entity) => (
                    <button
                      class={`${selectorButtonClass} ${
                        props.overrideRoleInput === entity.role
                          ? "border-brand bg-bg-3 text-ink-0"
                          : ""
                      }`}
                      type="button"
                      onClick={() => applyOverrideEntitySelection(entity)}
                      aria-pressed={props.overrideRoleInput === entity.role}
                    >
                      <span class="text-[0.85rem]">{entity.label}</span>
                      <span class="text-[0.72rem] text-ink-2">
                        {entity.hasExplicitOverride ? "active override" : "inherits defaults"}
                      </span>
                    </button>
                  )}
                </For>
              </Show>
            </section>
            <div class={formClass}>
              <label class={fieldLabelClass}>
                Override role
                <input
                  class={fieldControlClass}
                  value={props.overrideRoleInput}
                  readonly
                  aria-label="Override role"
                />
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
            </div>
          </div>
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
