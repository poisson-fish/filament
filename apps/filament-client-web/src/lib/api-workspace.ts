import {
  type AccessToken,
  type AuthSession,
} from "../domain/auth";
import {
  type ChannelId,
  type ChannelKindName,
  type ChannelName,
  type ChannelPermissionSnapshot,
  type ChannelRecord,
  type GuildId,
  type GuildName,
  type GuildRoleList,
  type GuildRoleRecord,
  type GuildRecord,
  type GuildVisibility,
  type DirectoryJoinResult,
  type ModerationResult,
  type PermissionName,
  type PublicGuildDirectory,
  type RoleName,
  type GuildMemberPage,
  type UserId,
  type WorkspaceRoleId,
  type WorkspaceRoleName,
  channelFromResponse,
  channelPermissionSnapshotFromResponse,
  directoryJoinResultFromResponse,
  guildMemberPageFromResponse,
  guildFromResponse,
  guildRoleListFromResponse,
  moderationResultFromResponse,
  publicGuildDirectoryFromResponse,
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
  createGuild(
    session: AuthSession,
    input: { name: GuildName; visibility?: GuildVisibility },
  ): Promise<GuildRecord>;
  fetchGuilds(session: AuthSession): Promise<GuildRecord[]>;
  updateGuild(
    session: AuthSession,
    guildId: GuildId,
    input: { name: GuildName; visibility?: GuildVisibility },
  ): Promise<GuildRecord>;
  fetchPublicGuildDirectory(
    session: AuthSession,
    input?: { query?: string; limit?: number },
  ): Promise<PublicGuildDirectory>;
  joinPublicGuild(
    session: AuthSession,
    guildId: GuildId,
  ): Promise<DirectoryJoinResult>;
  fetchGuildChannels(
    session: AuthSession,
    guildId: GuildId,
  ): Promise<ChannelRecord[]>;
  createChannel(
    session: AuthSession,
    guildId: GuildId,
    input: { name: ChannelName; kind: ChannelKindName },
  ): Promise<ChannelRecord>;
  fetchChannelPermissionSnapshot(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
  ): Promise<ChannelPermissionSnapshot>;
  fetchGuildMembers(
    session: AuthSession,
    guildId: GuildId,
    input?: { cursor?: UserId; limit?: number },
  ): Promise<GuildMemberPage>;
  addGuildMember(
    session: AuthSession,
    guildId: GuildId,
    userId: UserId,
  ): Promise<ModerationResult>;
  updateGuildMemberRole(
    session: AuthSession,
    guildId: GuildId,
    userId: UserId,
    role: RoleName,
  ): Promise<ModerationResult>;
  kickGuildMember(
    session: AuthSession,
    guildId: GuildId,
    userId: UserId,
  ): Promise<ModerationResult>;
  banGuildMember(
    session: AuthSession,
    guildId: GuildId,
    userId: UserId,
  ): Promise<ModerationResult>;
  setChannelRoleOverride(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
    role: RoleName,
    input: {
      allow: PermissionName[];
      deny: PermissionName[];
    },
  ): Promise<ModerationResult>;
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
  updateGuildDefaultJoinRole(
    session: AuthSession,
    guildId: GuildId,
    roleId: WorkspaceRoleId | null,
  ): Promise<ModerationResult>;
}

