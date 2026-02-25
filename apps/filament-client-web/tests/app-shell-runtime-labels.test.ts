import { createSignal } from "solid-js";
import { describe, expect, it } from "vitest";
import {
  guildIdFromInput,
  permissionFromInput,
  roleColorHexFromInput,
  userIdFromInput,
  type GuildRoleRecord,
  type WorkspaceRoleId,
  workspaceRoleIdFromInput,
  workspaceRoleNameFromInput,
} from "../src/domain/chat";
import { createAppShellRuntimeLabels } from "../src/features/app-shell/runtime/runtime-labels";

const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const USER_ID = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAZ");
const COLORED_ROLE_ID = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAY");

function labelsOptions(overrides?: {
  userRolesByGuildId?: Record<string, Record<string, WorkspaceRoleId[]>>;
}) {
  const [activeGuildId] = createSignal(GUILD_ID);
  const [workspaceRolesByGuildId] = createSignal<Record<string, GuildRoleRecord[]>>({
    [GUILD_ID]: [
      {
        roleId: COLORED_ROLE_ID,
        name: workspaceRoleNameFromInput("Responder"),
        position: 3,
        isSystem: false,
        permissions: [permissionFromInput("create_message")],
        colorHex: roleColorHexFromInput("#00AAFF"),
      },
    ],
  });
  const [workspaceUserRolesByGuildId] = createSignal<
    Record<string, Record<string, WorkspaceRoleId[]>>
  >(
    overrides?.userRolesByGuildId ?? {
      [GUILD_ID]: {
        [USER_ID]: [COLORED_ROLE_ID],
      },
    },
  );
  return {
    activeGuildId,
    workspaceRolesByGuildId,
    workspaceUserRolesByGuildId,
  };
}

describe("app shell runtime labels", () => {
  it("resolves actor labels from voice identities and username cache", () => {
    const [resolvedUsernames] = createSignal<Record<string, string>>({
      [USER_ID]: "owner",
    });
    const labels = createAppShellRuntimeLabels({
      resolvedUsernames,
      ...labelsOptions(),
    });

    expect(labels.actorLookupId(`u.${USER_ID}.mic`)).toBe(USER_ID);
    expect(labels.actorLabel(`u.${USER_ID}.mic`)).toBe("owner");
    expect(labels.displayUserLabel(USER_ID)).toBe("owner");
    expect(labels.displayUserColor(USER_ID)).toBe("#00AAFF");
  });

  it("falls back to shortened actor IDs and local participant suffix", () => {
    const [resolvedUsernames] = createSignal<Record<string, string>>({});
    const labels = createAppShellRuntimeLabels({
      resolvedUsernames,
      ...labelsOptions({ userRolesByGuildId: { [GUILD_ID]: {} } }),
    });

    expect(labels.actorLabel("this-is-a-very-long-actor-id")).toBe("this-is-a-very...");
    expect(labels.voiceParticipantLabel("local-user", true)).toBe("local-user (you)");
    expect(labels.voiceParticipantLabel("remote-user", false)).toBe("remote-user");
    expect(labels.displayUserColor(USER_ID)).toBeNull();
  });
});
