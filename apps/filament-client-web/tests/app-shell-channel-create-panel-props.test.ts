import { describe, expect, it, vi } from "vitest";
import { channelKindFromInput } from "../src/domain/chat";
import { createChannelCreatePanelProps } from "../src/features/app-shell/runtime/channel-create-panel-props";

describe("app shell channel create panel props", () => {
  it("maps channel create values and handlers", async () => {
    const onCreateChannelSubmit = vi.fn();
    const setNewChannelName = vi.fn();
    const setNewChannelKind = vi.fn();
    const onCancelChannelCreate = vi.fn();

    const panelProps = createChannelCreatePanelProps({
      newChannelName: "alerts",
      newChannelKind: channelKindFromInput("text"),
      isCreatingChannel: false,
      channelCreateError: "",
      onCreateChannelSubmit,
      setNewChannelName,
      setNewChannelKind,
      onCancelChannelCreate,
    });

    expect(panelProps.newChannelName).toBe("alerts");
    expect(panelProps.newChannelKind).toBe(channelKindFromInput("text"));
    expect(panelProps.isCreatingChannel).toBe(false);

    const submitEvent = {
      preventDefault: vi.fn(),
    } as unknown as SubmitEvent;

    await panelProps.onCreateChannelSubmit(submitEvent);
    expect(onCreateChannelSubmit).toHaveBeenCalledWith(submitEvent);

    panelProps.setNewChannelName("ops");
    panelProps.setNewChannelKind(channelKindFromInput("voice"));
    panelProps.onCancelChannelCreate();

    expect(setNewChannelName).toHaveBeenCalledWith("ops");
    expect(setNewChannelKind).toHaveBeenCalledWith(
      channelKindFromInput("voice"),
    );
    expect(onCancelChannelCreate).toHaveBeenCalledOnce();
  });
});
