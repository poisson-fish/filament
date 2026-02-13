import { describe, expect, it, vi } from "vitest";
import {
  channelKindFromInput,
  guildVisibilityFromInput,
} from "../src/domain/chat";
import { createWorkspaceChannelCreatePanelGroups } from "../src/features/app-shell/runtime/workspace-channel-create-panel-groups";

describe("app shell workspace/channel create panel groups", () => {
  it("maps workspace and channel create panel handlers into grouped options", async () => {
    const onCreateWorkspaceSubmit = vi.fn();
    const setCreateGuildName = vi.fn();
    const setCreateGuildVisibility = vi.fn();
    const setCreateChannelName = vi.fn();
    const setCreateChannelKind = vi.fn();
    const onCancelWorkspaceCreate = vi.fn();
    const onCreateChannelSubmit = vi.fn();
    const setNewChannelName = vi.fn();
    const setNewChannelKind = vi.fn();
    const onCancelChannelCreate = vi.fn();

    const panelGroups = createWorkspaceChannelCreatePanelGroups({
      workspaceCreate: {
        createGuildName: "Ops",
        createGuildVisibility: guildVisibilityFromInput("private"),
        createChannelName: "alerts",
        createChannelKind: channelKindFromInput("text"),
        isCreatingWorkspace: false,
        canDismissWorkspaceCreateForm: true,
        workspaceError: "",
        onCreateWorkspaceSubmit,
        setCreateGuildName,
        setCreateGuildVisibility,
        setCreateChannelName,
        setCreateChannelKind,
        onCancelWorkspaceCreate,
      },
      channelCreate: {
        newChannelName: "ops-voice",
        newChannelKind: channelKindFromInput("voice"),
        isCreatingChannel: true,
        channelCreateError: "",
        onCreateChannelSubmit,
        setNewChannelName,
        setNewChannelKind,
        onCancelChannelCreate,
      },
    });

    expect(panelGroups.workspaceCreate.createGuildName).toBe("Ops");
    expect(panelGroups.channelCreate.newChannelName).toBe("ops-voice");

    const workspaceSubmitEvent = {
      preventDefault: vi.fn(),
    } as unknown as SubmitEvent;

    const channelSubmitEvent = {
      preventDefault: vi.fn(),
    } as unknown as SubmitEvent;

    await panelGroups.workspaceCreate.onCreateWorkspaceSubmit(workspaceSubmitEvent);
    await panelGroups.channelCreate.onCreateChannelSubmit(channelSubmitEvent);
    panelGroups.workspaceCreate.setCreateGuildName("Platform");
    panelGroups.workspaceCreate.setCreateGuildVisibility(
      guildVisibilityFromInput("public"),
    );
    panelGroups.workspaceCreate.setCreateChannelName("incident-bridge");
    panelGroups.workspaceCreate.setCreateChannelKind(channelKindFromInput("voice"));
    panelGroups.workspaceCreate.onCancelWorkspaceCreate();
    panelGroups.channelCreate.setNewChannelName("general");
    panelGroups.channelCreate.setNewChannelKind(channelKindFromInput("text"));
    panelGroups.channelCreate.onCancelChannelCreate();

    expect(onCreateWorkspaceSubmit).toHaveBeenCalledWith(workspaceSubmitEvent);
    expect(onCreateChannelSubmit).toHaveBeenCalledWith(channelSubmitEvent);
    expect(setCreateGuildName).toHaveBeenCalledWith("Platform");
    expect(setCreateGuildVisibility).toHaveBeenCalledWith(
      guildVisibilityFromInput("public"),
    );
    expect(setCreateChannelName).toHaveBeenCalledWith("incident-bridge");
    expect(setCreateChannelKind).toHaveBeenCalledWith(
      channelKindFromInput("voice"),
    );
    expect(onCancelWorkspaceCreate).toHaveBeenCalledOnce();
    expect(setNewChannelName).toHaveBeenCalledWith("general");
    expect(setNewChannelKind).toHaveBeenCalledWith(channelKindFromInput("text"));
    expect(onCancelChannelCreate).toHaveBeenCalledOnce();
  });
});
