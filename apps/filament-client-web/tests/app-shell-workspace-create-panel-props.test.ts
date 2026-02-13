import { describe, expect, it, vi } from "vitest";
import {
  channelKindFromInput,
  guildVisibilityFromInput,
} from "../src/domain/chat";
import { createWorkspaceCreatePanelProps } from "../src/features/app-shell/runtime/workspace-create-panel-props";

describe("app shell workspace create panel props", () => {
  it("maps workspace create values and handlers", async () => {
    const onCreateWorkspaceSubmit = vi.fn();
    const setCreateGuildName = vi.fn();
    const setCreateGuildVisibility = vi.fn();
    const setCreateChannelName = vi.fn();
    const setCreateChannelKind = vi.fn();
    const onCancelWorkspaceCreate = vi.fn();

    const panelProps = createWorkspaceCreatePanelProps({
      createGuildName: "Blue Team",
      createGuildVisibility: guildVisibilityFromInput("private"),
      createChannelName: "incident-bridge",
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
    });

    expect(panelProps.createGuildName).toBe("Blue Team");
    expect(panelProps.createChannelName).toBe("incident-bridge");
    expect(panelProps.isCreatingWorkspace).toBe(false);

    const submitEvent = {
      preventDefault: vi.fn(),
    } as unknown as SubmitEvent;

    await panelProps.onCreateWorkspaceSubmit(submitEvent);
    expect(onCreateWorkspaceSubmit).toHaveBeenCalledWith(submitEvent);

    panelProps.setCreateGuildName("Red Team");
    panelProps.setCreateGuildVisibility(guildVisibilityFromInput("public"));
    panelProps.setCreateChannelName("war-room");
    panelProps.setCreateChannelKind(channelKindFromInput("voice"));
    panelProps.onCancelWorkspaceCreate();

    expect(setCreateGuildName).toHaveBeenCalledWith("Red Team");
    expect(setCreateGuildVisibility).toHaveBeenCalledWith(
      guildVisibilityFromInput("public"),
    );
    expect(setCreateChannelName).toHaveBeenCalledWith("war-room");
    expect(setCreateChannelKind).toHaveBeenCalledWith(
      channelKindFromInput("voice"),
    );
    expect(onCancelWorkspaceCreate).toHaveBeenCalledOnce();
  });
});
