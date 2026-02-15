import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";
import {
  channelIdFromInput,
  channelKindFromInput,
  channelNameFromInput,
  guildIdFromInput,
  guildNameFromInput,
  guildVisibilityFromInput,
  type PermissionName,
  roleFromInput,
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
  const channelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FC1");
  const channelName = channelNameFromInput("ops-voice");
  const channelKind = channelKindFromInput("voice");
  const role = roleFromInput("member");
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

  it("fetchGuildChannels fails closed on invalid channel list shape", async () => {
    const requestJson = vi.fn(async () => ({ channels: null }));
    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(api.fetchGuildChannels(session, guildId)).rejects.toMatchObject({
      status: 500,
      code: "invalid_channel_list_shape",
    });
  });

  it("createChannel delegates with bounded payload and parses strict DTO", async () => {
    const requestJson = vi.fn(async () => ({
      channel_id: channelId,
      name: channelName,
      kind: channelKind,
    }));
    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(
      api.createChannel(session, guildId, { name: channelName, kind: channelKind }),
    ).resolves.toMatchObject({ channelId, name: channelName, kind: channelKind });
    expect(requestJson).toHaveBeenCalledWith({
      method: "POST",
      path: `/guilds/${guildId}/channels`,
      accessToken: session.accessToken,
      body: { name: channelName, kind: channelKind },
    });
  });

  it("fetchChannelPermissionSnapshot parses strict permissions DTO", async () => {
    const requestJson = vi.fn(async () => ({
      role: "member",
      permissions: ["create_message", "subscribe_streams"],
    }));
    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(api.fetchChannelPermissionSnapshot(session, guildId, channelId)).resolves.toEqual({
      role: "member",
      permissions: ["create_message", "subscribe_streams"],
    });
    expect(requestJson).toHaveBeenCalledWith({
      method: "GET",
      path: `/guilds/${guildId}/channels/${channelId}/permissions/self`,
      accessToken: session.accessToken,
    });
  });

  it("fetchPublicGuildDirectory bounds query/limit and parses strict response", async () => {
    const requestJson = vi.fn(async () => ({
      guilds: [
        {
          guild_id: guildId,
          name: guildName,
          visibility: "public",
          member_count: 42,
          blurb: "public blurb",
        },
      ],
    }));

    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(
      api.fetchPublicGuildDirectory(session, { query: `  ${"x".repeat(80)}  `, limit: 50 }),
    ).resolves.toMatchObject({ guilds: [{ guildId, name: guildName }] });

    expect(requestJson).toHaveBeenCalledWith({
      method: "GET",
      path: `/guilds/public?q=${"x".repeat(64)}&limit=50`,
      accessToken: session.accessToken,
    });
  });

  it("joinPublicGuild delegates and parses strict join response", async () => {
    const requestJson = vi.fn(async () => ({
      guild_id: guildId,
      outcome: "accepted",
    }));
    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(api.joinPublicGuild(session, guildId)).resolves.toMatchObject({
      guildId,
      outcome: "accepted",
      joined: true,
    });
    expect(requestJson).toHaveBeenCalledWith({
      method: "POST",
      path: `/guilds/${guildId}/join`,
      accessToken: session.accessToken,
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

  it("member moderation endpoints delegate with strict paths", async () => {
    const requestJson = vi
      .fn()
      .mockResolvedValueOnce({ accepted: true })
      .mockResolvedValueOnce({ accepted: true })
      .mockResolvedValueOnce({ accepted: true })
      .mockResolvedValueOnce({ accepted: true });
    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    await expect(api.addGuildMember(session, guildId, userId)).resolves.toEqual({ accepted: true });
    await expect(api.updateGuildMemberRole(session, guildId, userId, role)).resolves.toEqual({
      accepted: true,
    });
    await expect(api.kickGuildMember(session, guildId, userId)).resolves.toEqual({ accepted: true });
    await expect(api.banGuildMember(session, guildId, userId)).resolves.toEqual({ accepted: true });

    expect(requestJson).toHaveBeenNthCalledWith(1, {
      method: "POST",
      path: `/guilds/${guildId}/members/${userId}`,
      accessToken: session.accessToken,
    });
    expect(requestJson).toHaveBeenNthCalledWith(2, {
      method: "PATCH",
      path: `/guilds/${guildId}/members/${userId}`,
      accessToken: session.accessToken,
      body: { role },
    });
    expect(requestJson).toHaveBeenNthCalledWith(3, {
      method: "POST",
      path: `/guilds/${guildId}/members/${userId}/kick`,
      accessToken: session.accessToken,
    });
    expect(requestJson).toHaveBeenNthCalledWith(4, {
      method: "POST",
      path: `/guilds/${guildId}/members/${userId}/ban`,
      accessToken: session.accessToken,
    });
  });

  it("setChannelRoleOverride delegates override payload unchanged", async () => {
    const requestJson = vi.fn(async () => ({ accepted: true }));
    const api = createWorkspaceApi({
      requestJson,
      createApiError: (status, code, message) => new MockApiError(status, code, message),
    });

    const input: { allow: PermissionName[]; deny: PermissionName[] } = {
      allow: ["create_message"],
      deny: ["subscribe_streams"],
    };
    await expect(api.setChannelRoleOverride(session, guildId, channelId, role, input)).resolves.toEqual({
      accepted: true,
    });

    expect(requestJson).toHaveBeenCalledWith({
      method: "POST",
      path: `/guilds/${guildId}/channels/${channelId}/overrides/${role}`,
      accessToken: session.accessToken,
      body: input,
    });
  });
});
