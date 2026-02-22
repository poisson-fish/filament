import { describe, expect, it } from "vitest";
import type { GuildRoleRecord, WorkspaceRoleId } from "../src/domain/chat";
import {
  applyChannelOverrides,
  computeBasePermissions,
  permissionBitsFromList,
  resolveEffectiveChannelPermissions,
} from "../src/features/app-shell/permissions/effective-permissions";

const ROLE_EVERYONE = "01ARZ3NDEKTSV4RRFFQ69G5RA1" as WorkspaceRoleId;
const ROLE_MEMBER = "01ARZ3NDEKTSV4RRFFQ69G5RA2" as WorkspaceRoleId;
const ROLE_MODERATOR = "01ARZ3NDEKTSV4RRFFQ69G5RA3" as WorkspaceRoleId;
const ROLE_OWNER = "01ARZ3NDEKTSV4RRFFQ69G5RA4" as WorkspaceRoleId;

function roleFixtures(): GuildRoleRecord[] {
  return [
    {
      roleId: ROLE_EVERYONE,
      name: "@everyone" as GuildRoleRecord["name"],
      position: 0,
      isSystem: true,
      permissions: ["create_message", "subscribe_streams"],
    },
    {
      roleId: ROLE_MEMBER,
      name: "member" as GuildRoleRecord["name"],
      position: 1,
      isSystem: false,
      permissions: ["create_message", "subscribe_streams"],
    },
    {
      roleId: ROLE_MODERATOR,
      name: "moderator" as GuildRoleRecord["name"],
      position: 100,
      isSystem: false,
      permissions: ["delete_message", "ban_member", "manage_channel_overrides"],
    },
    {
      roleId: ROLE_OWNER,
      name: "workspace_owner" as GuildRoleRecord["name"],
      position: 10_000,
      isSystem: true,
      permissions: ["manage_roles"],
    },
  ];
}

describe("app shell effective permissions", () => {
  it("computes base permissions by OR-ing role sets", () => {
    const bits = computeBasePermissions([
      permissionBitsFromList(["create_message"]),
      permissionBitsFromList(["delete_message"]),
    ]);
    expect((bits & permissionBitsFromList(["create_message"])) !== 0).toBe(true);
    expect((bits & permissionBitsFromList(["delete_message"])) !== 0).toBe(true);
  });

  it("applies channel overrides in precedence order with deny winning inside a layer", () => {
    const resolved = applyChannelOverrides(
      false,
      permissionBitsFromList(["create_message", "delete_message"]),
      null,
      [
        {
          allow: permissionBitsFromList(["create_message"]),
          deny: permissionBitsFromList(["create_message", "delete_message"]),
        },
      ],
      {
        allow: permissionBitsFromList(["delete_message"]),
        deny: 0,
      },
    );
    expect((resolved & permissionBitsFromList(["create_message"])) !== 0).toBe(false);
    expect((resolved & permissionBitsFromList(["delete_message"])) !== 0).toBe(true);
  });

  it("resolves owner role to full known permission set", () => {
    const bits = resolveEffectiveChannelPermissions({
      channelPermissionsSnapshot: {
        role: "owner",
        permissions: ["create_message"],
      },
      guildRoles: roleFixtures(),
      assignedRoleIds: [ROLE_OWNER],
      channelOverrides: [],
    });
    expect((bits & permissionBitsFromList(["manage_roles"])) !== 0).toBe(true);
    expect((bits & permissionBitsFromList(["publish_screen_share"])) !== 0).toBe(true);
  });

  it("applies matching legacy role overrides for assigned roles", () => {
    const bits = resolveEffectiveChannelPermissions({
      channelPermissionsSnapshot: {
        role: "moderator",
        permissions: ["create_message", "delete_message"],
      },
      guildRoles: roleFixtures(),
      assignedRoleIds: [ROLE_MODERATOR],
      channelOverrides: [
        {
          targetKind: "legacy_role",
          role: "moderator",
          allow: [],
          deny: ["delete_message"],
          updatedAtUnix: 1,
        },
      ],
    });
    expect((bits & permissionBitsFromList(["delete_message"])) !== 0).toBe(false);
  });
});
