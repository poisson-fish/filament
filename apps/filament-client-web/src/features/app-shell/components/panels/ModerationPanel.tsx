import { For, Show, createMemo } from "solid-js";
import type { PermissionName, RoleName } from "../../../../domain/chat";
import { KNOWN_PERMISSIONS } from "../../permissions/effective-permissions";
import { parsePermissionCsv } from "../../helpers";

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
  channelOverrideEffectivePermissions: Record<RoleName, PermissionName[]>;
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

interface PermissionOverrideEntry {
  permission: PermissionName;
  label: string;
  summary: string;
}

type TriStateOverride = "inherit" | "allow" | "deny";

const CHANNEL_OVERRIDE_PERMISSION_MATRIX: readonly PermissionOverrideEntry[] = [
  {
    permission: "create_message",
    label: "Create Messages",
    summary: "Send messages in the selected channel.",
  },
  {
    permission: "delete_message",
    label: "Delete Messages",
    summary: "Delete messages authored by other members.",
  },
  {
    permission: "manage_channel_overrides",
    label: "Manage Overrides",
    summary: "Edit channel override rules for roles and members.",
  },
  {
    permission: "ban_member",
    label: "Ban Members",
    summary: "Kick or ban members from the workspace.",
  },
  {
    permission: "manage_member_roles",
    label: "Manage Member Roles",
    summary: "Assign and remove workspace roles for members.",
  },
  {
    permission: "manage_workspace_roles",
    label: "Manage Workspace Roles",
    summary: "Create, update, delete, and reorder workspace roles.",
  },
  {
    permission: "view_audit_log",
    label: "View Audit Log",
    summary: "View workspace moderation audit history.",
  },
  {
    permission: "manage_ip_bans",
    label: "Manage IP Bans",
    summary: "Apply and remove guild IP ban entries.",
  },
  {
    permission: "publish_video",
    label: "Publish Camera",
    summary: "Publish camera tracks in voice channels.",
  },
  {
    permission: "publish_screen_share",
    label: "Publish Screen",
    summary: "Publish screen-share tracks in voice channels.",
  },
  {
    permission: "subscribe_streams",
    label: "Subscribe Streams",
    summary: "Receive remote media streams in voice channels.",
  },
  {
    permission: "manage_roles",
    label: "Legacy Manage Roles",
    summary: "Compatibility permission for pre-role-system flows.",
  },
];

function parseOverrideDraft(csv: string): PermissionName[] {
  try {
    return parsePermissionCsv(csv);
  } catch {
    return [];
  }
}

