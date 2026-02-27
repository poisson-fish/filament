import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  guildIdFromInput,
  workspaceRoleIdFromInput,
} from "../src/domain/chat";
import { createWorkspaceMembersController } from "../src/features/app-shell/controllers/workspace-members-controller";
import type { WorkspaceUserRolesByGuildId } from "../src/features/app-shell/state/workspace-state";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});
const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const ROLE_ID = workspaceRoleIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");

async function flushPromises(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
}

describe("app shell workspace members controller", () => {
  it("bootstraps member role assignments for the active guild", async () => {
    await createRoot(async (dispose) => {
      const [session] = createSignal(SESSION);
      const [activeGuildId] = createSignal(GUILD_ID);
      const [workspaceUserRolesByGuildId, setWorkspaceUserRolesByGuildId] =
        createSignal<WorkspaceUserRolesByGuildId>({});

      const fetchGuildMembersMock = vi.fn(async () => ({
        members: [
          {
            userId: "01ARZ3NDEKTSV4RRFFQ69G5FAB",
            roleIds: [ROLE_ID],
          },
          {
            userId: "01ARZ3NDEKTSV4RRFFQ69G5FAC",
            roleIds: [],
          },
        ],
        nextCursor: null,
      }));

      const controller = createWorkspaceMembersController(
        {
          session,
          activeGuildId,
          setWorkspaceUserRolesByGuildId,
        },
        {
          fetchGuildMembers: fetchGuildMembersMock,
        },
      );

      await flushPromises();
      expect(fetchGuildMembersMock).toHaveBeenCalledTimes(1);
      expect(controller.membersByGuildId()[GUILD_ID]).toEqual([
        "01ARZ3NDEKTSV4RRFFQ69G5FAB",
        "01ARZ3NDEKTSV4RRFFQ69G5FAC",
      ]);
      expect(workspaceUserRolesByGuildId()[GUILD_ID]).toEqual({
        "01ARZ3NDEKTSV4RRFFQ69G5FAB": [ROLE_ID],
      });
      dispose();
    });
  });
});