export function createWorkspaceApi(input: WorkspaceApiDependencies): WorkspaceApi {
  return {
    async createGuild(session, guildInput) {
      const dto = await input.requestJson({
        method: "POST",
        path: "/guilds",
        accessToken: session.accessToken,
        body: { name: guildInput.name, visibility: guildInput.visibility },
      });
      return guildFromResponse(dto);
    },

    async fetchGuilds(session) {
      const dto = await input.requestJson({
        method: "GET",
        path: "/guilds",
        accessToken: session.accessToken,
      });
      if (!dto || typeof dto !== "object" || !Array.isArray((dto as { guilds?: unknown }).guilds)) {
        throw input.createApiError(
          500,
          "invalid_guild_list_shape",
          "Unexpected guild list response.",
        );
      }
      return (dto as { guilds: unknown[] }).guilds.map((entry) => guildFromResponse(entry));
    },

    async updateGuild(session, guildId, guildInput) {
      const dto = await input.requestJson({
        method: "PATCH",
        path: `/guilds/${guildId}`,
        accessToken: session.accessToken,
        body: { name: guildInput.name, visibility: guildInput.visibility },
      });
      return guildFromResponse(dto);
    },

    async fetchPublicGuildDirectory(session, directoryInput) {
      const params = new URLSearchParams();
      const query = directoryInput?.query?.trim();
      if (query && query.length > 0) {
        params.set("q", query.slice(0, 64));
      }
      if (
        directoryInput?.limit
        && Number.isInteger(directoryInput.limit)
        && directoryInput.limit > 0
        && directoryInput.limit <= 50
      ) {
        params.set("limit", String(directoryInput.limit));
      }
      const suffix = params.size > 0 ? `?${params.toString()}` : "";
      const dto = await input.requestJson({
        method: "GET",
        path: `/guilds/public${suffix}`,
        accessToken: session.accessToken,
      });
      return publicGuildDirectoryFromResponse(dto);
    },

    async joinPublicGuild(session, guildId) {
      const dto = await input.requestJson({
        method: "POST",
        path: `/guilds/${guildId}/join`,
        accessToken: session.accessToken,
      });
      return directoryJoinResultFromResponse(dto);
    },

    async fetchGuildChannels(session, guildId) {
      const dto = await input.requestJson({
        method: "GET",
        path: `/guilds/${guildId}/channels`,
        accessToken: session.accessToken,
      });
      if (!dto || typeof dto !== "object" || !Array.isArray((dto as { channels?: unknown }).channels)) {
        throw input.createApiError(
          500,
          "invalid_channel_list_shape",
          "Unexpected channel list response.",
        );
      }
      return (dto as { channels: unknown[] }).channels.map((entry) => channelFromResponse(entry));
    },

    async createChannel(session, guildId, channelInput) {
      const dto = await input.requestJson({
        method: "POST",
        path: `/guilds/${guildId}/channels`,
        accessToken: session.accessToken,
        body: { name: channelInput.name, kind: channelInput.kind },
      });
      return channelFromResponse(dto);
    },

    async fetchChannelPermissionSnapshot(session, guildId, channelId) {
      const dto = await input.requestJson({
        method: "GET",
        path: `/guilds/${guildId}/channels/${channelId}/permissions/self`,
        accessToken: session.accessToken,
      });
      return channelPermissionSnapshotFromResponse(dto);
    },

    async fetchGuildMembers(session, guildId, memberInput) {
      const params = new URLSearchParams();
      if (memberInput?.cursor) {
        params.set("cursor", memberInput.cursor);
      }
      if (
        memberInput?.limit
        && Number.isInteger(memberInput.limit)
        && memberInput.limit > 0
        && memberInput.limit <= 200
      ) {
        params.set("limit", String(memberInput.limit));
      }
      const suffix = params.size > 0 ? `?${params.toString()}` : "";
      const dto = await input.requestJson({
        method: "GET",
        path: `/guilds/${guildId}/members${suffix}`,
        accessToken: session.accessToken,
      });
      return guildMemberPageFromResponse(dto);
    },

    async addGuildMember(session, guildId, userId) {
      const dto = await input.requestJson({
        method: "POST",
        path: `/guilds/${guildId}/members/${userId}`,
        accessToken: session.accessToken,
      });
      return moderationResultFromResponse(dto);
    },

    async updateGuildMemberRole(session, guildId, userId, role) {
      const dto = await input.requestJson({
        method: "PATCH",
        path: `/guilds/${guildId}/members/${userId}`,
        accessToken: session.accessToken,
        body: { role },
      });
      return moderationResultFromResponse(dto);
    },

    async kickGuildMember(session, guildId, userId) {
      const dto = await input.requestJson({
        method: "POST",
        path: `/guilds/${guildId}/members/${userId}/kick`,
        accessToken: session.accessToken,
      });
      return moderationResultFromResponse(dto);
    },

    async banGuildMember(session, guildId, userId) {
      const dto = await input.requestJson({
        method: "POST",
        path: `/guilds/${guildId}/members/${userId}/ban`,
        accessToken: session.accessToken,
      });
      return moderationResultFromResponse(dto);
    },

    async setChannelRoleOverride(session, guildId, channelId, role, roleInput) {
      const dto = await input.requestJson({
        method: "POST",
        path: `/guilds/${guildId}/channels/${channelId}/overrides/${role}`,
        accessToken: session.accessToken,
        body: roleInput,
      });
      return moderationResultFromResponse(dto);
    },

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

    async updateGuildDefaultJoinRole(session, guildId, roleId) {
      const dto = await input.requestJson({
        method: "POST",
        path: `/guilds/${guildId}/roles/default`,
        accessToken: session.accessToken,
        body: {
          role_id: roleId === null ? null : workspaceRoleIdFromInput(roleId),
        },
      });
      return moderationResultFromResponse(dto);
    },
  };
}
