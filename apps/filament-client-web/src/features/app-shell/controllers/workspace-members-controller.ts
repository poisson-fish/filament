import { createEffect, createSignal, type Accessor, type Setter } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import type { GuildId, UserId, WorkspaceRoleId } from "../../../domain/chat";
import { ApiError, fetchGuildMembers } from "../../../lib/api";
import { mapError } from "../helpers";
import type { WorkspaceUserRolesByGuildId } from "../state/workspace-state";
import {
  MAX_WORKSPACE_SETTINGS_MEMBERS,
  WORKSPACE_MEMBER_LIST_PAGE_SIZE,
} from "../config/workspace-member-list";

export interface WorkspaceMembersControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  setWorkspaceUserRolesByGuildId: Setter<WorkspaceUserRolesByGuildId>;
}

export interface WorkspaceMembersControllerDependencies {
  fetchGuildMembers: typeof fetchGuildMembers;
  mapError: (error: unknown, fallback: string) => string;
}

export interface WorkspaceMembersController {
  membersByGuildId: Accessor<Record<string, UserId[]>>;
  isLoadingMembers: Accessor<boolean>;
  memberListError: Accessor<string>;
  refreshMembers: () => Promise<void>;
}

const DEFAULT_WORKSPACE_MEMBERS_CONTROLLER_DEPENDENCIES: WorkspaceMembersControllerDependencies =
{
  fetchGuildMembers,
  mapError,
};

export function createWorkspaceMembersController(
  options: WorkspaceMembersControllerOptions,
  dependencies: Partial<WorkspaceMembersControllerDependencies> = {},
): WorkspaceMembersController {
  const deps = {
    ...DEFAULT_WORKSPACE_MEMBERS_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };

  const [membersByGuildId, setMembersByGuildId] = createSignal<Record<string, UserId[]>>({});
  const [isLoadingMembers, setLoadingMembers] = createSignal(false);
  const [memberListError, setMemberListError] = createSignal("");

  let loadVersion = 0;
  let lastGuildId: GuildId | null = null;
  let lastSessionToken = "";

  createEffect(() => {
    if (options.session()) {
      return;
    }
    setMembersByGuildId({});
    setLoadingMembers(false);
    setMemberListError("");
  });

  const refreshMembers = async (): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId) {
      return;
    }

    const requestVersion = ++loadVersion;
    setLoadingMembers(true);
    setMemberListError("");

    try {
      const members: Array<{ userId: UserId; roleIds: WorkspaceRoleId[] }> = [];
      let cursor: UserId | null = null;
      let remaining = MAX_WORKSPACE_SETTINGS_MEMBERS;

      while (remaining > 0) {
        const page = await deps.fetchGuildMembers(session, guildId, {
          cursor: cursor ?? undefined,
          limit: Math.min(WORKSPACE_MEMBER_LIST_PAGE_SIZE, remaining),
        });
        members.push(...page.members);
        remaining = MAX_WORKSPACE_SETTINGS_MEMBERS - members.length;
        if (!page.nextCursor || remaining <= 0) {
          cursor = null;
          break;
        }
        cursor = page.nextCursor;
      }

      if (requestVersion !== loadVersion) {
        return;
      }

      setMembersByGuildId((existing) => ({
        ...existing,
        [guildId]: members.map((member) => member.userId),
      }));

      options.setWorkspaceUserRolesByGuildId((existing) => {
        const current = existing[guildId] ?? {};
        const next = { ...current };
        for (const member of members) {
          if (member.roleIds.length > 0) {
            next[member.userId] = member.roleIds;
          } else if (member.userId in next) {
            delete next[member.userId];
          }
        }
        return {
          ...existing,
          [guildId]: next,
        };
      });
    } catch (error) {
      if (requestVersion !== loadVersion) {
        return;
      }
      if (error instanceof ApiError && error.code === "forbidden") {
        setMemberListError("Member roster access denied for this workspace.");
      } else {
        setMemberListError(
          deps.mapError(error, "Unable to load workspace members."),
        );
      }
    } finally {
      if (requestVersion === loadVersion) {
        setLoadingMembers(false);
      }
    }
  };

  createEffect(() => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId) {
      return;
    }

    const sessionToken = session.accessToken;
    if (lastGuildId !== guildId || lastSessionToken !== sessionToken) {
      lastGuildId = guildId;
      lastSessionToken = sessionToken;
      void refreshMembers();
    }
  });

  return {
    membersByGuildId,
    isLoadingMembers,
    memberListError,
    refreshMembers,
  };
}
