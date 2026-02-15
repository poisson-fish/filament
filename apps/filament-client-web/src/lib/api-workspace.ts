import {
  type AccessToken,
  type AuthSession,
} from "../domain/auth";
import {
  type GuildId,
  type GuildRoleList,
  type GuildRoleRecord,
  type ModerationResult,
  type PermissionName,
  type UserId,
  type WorkspaceRoleId,
  type WorkspaceRoleName,
  guildRoleListFromResponse,
  moderationResultFromResponse,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
} from "../domain/chat";

interface JsonRequest {
  method: "GET" | "POST" | "PATCH" | "DELETE";
  path: string;
  body?: unknown;
  accessToken?: AccessToken;
}

interface WorkspaceApiDependencies {
  requestJson: (request: JsonRequest) => Promise<unknown>;
  createApiError: (status: number, code: string, message: string) => Error;
}

export interface WorkspaceApi {
  fetchGuildRoles(
    session: AuthSession,
    guildId: GuildId,
  ): Promise<GuildRoleList>;
  createGuildRole(
    session: AuthSession,
    guildId: GuildId,
    input: {
      name: WorkspaceRoleName;
      permissions: PermissionName[];
      position?: number;
    },
  ): Promise<GuildRoleRecord>;
  updateGuildRole(
    session: AuthSession,
    guildId: GuildId,
    roleId: WorkspaceRoleId,
    input: {
      name?: WorkspaceRoleName;
      permissions?: PermissionName[];
    },
  ): Promise<GuildRoleRecord>;
  deleteGuildRole(
    session: AuthSession,
    guildId: GuildId,
    roleId: WorkspaceRoleId,
  ): Promise<ModerationResult>;
  reorderGuildRoles(
    session: AuthSession,
    guildId: GuildId,
    roleIds: WorkspaceRoleId[],
  ): Promise<ModerationResult>;
  assignGuildRole(
    session: AuthSession,
    guildId: GuildId,
    roleId: WorkspaceRoleId,
    userId: UserId,
  ): Promise<ModerationResult>;
  unassignGuildRole(
    session: AuthSession,
    guildId: GuildId,
    roleId: WorkspaceRoleId,
    userId: UserId,
  ): Promise<ModerationResult>;
}

export function createWorkspaceApi(input: WorkspaceApiDependencies): WorkspaceApi {
  return {
    async fetchGuildRoles(session, guildId) {
      const dto = await input.requestJson({
        method: "GET",
        path: `/guilds/${guildId}/roles`,
        accessToken: session.accessToken,
      });
      return guildRoleListFromResponse(dto);
    },

    async createGuildRole(session, guildId, roleInput) {
      const dto = await input.requestJson({
        method: "POST",
        path: `/guilds/${guildId}/roles`,
        accessToken: session.accessToken,
        body: {
          name: workspaceRoleNameFromInput(roleInput.name),
          permissions: roleInput.permissions,
          position:
            Number.isInteger(roleInput.position) && (roleInput.position as number) > 0
              ? roleInput.position
              : undefined,
        },
      });
      return guildRoleListFromResponse({ roles: [dto] }).roles[0]!;
    },

    async updateGuildRole(session, guildId, roleId, roleInput) {
      if (typeof roleInput.name === "undefined" && typeof roleInput.permissions === "undefined") {
        throw input.createApiError(
          400,
          "invalid_request",
          "Role update requires at least one field.",
        );
      }

      const body: Record<string, unknown> = {};
      if (typeof roleInput.name !== "undefined") {
        body.name = workspaceRoleNameFromInput(roleInput.name);
      }
      if (typeof roleInput.permissions !== "undefined") {
        body.permissions = roleInput.permissions;
      }

      const dto = await input.requestJson({
        method: "PATCH",
        path: `/guilds/${guildId}/roles/${workspaceRoleIdFromInput(roleId)}`,
        accessToken: session.accessToken,
        body,
      });
      return guildRoleListFromResponse({ roles: [dto] }).roles[0]!;
    },

    async deleteGuildRole(session, guildId, roleId) {
      const dto = await input.requestJson({
        method: "DELETE",
        path: `/guilds/${guildId}/roles/${workspaceRoleIdFromInput(roleId)}`,
        accessToken: session.accessToken,
      });
      return moderationResultFromResponse(dto);
    },

    async reorderGuildRoles(session, guildId, roleIds) {
      const deduped: WorkspaceRoleId[] = [];
      const seen = new Set<string>();
      for (const roleId of roleIds) {
        const parsed = workspaceRoleIdFromInput(roleId);
        if (seen.has(parsed)) {
          continue;
        }
        seen.add(parsed);
        deduped.push(parsed);
      }

      if (deduped.length < 1 || deduped.length > 64) {
        throw input.createApiError(
          400,
          "invalid_request",
          "role_ids must contain 1-64 unique role ids.",
        );
      }

      const dto = await input.requestJson({
        method: "POST",
        path: `/guilds/${guildId}/roles/reorder`,
        accessToken: session.accessToken,
        body: { role_ids: deduped },
      });
      return moderationResultFromResponse(dto);
    },

    async assignGuildRole(session, guildId, roleId, userId) {
      const dto = await input.requestJson({
        method: "POST",
        path: `/guilds/${guildId}/roles/${workspaceRoleIdFromInput(roleId)}/members/${userId}`,
        accessToken: session.accessToken,
      });
      return moderationResultFromResponse(dto);
    },

    async unassignGuildRole(session, guildId, roleId, userId) {
      const dto = await input.requestJson({
        method: "DELETE",
        path: `/guilds/${guildId}/roles/${workspaceRoleIdFromInput(roleId)}/members/${userId}`,
        accessToken: session.accessToken,
      });
      return moderationResultFromResponse(dto);
    },
  };
}