function serializePermissionSet(set: ReadonlySet<PermissionName>): string {
  const ordered = KNOWN_PERMISSIONS.filter((permission) => set.has(permission));
  return ordered.join(",");
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
  const triStateGroupClass = "grid gap-[0.3rem] rounded-[0.56rem] border border-line-soft bg-bg-1 p-[0.35rem]";
  const triStateButtonClass =
    "rounded-[0.5rem] border border-line-soft bg-bg-2 px-[0.5rem] py-[0.34rem] text-[0.74rem] text-ink-2 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-3";
  const statusOkClass = "mt-[0.92rem] text-[0.91rem] text-ok";
  const statusErrorClass = "mt-[0.92rem] text-[0.91rem] text-danger";

  const overrideDraft = createMemo(() => {
    const allow = new Set(parseOverrideDraft(props.overrideAllowCsv));
    const deny = new Set(
      parseOverrideDraft(props.overrideDenyCsv).filter((permission) => !allow.has(permission)),
    );
    return { allow, deny };
  });
  const selectedOverrideRoleEffectivePermissions = createMemo(() => {
    const role = props.overrideRoleInput as RoleName;
    return new Set(props.channelOverrideEffectivePermissions[role] ?? []);
  });

  const applyOverrideEntitySelection = (entity: ChannelOverrideEntity): void => {
    props.onOverrideRoleChange(entity.role);
    props.onOverrideAllowInput(entity.allow.join(","));
    props.onOverrideDenyInput(entity.deny.join(","));
  };

  const overrideStateForPermission = (permission: PermissionName): TriStateOverride => {
    const draft = overrideDraft();
    if (draft.allow.has(permission)) {
      return "allow";
    }
    if (draft.deny.has(permission)) {
      return "deny";
    }
    return "inherit";
  };

  const updatePermissionOverride = (
    permission: PermissionName,
    nextState: TriStateOverride,
  ): void => {
    const draft = overrideDraft();
    const allow = new Set(draft.allow);
    const deny = new Set(draft.deny);

    allow.delete(permission);
    deny.delete(permission);
    if (nextState === "allow") {
      allow.add(permission);
    } else if (nextState === "deny") {
      deny.add(permission);
    }

    props.onOverrideAllowInput(serializePermissionSet(allow));
    props.onOverrideDenyInput(serializePermissionSet(deny));
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
              <section class="grid gap-[0.42rem]" aria-label="Channel permission matrix">
                <strong class="text-[0.82rem] uppercase tracking-[0.06em] text-ink-2">
                  Permission overrides
                </strong>
                <For each={CHANNEL_OVERRIDE_PERMISSION_MATRIX}>
                  {(entry) => {
                    const isActive = (state: TriStateOverride) =>
                      overrideStateForPermission(entry.permission) === state;
                    const isEffectivelyAllowed = () =>
                      selectedOverrideRoleEffectivePermissions().has(entry.permission);
                    return (
                      <article class="grid gap-[0.3rem] rounded-[0.56rem] border border-line-soft bg-bg-2 p-[0.44rem]">
                        <div class="grid gap-[0.14rem]">
                          <h4 class="m-0 text-[0.83rem] font-semibold text-ink-0">
                            {entry.label}
                          </h4>
                          <p class="m-0 text-[0.73rem] text-ink-2">{entry.summary}</p>
                          <p
                            class={`m-0 text-[0.72rem] ${
                              isEffectivelyAllowed() ? "text-ok" : "text-danger"
                            }`}
                            aria-label={`${entry.label} effective ${
                              isEffectivelyAllowed() ? "allowed" : "denied"
                            }`}
                          >
                            Effective: {isEffectivelyAllowed() ? "Allowed" : "Denied"}
                          </p>
                        </div>
                        <div class={triStateGroupClass} role="radiogroup" aria-label={entry.label}>
                          <div class="grid grid-cols-3 gap-[0.3rem]">
                            <button
                              class={`${triStateButtonClass} ${
                                isActive("inherit") ? "border-brand bg-bg-3 text-ink-0" : ""
                              }`}
                              type="button"
                              role="radio"
                              aria-checked={isActive("inherit")}
                              aria-label={`${entry.label}: Inherit`}
                              onClick={() => updatePermissionOverride(entry.permission, "inherit")}
                            >
                              /
                            </button>
                            <button
                              class={`${triStateButtonClass} ${
                                isActive("allow") ? "border-ok bg-bg-3 text-ink-0" : ""
                              }`}
                              type="button"
                              role="radio"
                              aria-checked={isActive("allow")}
                              aria-label={`${entry.label}: Allow`}
                              onClick={() => updatePermissionOverride(entry.permission, "allow")}
                            >
                              ✓
                            </button>
                            <button
                              class={`${triStateButtonClass} ${
                                isActive("deny") ? "border-danger bg-bg-3 text-ink-0" : ""
                              }`}
                              type="button"
                              role="radio"
                              aria-checked={isActive("deny")}
                              aria-label={`${entry.label}: Deny`}
                              onClick={() => updatePermissionOverride(entry.permission, "deny")}
                            >
                              X
                            </button>
                          </div>
                        </div>
                      </article>
                    );
                  }}
                </For>
              </section>
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
