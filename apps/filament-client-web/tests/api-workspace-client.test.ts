import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";
import {
  channelFromResponse,
  channelIdFromInput,
  channelKindFromInput,
  channelNameFromInput,
  directoryJoinResultFromResponse,
  guildFromResponse,
  guildIdFromInput,
  guildNameFromInput,
  guildRoleListFromResponse,
  guildVisibilityFromInput,
  moderationResultFromResponse,
  roleFromInput,
  userIdFromInput,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
} from "../src/domain/chat";
import type { WorkspaceApi } from "../src/lib/api-workspace";
import { createWorkspaceClient } from "../src/lib/api-workspace-client";

describe("api-workspace-client", () => {
  function createSession() {
    return {
      accessToken: accessTokenFromInput("A".repeat(64)),
      refreshToken: refreshTokenFromInput("B".repeat(64)),
      expiresAtUnix: 2_000_000_000,
    };
  }

  function createWorkspaceApiStub(overrides?: Partial<WorkspaceApi>): WorkspaceApi {
    const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    const channelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB0");
    const roleId = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB1");

    const api: WorkspaceApi = {
      createGuild: vi.fn(async () =>
        guildFromResponse({
          guild_id: guildId,
          name: guildNameFromInput("filament-lab"),
          visibility: guildVisibilityFromInput("private"),
        }),
      ),
      fetchGuilds: vi.fn(async () => []),
      updateGuild: vi.fn(async () =>
        guildFromResponse({
          guild_id: guildId,
          name: guildNameFromInput("filament-lab"),
          visibility: guildVisibilityFromInput("private"),
        }),
      ),
      fetchPublicGuildDirectory: vi.fn(async () => ({ guilds: [] })),
      joinPublicGuild: vi.fn(async () =>
        directoryJoinResultFromResponse({
          guild_id: guildId,
          outcome: "accepted",
        }),
      ),
      fetchGuildChannels: vi.fn(async () =>
        [
          channelFromResponse({
            channel_id: channelId,
            name: channelNameFromInput("general"),
            kind: channelKindFromInput("text"),
          }),
        ],
      ),
      createChannel: vi.fn(async () =>
        channelFromResponse({
          channel_id: channelId,
          name: channelNameFromInput("general"),
          kind: channelKindFromInput("text"),
        }),
      ),
      fetchChannelPermissionSnapshot: vi.fn(async () => ({
        role: roleFromInput("member"),
        permissions: ["create_message" as const],
      })),
      addGuildMember: vi.fn(async () =>
        moderationResultFromResponse({
          accepted: true,
          actor_role: roleFromInput("moderator"),
          actor_permissions: ["manage_members"],
          target_role: roleFromInput("member"),
          guild_id: guildId,
        }),
      ),
      updateGuildMemberRole: vi.fn(async () =>
        moderationResultFromResponse({
          accepted: true,
          actor_role: roleFromInput("moderator"),
          actor_permissions: ["manage_members"],
          target_role: roleFromInput("member"),
          guild_id: guildId,
        }),
      ),
      kickGuildMember: vi.fn(async () =>
        moderationResultFromResponse({
          accepted: true,
          actor_role: roleFromInput("moderator"),
          actor_permissions: ["manage_members"],
          target_role: roleFromInput("member"),
          guild_id: guildId,
        }),
      ),
      banGuildMember: vi.fn(async () =>
        moderationResultFromResponse({
          accepted: true,
          actor_role: roleFromInput("moderator"),
          actor_permissions: ["manage_members"],
          target_role: roleFromInput("member"),
          guild_id: guildId,
        }),
      ),
      setChannelRoleOverride: vi.fn(async () =>
        moderationResultFromResponse({
          accepted: true,
          actor_role: roleFromInput("moderator"),
          actor_permissions: ["manage_channels"],
          target_role: roleFromInput("member"),
          guild_id: guildId,
        }),
      ),
      fetchGuildRoles: vi.fn(async () =>
        guildRoleListFromResponse({
          roles: [
            {
              role_id: roleId,
              name: workspaceRoleNameFromInput("Responder"),
              permissions: ["create_message"],
              position: 1,
              is_system: false,
            },
          ],
        }),
      ),
      createGuildRole: vi.fn(async () =>
        guildRoleListFromResponse({
          roles: [
            {
              role_id: roleId,
              name: workspaceRoleNameFromInput("Responder"),
              permissions: ["create_message"],
              position: 1,
              is_system: false,
            },
          ],
        }).roles[0]!,
      ),
      updateGuildRole: vi.fn(async () =>
        guildRoleListFromResponse({
          roles: [
            {
              role_id: roleId,
              name: workspaceRoleNameFromInput("Responder"),
              permissions: ["create_message"],
              position: 1,
              is_system: false,
            },
          ],
        }).roles[0]!,
      ),
      deleteGuildRole: vi.fn(async () =>
        moderationResultFromResponse({
          accepted: true,
          actor_role: roleFromInput("moderator"),
          actor_permissions: ["manage_roles"],
          target_role: roleFromInput("member"),
          guild_id: guildId,
        }),
      ),
      reorderGuildRoles: vi.fn(async () =>
        moderationResultFromResponse({
          accepted: true,
          actor_role: roleFromInput("moderator"),
          actor_permissions: ["manage_roles"],
          target_role: roleFromInput("member"),
          guild_id: guildId,
        }),
      ),
      assignGuildRole: vi.fn(async () =>
        moderationResultFromResponse({
          accepted: true,
          actor_role: roleFromInput("moderator"),
          actor_permissions: ["manage_roles"],
          target_role: roleFromInput("member"),
          guild_id: guildId,
        }),
      ),
      unassignGuildRole: vi.fn(async () =>
        moderationResultFromResponse({
          accepted: true,
          actor_role: roleFromInput("moderator"),
          actor_permissions: ["manage_roles"],
          target_role: roleFromInput("member"),
          guild_id: guildId,
        }),
      ),
    };

    return {
      ...api,
      ...overrides,
    };
  }

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("delegates createGuild through workspace API", async () => {
    const expectedGuild = guildFromResponse({
      guild_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      name: "filament-lab",
      visibility: "private",
    });
    const createGuild = vi.fn(async () => expectedGuild);
    const workspaceClient = createWorkspaceClient({
      workspaceApi: createWorkspaceApiStub({ createGuild }),
    });
    const session = createSession();
    const payload = {
      name: guildNameFromInput("filament-lab"),
      visibility: guildVisibilityFromInput("private"),
    };

    await expect(workspaceClient.createGuild(session, payload)).resolves.toBe(expectedGuild);
    expect(createGuild).toHaveBeenCalledWith(session, payload);
  });

  it("delegates fetchGuildRoles and returns upstream value", async () => {
    const expectedRoles = guildRoleListFromResponse({
      roles: [
        {
          role_id: "01ARZ3NDEKTSV4RRFFQ69G5FB1",
          name: "Responder",
          permissions: ["create_message"],
          position: 1,
          is_system: false,
        },
      ],
    });
    const fetchGuildRoles = vi.fn(async () => expectedRoles);
    const workspaceClient = createWorkspaceClient({
      workspaceApi: createWorkspaceApiStub({ fetchGuildRoles }),
    });
    const session = createSession();
    const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");

    await expect(workspaceClient.fetchGuildRoles(session, guildId)).resolves.toBe(expectedRoles);
    expect(fetchGuildRoles).toHaveBeenCalledWith(session, guildId);
  });

  it("delegates setChannelRoleOverride", async () => {
    const setChannelRoleOverride = vi.fn(async () =>
      moderationResultFromResponse({
        accepted: true,
        actor_role: roleFromInput("moderator"),
        actor_permissions: ["manage_channels"],
        target_role: roleFromInput("member"),
        guild_id: guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
      }),
    );
    const workspaceClient = createWorkspaceClient({
      workspaceApi: createWorkspaceApiStub({ setChannelRoleOverride }),
    });
    const session = createSession();
    const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    const channelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB0");
    const input = {
      allow: ["create_message"] as const,
      deny: ["delete_message"] as const,
    };

    await workspaceClient.setChannelRoleOverride(session, guildId, channelId, roleFromInput("member"), {
      allow: [...input.allow],
      deny: [...input.deny],
    });

    expect(setChannelRoleOverride).toHaveBeenCalledWith(
      session,
      guildId,
      channelId,
      roleFromInput("member"),
      { allow: ["create_message"], deny: ["delete_message"] },
    );
  });

  it("delegates assignGuildRole", async () => {
    const assignGuildRole = vi.fn(async () =>
      moderationResultFromResponse({
        accepted: true,
        actor_role: roleFromInput("moderator"),
        actor_permissions: ["manage_roles"],
        target_role: roleFromInput("member"),
        guild_id: guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
      }),
    );
    const workspaceClient = createWorkspaceClient({
      workspaceApi: createWorkspaceApiStub({ assignGuildRole }),
    });
    const session = createSession();
    const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    const roleId = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB1");
    const userId = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB2");

    await workspaceClient.assignGuildRole(session, guildId, roleId, userId);

    expect(assignGuildRole).toHaveBeenCalledWith(session, guildId, roleId, userId);
  });
});