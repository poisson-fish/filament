import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  type ChannelPermissionSnapshot,
  channelIdFromInput,
  guildIdFromInput,
  permissionFromInput,
  roleFromInput,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
} from "../src/domain/chat";
import { createRoleManagementController } from "../src/features/app-shell/controllers/role-management-controller";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});
const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const ROLE_ID = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");

function flushPromises(): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, 0));
}

describe("app shell role management controller", () => {
  it("runs role mutations and refreshes role/channel permission snapshots", async () => {
    await createRoot(async (dispose) => {
      const [session] = createSignal(SESSION);
      const [activeGuildId] = createSignal(GUILD_ID);
      const [activeChannelId] = createSignal(CHANNEL_ID);
      const [channelPermissions, setChannelPermissions] =
        createSignal<ChannelPermissionSnapshot | null>(null);

      const fetchGuildRolesMock = vi.fn(async () => ({
        roles: [
          {
            roleId: ROLE_ID,
            name: workspaceRoleNameFromInput("Responder"),
            position: 3,
            isSystem: false,
            permissions: [
              permissionFromInput("create_message"),
              permissionFromInput("subscribe_streams"),
            ],
          },
        ],
      }));
      const createGuildRoleMock = vi.fn(async () => ({
        roleId: ROLE_ID,
        name: workspaceRoleNameFromInput("Responder"),
        position: 3,
        isSystem: false,
        permissions: [permissionFromInput("create_message")],
      }));
      const updateGuildRoleMock = vi.fn(async () => ({
        roleId: ROLE_ID,
        name: workspaceRoleNameFromInput("Responder"),
        position: 3,
        isSystem: false,
        permissions: [
          permissionFromInput("create_message"),
          permissionFromInput("delete_message"),
        ],
      }));
      const deleteGuildRoleMock = vi.fn(async () => ({ accepted: true as const }));
      const reorderGuildRolesMock = vi.fn(async () => ({ accepted: true as const }));
      const assignGuildRoleMock = vi.fn(async () => ({ accepted: true as const }));
      const unassignGuildRoleMock = vi.fn(async () => ({ accepted: true as const }));
      const fetchChannelPermissionSnapshotMock = vi.fn(async () => ({
        role: roleFromInput("moderator"),
        permissions: [
          permissionFromInput("create_message"),
          permissionFromInput("manage_workspace_roles"),
        ],
      }));

      const controller = createRoleManagementController(
        {
          session,
          activeGuildId,
          activeChannelId,
          setChannelPermissions,
        },
        {
          fetchGuildRoles: fetchGuildRolesMock,
          createGuildRole: createGuildRoleMock,
          updateGuildRole: updateGuildRoleMock,
          deleteGuildRole: deleteGuildRoleMock,
          reorderGuildRoles: reorderGuildRolesMock,
          assignGuildRole: assignGuildRoleMock,
          unassignGuildRole: unassignGuildRoleMock,
          fetchChannelPermissionSnapshot: fetchChannelPermissionSnapshotMock,
        },
      );

      await flushPromises();
      expect(fetchGuildRolesMock).toHaveBeenCalledTimes(1);
      expect(controller.roles()).toHaveLength(1);

      await controller.createRole({
        name: "Responder",
        permissions: ["create_message", "subscribe_streams"],
      });
      expect(createGuildRoleMock).toHaveBeenCalledWith(SESSION, GUILD_ID, {
        name: "Responder",
        permissions: ["create_message", "subscribe_streams"],
        position: undefined,
      });
      expect(fetchChannelPermissionSnapshotMock).toHaveBeenCalledWith(
        SESSION,
        GUILD_ID,
        CHANNEL_ID,
      );
      expect(channelPermissions()).toEqual({
        role: "moderator",
        permissions: ["create_message", "manage_workspace_roles"],
      });

      await controller.updateRole(ROLE_ID, {
        name: "Responder",
        permissions: ["create_message", "delete_message"],
      });
      expect(updateGuildRoleMock).toHaveBeenCalledWith(SESSION, GUILD_ID, ROLE_ID, {
        name: "Responder",
        permissions: ["create_message", "delete_message"],
      });

      await controller.reorderRoles([ROLE_ID]);
      expect(reorderGuildRolesMock).toHaveBeenCalledWith(SESSION, GUILD_ID, [ROLE_ID]);

      await controller.assignRoleToMember("01ARZ3NDEKTSV4RRFFQ69G5FAY", ROLE_ID);
      expect(assignGuildRoleMock).toHaveBeenCalledWith(
        SESSION,
        GUILD_ID,
        ROLE_ID,
        "01ARZ3NDEKTSV4RRFFQ69G5FAY",
      );

      await controller.unassignRoleFromMember("01ARZ3NDEKTSV4RRFFQ69G5FAY", ROLE_ID);
      expect(unassignGuildRoleMock).toHaveBeenCalledWith(
        SESSION,
        GUILD_ID,
        ROLE_ID,
        "01ARZ3NDEKTSV4RRFFQ69G5FAY",
      );

      await controller.deleteRole(ROLE_ID);
      expect(deleteGuildRoleMock).toHaveBeenCalledWith(SESSION, GUILD_ID, ROLE_ID);

      expect(fetchGuildRolesMock).toHaveBeenCalledTimes(7);
      expect(controller.roleManagementStatus()).toBe("Role deleted.");
      expect(controller.roleManagementError()).toBe("");
      dispose();
    });
  });

  it("rejects invalid member target IDs before assignment calls", async () => {
    await createRoot(async (dispose) => {
      const [session] = createSignal(SESSION);
      const [activeGuildId] = createSignal(GUILD_ID);
      const [activeChannelId] = createSignal(CHANNEL_ID);
      const [_channelPermissions, setChannelPermissions] =
        createSignal<ChannelPermissionSnapshot | null>(null);

      const assignGuildRoleMock = vi.fn(async () => ({ accepted: true as const }));

      const controller = createRoleManagementController(
        {
          session,
          activeGuildId,
          activeChannelId,
          setChannelPermissions,
        },
        {
          fetchGuildRoles: vi.fn(async () => ({ roles: [] })),
          createGuildRole: vi.fn(),
          updateGuildRole: vi.fn(),
          deleteGuildRole: vi.fn(),
          reorderGuildRoles: vi.fn(),
          assignGuildRole: assignGuildRoleMock,
          unassignGuildRole: vi.fn(),
          fetchChannelPermissionSnapshot: vi.fn(),
        },
      );

      await flushPromises();
      await controller.assignRoleToMember("not-ulid", ROLE_ID);
      expect(assignGuildRoleMock).not.toHaveBeenCalled();
      expect(controller.roleManagementError()).toContain("Target user ID is invalid");
      dispose();
    });
  });
});
