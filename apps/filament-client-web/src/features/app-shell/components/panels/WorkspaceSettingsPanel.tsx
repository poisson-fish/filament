import { For, Show, createEffect, createMemo, createSignal } from "solid-js";
import type {
  GuildRoleRecord,
  GuildVisibility,
  WorkspaceRoleId,
} from "../../../../domain/chat";

export interface WorkspaceSettingsMemberRecord {
  userId: string;
  label: string;
  roleIds: WorkspaceRoleId[];
}

export interface WorkspaceSettingsPanelProps {
  hasActiveWorkspace: boolean;
  canManageWorkspaceSettings: boolean;
  canManageMemberRoles: boolean;
  workspaceName: string;
  workspaceVisibility: GuildVisibility;
  isSavingWorkspaceSettings: boolean;
  workspaceSettingsStatus: string;
  workspaceSettingsError: string;
  memberRoleStatus: string;
  memberRoleError: string;
  isMutatingMemberRoles: boolean;
  members: WorkspaceSettingsMemberRecord[];
  roles: GuildRoleRecord[];
  assignableRoleIds: WorkspaceRoleId[];
  onWorkspaceNameInput: (value: string) => void;
  onWorkspaceVisibilityChange: (value: GuildVisibility) => void;
  onSaveWorkspaceSettings: () => Promise<void> | void;
  onAssignMemberRole: (userId: string, roleId: WorkspaceRoleId) => Promise<void> | void;
  onUnassignMemberRole: (userId: string, roleId: WorkspaceRoleId) => Promise<void> | void;
}

