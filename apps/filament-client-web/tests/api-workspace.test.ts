import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";
import {
  guildIdFromInput,
  guildNameFromInput,
  guildVisibilityFromInput,
  userIdFromInput,
  workspaceRoleIdFromInput,
} from "../src/domain/chat";
import { createWorkspaceApi } from "../src/lib/api-workspace";

class MockApiError extends Error {
  readonly status: number;
  readonly code: string;

  constructor(status: number, code: string, message: string) {
    super(message);
    this.name = "MockApiError";
    this.status = status;
    this.code = code;
  }
}

describe("api-workspace", () => {
  const session = {
    accessToken: accessTokenFromInput("A".repeat(64)),
    refreshToken: refreshTokenFromInput("B".repeat(64)),
    expiresAtUnix: 2_000_000_000,
  };
  const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
  const guildName = guildNameFromInput("Filament Ops");
  const guildVisibility = guildVisibilityFromInput("private");
  const roleId = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB1");
  const userId = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB2");

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("fetchGuildRoles sends bounded request and parses strict DTO", async () => {
    const requestJson = vi.fn(async () => ({
      roles: [
        {
          role_id: roleId,
          name: "Responder",
          position: 2,
          is_system: false,
          permissions: ["create_message"],
        },
      ],
    }));

    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(api.fetchGuildRoles(session, guildId)).resolves.toMatchObject({
      roles: [{ roleId, name: "Responder" }],
    });
    expect(requestJson).toHaveBeenCalledWith({
      method: "GET",
      path: `/guilds/${guildId}/roles`,
      accessToken: session.accessToken,
    });
  });

  it("createGuild delegates and parses strict guild response", async () => {
    const requestJson = vi.fn(async () => ({
      guild_id: guildId,
      name: guildName,
      visibility: guildVisibility,
    }));

    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(
      api.createGuild(session, { name: guildName, visibility: guildVisibility }),
    ).resolves.toMatchObject({ guildId, name: guildName, visibility: guildVisibility });
    expect(requestJson).toHaveBeenCalledWith({
      method: "POST",
      path: "/guilds",
      accessToken: session.accessToken,
      body: { name: guildName, visibility: guildVisibility },
    });
  });

  it("fetchGuilds fails closed on invalid guild list shape", async () => {
    const requestJson = vi.fn(async () => ({ guilds: null }));
    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(api.fetchGuilds(session)).rejects.toMatchObject({
      status: 500,
      code: "invalid_guild_list_shape",
    });
  });

  it("updateGuild delegates with guild path and strict payload", async () => {
    const requestJson = vi.fn(async () => ({
      guild_id: guildId,
      name: guildName,
      visibility: guildVisibility,
    }));

    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(
      api.updateGuild(session, guildId, { name: guildName, visibility: guildVisibility }),
    ).resolves.toMatchObject({ guildId, name: guildName, visibility: guildVisibility });
    expect(requestJson).toHaveBeenCalledWith({
      method: "PATCH",
      path: `/guilds/${guildId}`,
      accessToken: session.accessToken,
      body: { name: guildName, visibility: guildVisibility },
    });
  });

  it("updateGuildRole fails closed when no update fields are provided", async () => {
    const requestJson = vi.fn(async () => null);
    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(api.updateGuildRole(session, guildId, roleId, {})).rejects.toMatchObject({
      status: 400,
      code: "invalid_request",
    });
    expect(requestJson).not.toHaveBeenCalled();
  });

  it("reorderGuildRoles dedupes role ids before sending request", async () => {
    const requestJson = vi.fn(async () => ({ accepted: true }));
    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    const roleIdTwo = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB3");
    await expect(
      api.reorderGuildRoles(session, guildId, [roleId, roleIdTwo, roleId]),
    ).resolves.toEqual({ accepted: true });

    expect(requestJson).toHaveBeenCalledWith({
      method: "POST",
      path: `/guilds/${guildId}/roles/reorder`,
      accessToken: session.accessToken,
      body: { role_ids: [roleId, roleIdTwo] },
    });
  });

  it("assign/unassign role delegates with normalized role id path", async () => {
    const requestJson = vi
      .fn()
      .mockResolvedValueOnce({ accepted: true })
      .mockResolvedValueOnce({ accepted: true });
    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    const normalizedRoleId = workspaceRoleIdFromInput(roleId);

    await expect(api.assignGuildRole(session, guildId, normalizedRoleId, userId)).resolves.toEqual({
      accepted: true,
    });
    await expect(api.unassignGuildRole(session, guildId, normalizedRoleId, userId)).resolves.toEqual({
      accepted: true,
    });

    expect(requestJson).toHaveBeenNthCalledWith(1, {
      method: "POST",
      path: `/guilds/${guildId}/roles/${normalizedRoleId}/members/${userId}`,
      accessToken: session.accessToken,
    });
    expect(requestJson).toHaveBeenNthCalledWith(2, {
      method: "DELETE",
      path: `/guilds/${guildId}/roles/${normalizedRoleId}/members/${userId}`,
      accessToken: session.accessToken,
    });
  });
});
