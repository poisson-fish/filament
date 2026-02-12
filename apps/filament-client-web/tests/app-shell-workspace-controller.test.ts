import { describe, expect, it } from "vitest";
import {
  channelIdFromInput,
  channelNameFromInput,
  guildIdFromInput,
  guildNameFromInput,
  type WorkspaceRecord,
} from "../src/domain/chat";
import {
  pruneWorkspaceChannel,
  resolveWorkspaceSelection,
  shouldResetChannelPermissionsForError,
  shouldResetWorkspacesForBootstrapError,
} from "../src/features/app-shell/controllers/workspace-controller";
import { ApiError } from "../src/lib/api";

const GUILD_A = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAA");
const GUILD_B = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAB");
const CHANNEL_A1 = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAC");
const CHANNEL_A2 = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAD");
const CHANNEL_B1 = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAE");

function workspaceFixture(): WorkspaceRecord[] {
  return [
    {
      guildId: GUILD_A,
      guildName: guildNameFromInput("Security Ops"),
      visibility: "private",
      channels: [
        { channelId: CHANNEL_A1, name: channelNameFromInput("incident-room"), kind: "text" },
        { channelId: CHANNEL_A2, name: channelNameFromInput("war-room"), kind: "voice" },
      ],
    },
    {
      guildId: GUILD_B,
      guildName: guildNameFromInput("NOC"),
      visibility: "private",
      channels: [{ channelId: CHANNEL_B1, name: channelNameFromInput("alerts"), kind: "text" }],
    },
  ];
}

describe("app shell workspace controller", () => {
  it("prunes an inaccessible channel and drops empty workspaces", () => {
    const pruned = pruneWorkspaceChannel(workspaceFixture(), GUILD_B, CHANNEL_B1);

    expect(pruned).toHaveLength(1);
    expect(pruned[0]?.guildId).toBe(GUILD_A);
    expect(pruned[0]?.channels.map((channel) => channel.channelId)).toEqual([
      CHANNEL_A1,
      CHANNEL_A2,
    ]);
  });

  it("reselects fallback channel when active channel is removed", () => {
    const pruned = pruneWorkspaceChannel(workspaceFixture(), GUILD_A, CHANNEL_A1);
    const selection = resolveWorkspaceSelection(pruned, GUILD_A, CHANNEL_A1);

    expect(selection.guildId).toBe(GUILD_A);
    expect(selection.channelId).toBe(CHANNEL_A2);
  });

  it("resets channel permission state only for hard authz channel errors", () => {
    expect(
      shouldResetChannelPermissionsForError(
        new ApiError(403, "forbidden", "forbidden"),
      ),
    ).toBe(true);
    expect(
      shouldResetChannelPermissionsForError(
        new ApiError(404, "not_found", "not_found"),
      ),
    ).toBe(true);
    expect(
      shouldResetChannelPermissionsForError(
        new ApiError(429, "rate_limited", "rate_limited"),
      ),
    ).toBe(false);
    expect(
      shouldResetChannelPermissionsForError(
        new ApiError(0, "network_error", "network_error"),
      ),
    ).toBe(false);
    expect(shouldResetChannelPermissionsForError(new Error("oops"))).toBe(false);
  });

  it("resets bootstrap workspace state only for invalid auth", () => {
    expect(
      shouldResetWorkspacesForBootstrapError(
        new ApiError(401, "invalid_credentials", "invalid_credentials"),
      ),
    ).toBe(true);
    expect(
      shouldResetWorkspacesForBootstrapError(
        new ApiError(429, "rate_limited", "rate_limited"),
      ),
    ).toBe(false);
    expect(
      shouldResetWorkspacesForBootstrapError(
        new ApiError(0, "network_error", "network_error"),
      ),
    ).toBe(false);
    expect(
      shouldResetWorkspacesForBootstrapError(
        new ApiError(500, "internal_error", "internal_error"),
      ),
    ).toBe(false);
    expect(shouldResetWorkspacesForBootstrapError(new Error("oops"))).toBe(false);
  });
});
