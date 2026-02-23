import { For, Match, Show, Switch, createEffect, createMemo, createSignal } from "solid-js";
import type {
  GuildRoleRecord,
  GuildVisibility,
  RoleName,
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
  viewAsRoleSimulatorEnabled: boolean;
  viewAsRoleSimulatorRole: RoleName;
  members: WorkspaceSettingsMemberRecord[];
  roles: GuildRoleRecord[];
  assignableRoleIds: WorkspaceRoleId[];
  onWorkspaceNameInput: (value: string) => void;
  onWorkspaceVisibilityChange: (value: GuildVisibility) => void;
  onViewAsRoleSimulatorToggle: (value: boolean) => void;
  onViewAsRoleSimulatorRoleChange: (value: RoleName) => void;
  onSaveWorkspaceSettings: () => Promise<void> | void;
  onAssignMemberRole: (userId: string, roleId: WorkspaceRoleId) => Promise<void> | void;
  onUnassignMemberRole: (userId: string, roleId: WorkspaceRoleId) => Promise<void> | void;
}

type WorkspaceSettingsSectionId = "profile" | "simulator" | "members";

interface WorkspaceSettingsSectionRecord {
  id: WorkspaceSettingsSectionId;
  label: string;
  summary: string;
}

interface BannerPreset {
  id: string;
  label: string;
  gradient: string;
}

const WORKSPACE_SETTINGS_SECTIONS: WorkspaceSettingsSectionRecord[] = [
  {
    id: "profile",
    label: "Server Profile",
    summary: "Name, visibility, banner and card preview.",
  },
  {
    id: "simulator",
    label: "Permission Simulator",
    summary: "Locally clamp the UI to role permissions.",
  },
  {
    id: "members",
    label: "Members",
    summary: "Assign and remove workspace role mappings.",
  },
];

const BANNER_PRESETS: BannerPreset[] = [
  {
    id: "sun",
    label: "Sun Glow",
    gradient: "linear-gradient(135deg, #f4e4a2 0%, #d7bf63 100%)",
  },
  {
    id: "rose",
    label: "Rose Pulse",
    gradient: "linear-gradient(135deg, #c74989 0%, #7d2f6a 100%)",
  },
  {
    id: "ember",
    label: "Ember Drift",
    gradient: "linear-gradient(135deg, #b34d30 0%, #782530 100%)",
  },
  {
    id: "amber",
    label: "Amber Dusk",
    gradient: "linear-gradient(135deg, #c67a32 0%, #7d5231 100%)",
  },
  {
    id: "citrus",
    label: "Citrus Moss",
    gradient: "linear-gradient(135deg, #9e933f 0%, #5f7043 100%)",
  },
  {
    id: "violet",
    label: "Violet Fog",
    gradient: "linear-gradient(135deg, #5f4b82 0%, #7a5b91 100%)",
  },
  {
    id: "aqua",
    label: "Aqua Orbit",
    gradient: "linear-gradient(135deg, #2f5d9f 0%, #3d8aa2 100%)",
  },
  {
    id: "teal",
    label: "Teal Field",
    gradient: "linear-gradient(135deg, #3f8b84 0%, #5ba8a2 100%)",
  },
  {
    id: "forest",
    label: "Forest Night",
    gradient: "linear-gradient(135deg, #355b2a 0%, #1f2f2e 100%)",
  },
  {
    id: "slate",
    label: "Slate Fade",
    gradient: "linear-gradient(135deg, #414651 0%, #252833 100%)",
  },
];

function normalizeTraitInput(value: string): string {
  return value.trim().replace(/\s+/g, " ");
}

