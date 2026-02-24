import { type AuthSession } from "../domain/auth";
import {
  type ChannelId,
  type ChannelKindName,
  type ChannelName,
  type ChannelPermissionSnapshot,
  type ChannelRecord,
  type DirectoryJoinResult,
  type GuildId,
  type GuildName,
  type GuildRecord,
  type GuildMemberPage,
  type GuildRoleList,
  type GuildRoleRecord,
  type GuildVisibility,
  type ModerationResult,
  type PermissionName,
  type PublicGuildDirectory,
  type RoleName,
  type UserId,
  type WorkspaceRoleId,
  type WorkspaceRoleName,
} from "../domain/chat";
import type { WorkspaceApi } from "./api-workspace";

interface WorkspaceClientDependencies {
  workspaceApi: WorkspaceApi;
}

export interface WorkspaceClient {
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
}

export function createWorkspaceClient(input: WorkspaceClientDependencies): WorkspaceClient {
  return {
    createGuild(session, payload) {
      return input.workspaceApi.createGuild(session, payload);
    },

    fetchGuilds(session) {
      return input.workspaceApi.fetchGuilds(session);
    },

    updateGuild(session, guildId, payload) {
      return input.workspaceApi.updateGuild(session, guildId, payload);
    },

    fetchPublicGuildDirectory(session, payload) {
      return input.workspaceApi.fetchPublicGuildDirectory(session, payload);
    },

    joinPublicGuild(session, guildId) {
      return input.workspaceApi.joinPublicGuild(session, guildId);
    },

    fetchGuildChannels(session, guildId) {
      return input.workspaceApi.fetchGuildChannels(session, guildId);
    },

    createChannel(session, guildId, payload) {
      return input.workspaceApi.createChannel(session, guildId, payload);
    },

    fetchChannelPermissionSnapshot(session, guildId, channelId) {
      return input.workspaceApi.fetchChannelPermissionSnapshot(session, guildId, channelId);
    },

    fetchGuildMembers(session, guildId, payload) {
      return input.workspaceApi.fetchGuildMembers(session, guildId, payload);
    },

    addGuildMember(session, guildId, userId) {
      return input.workspaceApi.addGuildMember(session, guildId, userId);
    },

    updateGuildMemberRole(session, guildId, userId, role) {
      return input.workspaceApi.updateGuildMemberRole(session, guildId, userId, role);
    },

    kickGuildMember(session, guildId, userId) {
      return input.workspaceApi.kickGuildMember(session, guildId, userId);
    },

    banGuildMember(session, guildId, userId) {
      return input.workspaceApi.banGuildMember(session, guildId, userId);
    },

    setChannelRoleOverride(session, guildId, channelId, role, payload) {
      return input.workspaceApi.setChannelRoleOverride(session, guildId, channelId, role, payload);
    },

    fetchGuildRoles(session, guildId) {
      return input.workspaceApi.fetchGuildRoles(session, guildId);
    },

    createGuildRole(session, guildId, payload) {
      return input.workspaceApi.createGuildRole(session, guildId, payload);
    },

    updateGuildRole(session, guildId, roleId, payload) {
      return input.workspaceApi.updateGuildRole(session, guildId, roleId, payload);
    },

    deleteGuildRole(session, guildId, roleId) {
      return input.workspaceApi.deleteGuildRole(session, guildId, roleId);
    },

    reorderGuildRoles(session, guildId, roleIds) {
      return input.workspaceApi.reorderGuildRoles(session, guildId, roleIds);
    },

    assignGuildRole(session, guildId, roleId, userId) {
      return input.workspaceApi.assignGuildRole(session, guildId, roleId, userId);
    },

    unassignGuildRole(session, guildId, roleId, userId) {
      return input.workspaceApi.unassignGuildRole(session, guildId, roleId, userId);
    },
  };
}