export function WorkspaceSettingsPanel(props: WorkspaceSettingsPanelProps) {
  const [memberRoleDraftByUserId, setMemberRoleDraftByUserId] = createSignal<
    Record<string, WorkspaceRoleId | "">
  >({});
  const [memberSearchQuery, setMemberSearchQuery] = createSignal("");
  const [memberRoleClientError, setMemberRoleClientError] = createSignal("");

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
  const memberListClass = "m-0 grid list-none gap-[0.46rem] p-0";
  const memberRowClass =
    "grid gap-[0.45rem] rounded-[0.62rem] border border-line-soft bg-bg-1 p-[0.56rem]";
  const badgeClass =
    "inline-flex items-center gap-[0.32rem] rounded-[99px] border border-line-soft bg-bg-2 px-[0.5rem] py-[0.2rem] text-[0.76rem] text-ink-1";
  const miniButtonClass =
    "rounded-[0.45rem] border border-line-soft bg-bg-2 px-[0.5rem] py-[0.34rem] text-[0.75rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-3 disabled:cursor-default disabled:opacity-62";
  const statusOkClass = "mt-[0.92rem] text-[0.91rem] text-ok";
  const statusErrorClass = "mt-[0.92rem] text-[0.91rem] text-danger";

  const roleById = createMemo(() => {
    const next = new Map<WorkspaceRoleId, GuildRoleRecord>();
    for (const role of props.roles) {
      next.set(role.roleId, role);
    }
    return next;
  });
  const assignableRoleIdSet = createMemo(
    () => new Set<WorkspaceRoleId>(props.assignableRoleIds),
  );
  const assignableRoles = createMemo(() =>
    props.roles.filter((role) => assignableRoleIdSet().has(role.roleId)),
  );
  const filteredMembers = createMemo(() => {
    const query = memberSearchQuery().trim().toLowerCase();
    if (!query) {
      return props.members;
    }
    return props.members.filter((member) =>
      member.label.toLowerCase().includes(query) || member.userId.toLowerCase().includes(query),
    );
  });

  const resolveDraftRoleIdForMember = (userId: string): WorkspaceRoleId | "" => {
    const current = memberRoleDraftByUserId()[userId];
    if (typeof current === "string" && assignableRoleIdSet().has(current as WorkspaceRoleId)) {
      return current as WorkspaceRoleId;
    }
    const fallback = assignableRoles()[0]?.roleId;
    return fallback ?? "";
  };

  createEffect(() => {
    const draft = memberRoleDraftByUserId();
    const next: Record<string, WorkspaceRoleId | ""> = {};
    let changed = false;
    for (const member of props.members) {
      const current = draft[member.userId];
      if (typeof current === "string" && assignableRoleIdSet().has(current as WorkspaceRoleId)) {
        next[member.userId] = current;
      } else {
        next[member.userId] = resolveDraftRoleIdForMember(member.userId);
        changed = true;
      }
    }
    if (Object.keys(draft).length !== Object.keys(next).length) {
      changed = true;
    }
    if (changed) {
      setMemberRoleDraftByUserId(next);
    }
  });

  const onAssignRole = async (userId: string): Promise<void> => {
    if (!props.hasActiveWorkspace || !props.canManageMemberRoles) {
      return;
    }
    const selectedRoleId = resolveDraftRoleIdForMember(userId);
    if (!selectedRoleId) {
      setMemberRoleClientError("No assignable roles available for your current hierarchy.");
      return;
    }
    setMemberRoleClientError("");
    await props.onAssignMemberRole(userId, selectedRoleId);
  };

  const onUnassignRole = async (userId: string, roleId: WorkspaceRoleId): Promise<void> => {
    if (!props.hasActiveWorkspace || !props.canManageMemberRoles) {
      return;
    }
    if (!assignableRoleIdSet().has(roleId)) {
      setMemberRoleClientError("Role hierarchy blocks removing this assignment.");
      return;
    }
    setMemberRoleClientError("");
    await props.onUnassignMemberRole(userId, roleId);
  };

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
      <Show when={props.hasActiveWorkspace}>
        <section class={panelSectionClass} aria-label="workspace members settings">
          <p class={sectionLabelClassName}>MEMBERS</p>
          <label class={fieldLabelClass}>
            Search members
            <input
              class={fieldControlClass}
              aria-label="Workspace members search"
              value={memberSearchQuery()}
              maxlength="64"
              onInput={(event) => setMemberSearchQuery(event.currentTarget.value)}
            />
          </label>
          <Show when={props.canManageMemberRoles} fallback={
            <p class={mutedTextClass}>
              You need member-role permissions to edit assignments.
            </p>
          }>
            <Show
              when={filteredMembers().length > 0}
              fallback={
                <p class={mutedTextClass}>
                  No known members yet. Member rows appear after presence or role events are received.
                </p>
              }
            >
              <ul class={memberListClass}>
                <For each={filteredMembers()}>
                  {(member) => (
                    <li class={memberRowClass}>
                      <div class="grid gap-[0.14rem]">
                        <strong class="break-words text-[0.88rem] text-ink-0">{member.label}</strong>
                        <code class="break-all text-[0.74rem] text-ink-2">{member.userId}</code>
                      </div>
                      <div class="flex flex-wrap gap-[0.32rem]" aria-label={`Assigned roles for ${member.label}`}>
                        <Show
                          when={member.roleIds.length > 0}
                          fallback={<span class={badgeClass}>No custom roles</span>}
                        >
                          <For each={member.roleIds}>
                            {(roleId) => {
                              const role = roleById().get(roleId);
                              return (
                                <span class={badgeClass}>
                                  {role?.name ?? roleId}
                                  <button
                                    class={miniButtonClass}
                                    type="button"
                                    onClick={() => void onUnassignRole(member.userId, roleId)}
                                    disabled={
                                      props.isMutatingMemberRoles || !assignableRoleIdSet().has(roleId)
                                    }
                                    aria-label={`Unassign ${role?.name ?? roleId} from ${member.label}`}
                                  >
                                    Remove
                                  </button>
                                </span>
                              );
                            }}
                          </For>
                        </Show>
                      </div>
                      <div class="grid gap-[0.4rem] sm:grid-cols-[minmax(0,1fr)_auto] sm:items-end">
                        <label class={fieldLabelClass}>
                          Add role
                          <select
                            class={fieldControlClass}
                            aria-label={`Role assignment for ${member.label}`}
                            value={resolveDraftRoleIdForMember(member.userId)}
                            onChange={(event) => {
                              const nextRoleId = event.currentTarget.value as WorkspaceRoleId;
                              setMemberRoleDraftByUserId((existing) => ({
                                ...existing,
                                [member.userId]: nextRoleId,
                              }));
                              setMemberRoleClientError("");
                            }}
                            disabled={props.isMutatingMemberRoles || assignableRoles().length === 0}
                          >
                            <For each={assignableRoles()}>
                              {(role) => <option value={role.roleId}>{role.name}</option>}
                            </For>
                          </select>
                        </label>
                        <button
                          class={submitButtonClass}
                          type="button"
                          onClick={() => void onAssignRole(member.userId)}
                          disabled={props.isMutatingMemberRoles || assignableRoles().length === 0}
                        >
                          {props.isMutatingMemberRoles ? "Applying..." : "Assign role"}
                        </button>
                      </div>
                    </li>
                  )}
                </For>
              </ul>
            </Show>
          </Show>
          <Show when={props.memberRoleStatus}>
            <p class={statusOkClass}>{props.memberRoleStatus}</p>
          </Show>
          <Show when={props.memberRoleError || memberRoleClientError()}>
            <p class={statusErrorClass}>{props.memberRoleError || memberRoleClientError()}</p>
          </Show>
        </section>
      </Show>
    </section>
  );
}
