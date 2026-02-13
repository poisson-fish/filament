import { describe, expect, it, vi } from "vitest";
import {
  channelKindFromInput,
  guildVisibilityFromInput,
} from "../src/domain/chat";
import { createWorkspaceChannelCreatePanelGroupsOptions } from "../src/features/app-shell/runtime/workspace-channel-create-panel-groups-options";

describe("app shell workspace/channel create panel group state options", () => {
  it("maps runtime accessors and handlers into workspace/channel create group options", () => {
    const onCreateWorkspaceSubmit = vi.fn();
    const onCreateChannelSubmit = vi.fn();
    const setCreateGuildName = vi.fn();
    const setCreateGuildVisibility = vi.fn();
    const setCreateChannelName = vi.fn();
    const setCreateChannelKind = vi.fn();
    const setNewChannelName = vi.fn();
    const setNewChannelKind = vi.fn();
    const closeOverlayPanel = vi.fn();

    const options = createWorkspaceChannelCreatePanelGroupsOptions({
      createGuildName: () => "Ops",
      createGuildVisibility: () => guildVisibilityFromInput("private"),
      createChannelName: () => "alerts",
      createChannelKind: () => channelKindFromInput("text"),
      isCreatingWorkspace: () => false,
      canDismissWorkspaceCreateForm: () => true,
      workspaceError: () => "",
      onCreateWorkspaceSubmit,
      setCreateGuildName,
      setCreateGuildVisibility,
      setCreateChannelName,
      setCreateChannelKind,
      newChannelName: () => "ops-voice",
      newChannelKind: () => channelKindFromInput("voice"),
      isCreatingChannel: () => true,
      channelCreateError: () => "duplicate channel",
      onCreateChannelSubmit,
      setNewChannelName,
      setNewChannelKind,
      closeOverlayPanel,
    });

    expect(options.workspaceCreate.createGuildName).toBe("Ops");
    expect(options.workspaceCreate.createChannelName).toBe("alerts");
    expect(options.channelCreate.newChannelName).toBe("ops-voice");
    expect(options.channelCreate.channelCreateError).toBe("duplicate channel");

    options.workspaceCreate.setCreateGuildName("Platform");
    options.workspaceCreate.setCreateGuildVisibility(
      guildVisibilityFromInput("public"),
    );
    options.workspaceCreate.setCreateChannelName("incident-bridge");
    options.workspaceCreate.setCreateChannelKind(channelKindFromInput("voice"));
    options.channelCreate.setNewChannelName("general");
    options.channelCreate.setNewChannelKind(channelKindFromInput("text"));
    options.workspaceCreate.onCancelWorkspaceCreate();
    options.channelCreate.onCancelChannelCreate();

    expect(setCreateGuildName).toHaveBeenCalledWith("Platform");
    expect(setCreateGuildVisibility).toHaveBeenCalledWith(
      guildVisibilityFromInput("public"),
    );
    expect(setCreateChannelName).toHaveBeenCalledWith("incident-bridge");
    expect(setCreateChannelKind).toHaveBeenCalledWith(
      channelKindFromInput("voice"),
    );
    expect(setNewChannelName).toHaveBeenCalledWith("general");
    expect(setNewChannelKind).toHaveBeenCalledWith(channelKindFromInput("text"));
    expect(closeOverlayPanel).toHaveBeenCalledTimes(2);
    expect(options.workspaceCreate.onCreateWorkspaceSubmit).toBe(
      onCreateWorkspaceSubmit,
    );
    expect(options.channelCreate.onCreateChannelSubmit).toBe(
      onCreateChannelSubmit,
    );
  });
});