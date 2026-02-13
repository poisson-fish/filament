import { describe, expect, it, vi } from "vitest";

const runtimeMocks = vi.hoisted(() => ({
  createWorkspaceChannelOperationsController: vi.fn(),
}));

vi.mock(
  "../src/features/app-shell/runtime/workspace-channel-operations-controller",
  () => ({
    createWorkspaceChannelOperationsController:
      runtimeMocks.createWorkspaceChannelOperationsController,
  }),
);

import { createWorkspaceChannelOperationsActions } from "../src/features/app-shell/runtime/workspace-channel-operations-actions";

describe("app shell workspace-channel operations actions", () => {
  it("maps runtime state dependencies into workspace-channel operations controller", () => {
    const workspaceChannelState = {
      activeGuildId: vi.fn(),
      createGuildName: vi.fn(),
      createGuildVisibility: vi.fn(),
      createChannelName: vi.fn(),
      createChannelKind: vi.fn(),
      isCreatingWorkspace: vi.fn(),
      isCreatingChannel: vi.fn(),
      newChannelName: vi.fn(),
      newChannelKind: vi.fn(),
      setWorkspaces: vi.fn(),
      setActiveGuildId: vi.fn(),
      setActiveChannelId: vi.fn(),
      setCreateChannelKind: vi.fn(),
      setWorkspaceError: vi.fn(),
      setCreatingWorkspace: vi.fn(),
      setChannelCreateError: vi.fn(),
      setCreatingChannel: vi.fn(),
      setNewChannelName: vi.fn(),
      setNewChannelKind: vi.fn(),
    };

    const messageState = {
      setMessageStatus: vi.fn(),
    };

    const overlayState = {
      setActiveOverlayPanel: vi.fn(),
    };

    const session = vi.fn();
    const workspaceChannelOperationsController = {
      createWorkspace: vi.fn(async () => undefined),
      createNewChannel: vi.fn(async () => undefined),
    };

    runtimeMocks.createWorkspaceChannelOperationsController.mockReturnValue(
      workspaceChannelOperationsController,
    );

    const result = createWorkspaceChannelOperationsActions({
      session,
      workspaceChannelState: workspaceChannelState as unknown as Parameters<
        typeof createWorkspaceChannelOperationsActions
      >[0]["workspaceChannelState"],
      messageState: messageState as unknown as Parameters<
        typeof createWorkspaceChannelOperationsActions
      >[0]["messageState"],
      overlayState: overlayState as unknown as Parameters<
        typeof createWorkspaceChannelOperationsActions
      >[0]["overlayState"],
    });

    expect(
      runtimeMocks.createWorkspaceChannelOperationsController,
    ).toHaveBeenCalledWith({
      session,
      activeGuildId: workspaceChannelState.activeGuildId,
      createGuildName: workspaceChannelState.createGuildName,
      createGuildVisibility: workspaceChannelState.createGuildVisibility,
      createChannelName: workspaceChannelState.createChannelName,
      createChannelKind: workspaceChannelState.createChannelKind,
      isCreatingWorkspace: workspaceChannelState.isCreatingWorkspace,
      isCreatingChannel: workspaceChannelState.isCreatingChannel,
      newChannelName: workspaceChannelState.newChannelName,
      newChannelKind: workspaceChannelState.newChannelKind,
      setWorkspaces: workspaceChannelState.setWorkspaces,
      setActiveGuildId: workspaceChannelState.setActiveGuildId,
      setActiveChannelId: workspaceChannelState.setActiveChannelId,
      setCreateChannelKind: workspaceChannelState.setCreateChannelKind,
      setWorkspaceError: workspaceChannelState.setWorkspaceError,
      setCreatingWorkspace: workspaceChannelState.setCreatingWorkspace,
      setMessageStatus: messageState.setMessageStatus,
      setActiveOverlayPanel: overlayState.setActiveOverlayPanel,
      setChannelCreateError: workspaceChannelState.setChannelCreateError,
      setCreatingChannel: workspaceChannelState.setCreatingChannel,
      setNewChannelName: workspaceChannelState.setNewChannelName,
      setNewChannelKind: workspaceChannelState.setNewChannelKind,
    });
    expect(result).toBe(workspaceChannelOperationsController);
  });
});
