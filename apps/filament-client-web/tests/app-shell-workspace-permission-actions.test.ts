import { createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  channelIdFromInput,
  channelNameFromInput,
  type ChannelPermissionSnapshot,
  guildIdFromInput,
  guildNameFromInput,
  permissionFromInput,
  roleFromInput,
  type WorkspaceRecord,
} from "../src/domain/chat";
import { ApiError } from "../src/lib/api";
import { createWorkspacePermissionActions } from "../src/features/app-shell/runtime/workspace-permission-actions";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

const ACTIVE_GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const OTHER_GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");
const ACTIVE_CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAY");

function createWorkspacePermissionActionsHarness() {
  const [session] = createSignal(SESSION);
  const [activeGuildId] = createSignal<typeof ACTIVE_GUILD_ID | null>(ACTIVE_GUILD_ID);
  const [activeChannelId] = createSignal<typeof ACTIVE_CHANNEL_ID | null>(ACTIVE_CHANNEL_ID);
  const [channelPermissions, setChannelPermissions] =
    createSignal<ChannelPermissionSnapshot | null>({
      role: roleFromInput("member"),
      permissions: [],
    });
  const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([
    {
      guildId: ACTIVE_GUILD_ID,
      guildName: guildNameFromInput("Security Ops"),
      visibility: "private",
      channels: [
        {
          channelId: ACTIVE_CHANNEL_ID,
          name: channelNameFromInput("general"),
          kind: "text",
        },
      ],
    },
  ]);

  const refreshRoles = vi.fn();
  const fetchChannelPermissionSnapshot = vi.fn(async () => ({
    role: roleFromInput("member"),
    permissions: [permissionFromInput("create_message")],
  }));
  const pruneWorkspaceChannel = vi.fn((existing: WorkspaceRecord[]) => existing.slice(1));
  const shouldResetChannelPermissionsForError = vi.fn(() => false);

  const actions = createWorkspacePermissionActions(
    {
      session,
      activeGuildId,
      activeChannelId,
      setChannelPermissions,
      setWorkspaces,
      refreshRoles,
    },
    {
      fetchChannelPermissionSnapshot,
      pruneWorkspaceChannel,
      shouldResetChannelPermissionsForError,
    },
  );

  return {
    actions,
    state: {
      channelPermissions,
      workspaces,
    },
    deps: {
      refreshRoles,
      fetchChannelPermissionSnapshot,
      pruneWorkspaceChannel,
      shouldResetChannelPermissionsForError,
    },
  };
}

describe("app shell workspace permission actions", () => {
  it("refreshes roles and channel permissions when active workspace matches", async () => {
    const harness = createWorkspacePermissionActionsHarness();

    await harness.actions.refreshWorkspacePermissionStateFromGateway(ACTIVE_GUILD_ID);

    expect(harness.deps.refreshRoles).toHaveBeenCalledTimes(1);
    expect(harness.deps.fetchChannelPermissionSnapshot).toHaveBeenCalledTimes(1);
    expect(harness.state.channelPermissions()?.permissions).toContain(
      permissionFromInput("create_message"),
    );
  });

  it("no-ops for non-active guild updates", async () => {
    const harness = createWorkspacePermissionActionsHarness();

    await harness.actions.refreshWorkspacePermissionStateFromGateway(OTHER_GUILD_ID);

    expect(harness.deps.refreshRoles).not.toHaveBeenCalled();
    expect(harness.deps.fetchChannelPermissionSnapshot).not.toHaveBeenCalled();
  });

  it("resets permissions and prunes channel on forbidden/not-found errors", async () => {
    const harness = createWorkspacePermissionActionsHarness();
    const deniedError = new ApiError(403, "forbidden", "denied");

    harness.deps.fetchChannelPermissionSnapshot.mockRejectedValueOnce(deniedError);
    harness.deps.shouldResetChannelPermissionsForError.mockReturnValueOnce(true);

    await harness.actions.refreshWorkspacePermissionStateFromGateway(ACTIVE_GUILD_ID);

    expect(harness.deps.shouldResetChannelPermissionsForError).toHaveBeenCalledWith(
      deniedError,
    );
    expect(harness.state.channelPermissions()).toBeNull();
    expect(harness.deps.pruneWorkspaceChannel).toHaveBeenCalledTimes(1);
    expect(harness.state.workspaces()).toEqual([]);
  });
});