export function WorkspaceSettingsPanel(props: WorkspaceSettingsPanelProps) {
  const [memberRoleDraftByUserId, setMemberRoleDraftByUserId] = createSignal<
    Record<string, WorkspaceRoleId | "">
  >({});
  const [memberSearchQuery, setMemberSearchQuery] = createSignal("");
  const [memberRoleClientError, setMemberRoleClientError] = createSignal("");
  const [activeSectionId, setActiveSectionId] = createSignal<WorkspaceSettingsSectionId>("profile");
  const [selectedBannerPresetId, setSelectedBannerPresetId] = createSignal(BANNER_PRESETS[0]?.id ?? "");
  const [traitDraft, setTraitDraft] = createSignal("");
  const [traits, setTraits] = createSignal<string[]>([]);

  const panelSectionClass = "grid gap-[0.64rem] rounded-[0.86rem] border border-line bg-bg-2 p-[0.9rem]";
  const panelShellClass = "grid min-w-0 content-start gap-[0.84rem]";
  const sectionLabelClassName =
    "m-0 text-[0.68rem] uppercase tracking-[0.08em] text-ink-2";
  const mutedTextClass = "m-0 text-[0.91rem] text-ink-2";
  const fieldLabelClass = "grid gap-[0.32rem] text-[0.84rem] text-ink-1";
  const fieldControlClass =
    "rounded-[0.62rem] border border-line-soft bg-bg-1 px-[0.6rem] py-[0.62rem] text-ink-0 transition-colors duration-[140ms] ease-out focus:outline-none focus:border-brand-strong disabled:cursor-default disabled:opacity-62";
  const submitButtonClass =
    "min-h-[2.16rem] rounded-[0.62rem] border border-line-soft bg-bg-3 px-[0.82rem] py-[0.5rem] text-ink-1 transition-all duration-[140ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-4 disabled:cursor-default disabled:opacity-62";
  const navButtonClass =
    "w-full rounded-[0.68rem] border border-line-soft bg-bg-3 px-[0.7rem] py-[0.58rem] text-left transition-all duration-[140ms] ease-out focus:outline-none focus-visible:ring-2 focus-visible:ring-brand/60";
  const statusOkClass = "mt-[0.3rem] text-[0.91rem] text-ok";
  const statusErrorClass = "mt-[0.3rem] text-[0.91rem] text-danger";
  const tertiaryButtonClass =
    "min-h-[2rem] rounded-[0.56rem] border border-line-soft bg-bg-3 px-[0.68rem] py-[0.42rem] text-[0.83rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-4 disabled:cursor-default disabled:opacity-62";
  const neutralButtonClass =
    "min-h-[2rem] rounded-[0.56rem] border border-line-soft bg-bg-1 px-[0.68rem] py-[0.42rem] text-[0.83rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-2 disabled:cursor-default disabled:opacity-62";
  const destructiveButtonClass =
    "min-h-[2rem] rounded-[0.56rem] border border-danger/35 bg-danger/10 px-[0.68rem] py-[0.42rem] text-[0.83rem] text-danger transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-danger/20 disabled:cursor-default disabled:opacity-62";
  const memberListClass = "m-0 grid list-none gap-[0.46rem] p-0";
  const memberRowClass =
    "grid gap-[0.45rem] rounded-[0.7rem] border border-line-soft bg-bg-1 p-[0.62rem]";
  const badgeClass =
    "inline-flex items-center gap-[0.32rem] rounded-[99px] border border-line-soft bg-bg-2 px-[0.5rem] py-[0.2rem] text-[0.76rem] text-ink-1";
  const miniButtonClass =
    "rounded-[0.45rem] border border-line-soft bg-bg-2 px-[0.5rem] py-[0.34rem] text-[0.75rem] text-ink-1 transition-colors duration-[120ms] ease-out enabled:cursor-pointer enabled:hover:bg-bg-3 disabled:cursor-default disabled:opacity-62";

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
  const selectedBannerPreset = createMemo(
    () =>
      BANNER_PRESETS.find((preset) => preset.id === selectedBannerPresetId()) ??
      BANNER_PRESETS[0],
  );
  const workspaceInitial = createMemo(() => {
    const fromName = props.workspaceName.trim().charAt(0).toUpperCase();
    return fromName || "W";
  });
  const memberCountLabel = createMemo(() => {
    const count = props.members.length;
    return `${count} Member${count === 1 ? "" : "s"}`;
  });
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

  const normalizedTraitDraft = createMemo(() => normalizeTraitInput(traitDraft()).slice(0, 24));
  const canAddTrait = createMemo(() => {
    const candidate = normalizedTraitDraft();
    if (!candidate) {
      return false;
    }
    if (traits().length >= 5) {
      return false;
    }
    const lowered = candidate.toLowerCase();
    return !traits().some((existing) => existing.toLowerCase() === lowered);
  });

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

  const addTrait = (): void => {
    if (!canAddTrait()) {
      return;
    }
    const candidate = normalizedTraitDraft();
    setTraits((existing) => [...existing, candidate]);
    setTraitDraft("");
  };

  const removeTrait = (trait: string): void => {
    setTraits((existing) => existing.filter((current) => current !== trait));
  };

  return (
    <section class="grid grid-cols-1 gap-[0.9rem] md:grid-cols-[13.25rem_minmax(0,1fr)]" aria-label="workspace settings">
      <aside class="grid content-start gap-[0.7rem] rounded-[0.86rem] border border-line bg-bg-2 p-[0.78rem]" aria-label="Workspace settings navigation">
        <p class={sectionLabelClassName}>WORKSPACE</p>
        <p class="m-0 break-words text-[0.94rem] font-[760] text-ink-0">{props.workspaceName || "Untitled workspace"}</p>
        <ul class="m-0 grid list-none gap-[0.44rem] p-0">
          <For each={WORKSPACE_SETTINGS_SECTIONS}>
            {(section) => {
              const isActive = () => activeSectionId() === section.id;
              return (
                <li>
                  <button
                    type="button"
                    class={navButtonClass}
                    classList={{
                      "border-brand/85 bg-brand/20 text-ink-0": isActive(),
                    }}
                    onClick={() => setActiveSectionId(section.id)}
                    aria-label={`Open ${section.label} workspace section`}
                    aria-current={isActive() ? "page" : undefined}
                  >
                    <p class="m-0 text-[0.86rem] font-[700] text-ink-0">{section.label}</p>
                    <p class="m-[0.18rem_0_0] text-[0.74rem] text-ink-2 leading-[1.34]">{section.summary}</p>
                  </button>
                </li>
              );
            }}
          </For>
        </ul>
        <div class="grid gap-[0.36rem] rounded-[0.7rem] border border-line-soft bg-bg-1 p-[0.56rem]">
          <p class={sectionLabelClassName}>NOTES</p>
          <p class={mutedTextClass}>Banner and trait edits are local preview controls only.</p>
        </div>
      </aside>
      <Show
        when={props.hasActiveWorkspace}
        fallback={
          <section class={panelSectionClass}>
            <p class={sectionLabelClassName}>WORKSPACE</p>
            <p class={mutedTextClass}>No active workspace selected.</p>
          </section>
        }
      >
        <section class={panelShellClass}>
          <Switch>
            <Match when={activeSectionId() === "profile"}>
              <section class={panelSectionClass} aria-label="workspace profile settings">
                <div class="flex flex-wrap items-start gap-[0.78rem]">
                  <form
                    class="grid min-w-[17.5rem] flex-[1_1_22rem] gap-[0.75rem]"
                    onSubmit={(event) => {
                      event.preventDefault();
                      void props.onSaveWorkspaceSettings();
                    }}
                  >
                    <header class="grid gap-[0.34rem]">
                      <p class={sectionLabelClassName}>SERVER PROFILE</p>
                      <h4 class="m-0 text-[1.44rem] leading-tight text-ink-0">Server Profile</h4>
                      <p class={mutedTextClass}>
                        Customize how your server appears in invites and discovery previews.
                      </p>
                    </header>
                    <label class={fieldLabelClass}>
                      Name
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
                    <section class="grid gap-[0.55rem] rounded-[0.75rem] border border-line-soft bg-bg-1 p-[0.62rem]">
                      <p class={sectionLabelClassName}>ICON</p>
                      <p class={mutedTextClass}>Recommended icon resolution: at least 512x512.</p>
                      <div class="flex flex-wrap gap-[0.44rem]">
                        <button
                          class={tertiaryButtonClass}
                          type="button"
                        >
                          Change server icon
                        </button>
                        <button
                          class={destructiveButtonClass}
                          type="button"
                        >
                          Remove icon
                        </button>
                      </div>
                    </section>
                    <section class="grid gap-[0.55rem] rounded-[0.75rem] border border-line-soft bg-bg-1 p-[0.62rem]">
                      <p class={sectionLabelClassName}>BANNER</p>
                      <ul class="m-0 grid list-none grid-cols-[repeat(auto-fit,minmax(5.4rem,1fr))] gap-[0.46rem] p-0" aria-label="Workspace banner presets">
                        <For each={BANNER_PRESETS}>
                          {(preset) => (
                            <li>
                              <button
                                class="relative h-[2.9rem] w-full overflow-hidden rounded-[0.56rem] border border-line-soft transition-all duration-[140ms] ease-out hover:-translate-y-px"
                                classList={{
                                  "border-brand shadow-[0_0_0_1px_var(--brand)]": selectedBannerPresetId() === preset.id,
                                }}
                                type="button"
                                onClick={() => setSelectedBannerPresetId(preset.id)}
                                aria-label={`Use ${preset.label} banner preset`}
                                aria-pressed={selectedBannerPresetId() === preset.id}
                              >
                                <span class="absolute inset-0 block" style={{ background: preset.gradient }} />
                                <span class="sr-only">{preset.label}</span>
                              </button>
                            </li>
                          )}
                        </For>
                      </ul>
                    </section>
                    <section class="grid gap-[0.55rem] rounded-[0.75rem] border border-line-soft bg-bg-1 p-[0.62rem]">
                      <p class={sectionLabelClassName}>TRAITS</p>
                      <p class={mutedTextClass}>Add up to 5 local preview traits for your server card.</p>
                      <div class="flex flex-wrap gap-[0.45rem]">
                        <input
                          class={`${fieldControlClass} min-w-[10rem] flex-[1_1_10rem]`}
                          aria-label="Workspace trait input"
                          value={traitDraft()}
                          maxlength="24"
                          onInput={(event) => setTraitDraft(event.currentTarget.value)}
                          disabled={traits().length >= 5}
                          placeholder="e.g. security, infra"
                        />
                        <button
                          class={`${neutralButtonClass} flex-none`}
                          type="button"
                          onClick={addTrait}
                          disabled={!canAddTrait()}
                        >
                          Add trait
                        </button>
                      </div>
                      <div class="flex flex-wrap gap-[0.32rem]">
                        <Show
                          when={traits().length > 0}
                          fallback={<span class={badgeClass}>No traits added</span>}
                        >
                          <For each={traits()}>
                            {(trait) => (
                              <span class={badgeClass}>
                                {trait}
                                <button
                                  class={miniButtonClass}
                                  type="button"
                                  onClick={() => removeTrait(trait)}
                                  aria-label={`Remove trait ${trait}`}
                                >
                                  x
                                </button>
                              </span>
                            )}
                          </For>
                        </Show>
                      </div>
                    </section>
                    <div class="flex flex-wrap gap-[0.44rem]">
                      <button
                        class={submitButtonClass}
                        type="submit"
                        disabled={props.isSavingWorkspaceSettings || !props.canManageWorkspaceSettings}
                      >
                        {props.isSavingWorkspaceSettings ? "Saving..." : "Save workspace"}
                      </button>
                    </div>
                    <Show when={!props.canManageWorkspaceSettings}>
                      <p class={mutedTextClass}>You need workspace role-management permissions to update these settings.</p>
                    </Show>
                    <Show when={props.workspaceSettingsStatus}>
                      <p class={statusOkClass}>{props.workspaceSettingsStatus}</p>
                    </Show>
                    <Show when={props.workspaceSettingsError}>
                      <p class={statusErrorClass}>{props.workspaceSettingsError}</p>
                    </Show>
                  </form>
                  <section class="grid w-[18.5rem] max-w-full flex-[0_0_18.5rem] content-start gap-[0.52rem] rounded-[0.78rem] border border-line-soft bg-bg-1 p-[0.64rem]">
                    <p class={sectionLabelClassName}>PREVIEW</p>
                    <article class="overflow-hidden rounded-[0.72rem] border border-line-soft bg-bg-2">
                      <div
                        class="h-[5.5rem] w-full transition-all duration-[180ms] ease-out"
                        style={{ background: selectedBannerPreset()?.gradient ?? BANNER_PRESETS[0]?.gradient ?? "#2c313c" }}
                      />
                      <div class="relative grid gap-[0.4rem] px-[0.74rem] pb-[0.74rem] pt-[1.75rem]">
                        <span
                          class="absolute left-[0.72rem] top-[-1.3rem] inline-flex h-[2.7rem] w-[2.7rem] items-center justify-center overflow-hidden rounded-[0.64rem] border-2 border-bg-2 bg-bg-4 text-[1.05rem] font-[780] text-ink-0"
                          aria-hidden="true"
                        >
                          {workspaceInitial()}
                        </span>
                        <p class="m-0 text-[1.02rem] font-[760] leading-tight text-ink-0">{props.workspaceName || "Workspace"}</p>
                        <p class="m-0 text-[0.8rem] text-ink-2">
                          {props.workspaceVisibility === "public" ? "Public" : "Private"} • {memberCountLabel()}
                        </p>
                        <div class="flex flex-wrap gap-[0.3rem]">
                          <Show when={traits().length > 0} fallback={<span class={badgeClass}>No traits</span>}>
                            <For each={traits().slice(0, 3)}>
                              {(trait) => <span class={badgeClass}>{trait}</span>}
                            </For>
                          </Show>
                        </div>
                      </div>
                    </article>
                  </section>
                </div>
              </section>
            </Match>
            <Match when={activeSectionId() === "simulator"}>
              <section class={panelSectionClass} aria-label="workspace permission simulator">
                <header class="grid gap-[0.34rem]">
                  <p class={sectionLabelClassName}>PERMISSION SIMULATOR</p>
                  <h4 class="m-0 text-[1.34rem] leading-tight text-ink-0">View As Role</h4>
                  <p class={mutedTextClass}>Simulation is local-only and does not change server authorization.</p>
                </header>
                <label class={fieldLabelClass}>
                  View server as role
                  <span class="flex items-center gap-[0.5rem] text-[0.84rem] text-ink-1">
                    <input
                      aria-label="Enable view server as role simulator"
                      type="checkbox"
                      checked={props.viewAsRoleSimulatorEnabled}
                      onChange={(event) =>
                        props.onViewAsRoleSimulatorToggle(event.currentTarget.checked)}
                    />
                    Clamp local UI permissions using a simulated role.
                  </span>
                </label>
                <label class={fieldLabelClass}>
                  Simulated role
                  <select
                    class={fieldControlClass}
                    aria-label="Workspace role simulator selection"
                    value={props.viewAsRoleSimulatorRole}
                    onChange={(event) =>
                      props.onViewAsRoleSimulatorRoleChange(
                        event.currentTarget.value === "owner"
                          ? "owner"
                          : event.currentTarget.value === "moderator"
                            ? "moderator"
                            : "member",
                      )}
                    disabled={!props.viewAsRoleSimulatorEnabled}
                  >
                    <option value="owner">owner</option>
                    <option value="moderator">moderator</option>
                    <option value="member">member</option>
                  </select>
                </label>
              </section>
            </Match>
            <Match when={activeSectionId() === "members"}>
              <section class={panelSectionClass} aria-label="workspace members settings">
                <header class="grid gap-[0.34rem]">
                  <p class={sectionLabelClassName}>MEMBERS</p>
                  <h4 class="m-0 text-[1.34rem] leading-tight text-ink-0">Member Role Assignments</h4>
                  <p class={mutedTextClass}>Grant or remove assignable roles for active workspace members.</p>
                </header>
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
            </Match>
          </Switch>
        </section>
      </Show>
    </section>
  );
}
