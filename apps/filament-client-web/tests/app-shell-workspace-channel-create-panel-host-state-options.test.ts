import { describe, expect, it, vi } from "vitest";
import { createWorkspaceChannelCreatePanelHostStateOptions } from "../src/features/app-shell/runtime/workspace-channel-create-panel-host-state-options";

describe("app shell workspace/channel-create panel-host state options", () => {
  it("maps runtime workspace/channel-create accessors and handlers", () => {
    const createGuildName = () => "Security Ops";
    const createGuildVisibility = () => "private" as const;
    const createChannelName = () => "incident-room";
    const createChannelKind = () => "text" as const;
    const isCreatingWorkspace = () => false;
    const workspaceError = () => "";
    const setCreateGuildName = vi.fn();
    const setCreateGuildVisibility = vi.fn();
    const setCreateChannelName = vi.fn();
    const setCreateChannelKind = vi.fn();
    const newChannelName = () => "backend";
    const newChannelKind = () => "voice" as const;
    const isCreatingChannel = () => true;
    const channelCreateError = () => "duplicate";
    const setNewChannelName = vi.fn();
    const setNewChannelKind = vi.fn();
    const canDismissWorkspaceCreateForm = () => true;
    const createWorkspace = vi.fn();
    const createNewChannel = vi.fn();
    const closeOverlayPanel = vi.fn();

    const options = createWorkspaceChannelCreatePanelHostStateOptions({
      workspaceChannelState: {
        createGuildName,
        createGuildVisibility,
        createChannelName,
        createChannelKind,
        isCreatingWorkspace,
        workspaceError,
        setCreateGuildName,
        setCreateGuildVisibility,
        setCreateChannelName,
        setCreateChannelKind,
        newChannelName,
        newChannelKind,
        isCreatingChannel,
        channelCreateError,
        setNewChannelName,
        setNewChannelKind,
      } as unknown as Parameters<
        typeof createWorkspaceChannelCreatePanelHostStateOptions
      >[0]["workspaceChannelState"],
      selectors: {
        canDismissWorkspaceCreateForm,
      } as unknown as Parameters<
        typeof createWorkspaceChannelCreatePanelHostStateOptions
      >[0]["selectors"],
      workspaceChannelOperations: {
        createWorkspace,
        createNewChannel,
      },
      closeOverlayPanel,
    });

    expect(options.createGuildName).toBe(createGuildName);
    expect(options.createGuildVisibility).toBe(createGuildVisibility);
    expect(options.createChannelName).toBe(createChannelName);
    expect(options.createChannelKind).toBe(createChannelKind);
    expect(options.isCreatingWorkspace).toBe(isCreatingWorkspace);
    expect(options.workspaceError).toBe(workspaceError);
    expect(options.setCreateGuildName).toBe(setCreateGuildName);
    expect(options.setCreateGuildVisibility).toBe(setCreateGuildVisibility);
    expect(options.setCreateChannelName).toBe(setCreateChannelName);
    expect(options.setCreateChannelKind).toBe(setCreateChannelKind);
    expect(options.newChannelName).toBe(newChannelName);
    expect(options.newChannelKind).toBe(newChannelKind);
    expect(options.isCreatingChannel).toBe(isCreatingChannel);
    expect(options.channelCreateError).toBe(channelCreateError);
    expect(options.setNewChannelName).toBe(setNewChannelName);
    expect(options.setNewChannelKind).toBe(setNewChannelKind);
    expect(options.canDismissWorkspaceCreateForm).toBe(
      canDismissWorkspaceCreateForm,
    );
    expect(options.onCreateWorkspaceSubmit).toBe(createWorkspace);
    expect(options.onCreateChannelSubmit).toBe(createNewChannel);
    expect(options.closeOverlayPanel).toBe(closeOverlayPanel);
  });
});
