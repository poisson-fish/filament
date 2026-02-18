import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  ChannelCreatePanel,
  type ChannelCreatePanelProps,
} from "../src/features/app-shell/components/panels/ChannelCreatePanel";

function channelCreatePanelPropsFixture(
  overrides: Partial<ChannelCreatePanelProps> = {},
): ChannelCreatePanelProps {
  return {
    newChannelName: "ops",
    newChannelKind: "text",
    isCreatingChannel: false,
    channelCreateError: "",
    onSubmit: vi.fn((event: SubmitEvent) => event.preventDefault()),
    onNewChannelNameInput: vi.fn(),
    onNewChannelKindChange: vi.fn(),
    onCancel: vi.fn(),
    ...overrides,
  };
}

describe("app shell channel create panel", () => {
  it("renders utility classes and does not depend on legacy helper hooks", () => {
    render(() =>
      <ChannelCreatePanel
        {...channelCreatePanelPropsFixture({
          channelCreateError: "channel name already exists",
        })}
      />,
    );

    const channelNameInput = screen.getByLabelText("Channel name");
    expect(channelNameInput).toHaveClass("border-line-soft");
    expect(channelNameInput).toHaveClass("bg-bg-2");
    expect(channelNameInput.closest("label")).toHaveClass("grid");

    const createButton = screen.getByRole("button", { name: "Create channel" });
    expect(createButton).toHaveClass("flex-1");
    expect(createButton).toHaveClass("border-line-soft");
    expect(screen.getByRole("button", { name: "Cancel" }).closest("div")).toHaveClass("flex");
    expect(screen.getByText("channel name already exists")).toHaveClass("text-danger");

    expect(document.querySelector(".member-group")).toBeNull();
    expect(document.querySelector(".inline-form")).toBeNull();
    expect(document.querySelector(".button-row")).toBeNull();
    expect(document.querySelector(".status")).toBeNull();
  });

  it("keeps handlers wired for submit/cancel/name/kind interactions", async () => {
    const onSubmit = vi.fn((event: SubmitEvent) => event.preventDefault());
    const onNewChannelNameInput = vi.fn();
    const onNewChannelKindChange = vi.fn();
    const onCancel = vi.fn();

    render(() =>
      <ChannelCreatePanel
        {...channelCreatePanelPropsFixture({
          onSubmit,
          onNewChannelNameInput,
          onNewChannelKindChange,
          onCancel,
        })}
      />,
    );

    await fireEvent.input(screen.getByLabelText("Channel name"), {
      target: { value: "alerts" },
    });
    expect(onNewChannelNameInput).toHaveBeenCalledWith("alerts");

    await fireEvent.change(screen.getByLabelText("Channel type"), {
      target: { value: "voice" },
    });
    expect(onNewChannelKindChange).toHaveBeenCalledWith("voice");

    const form = screen.getByRole("button", { name: "Create channel" }).closest("form");
    expect(form).not.toBeNull();
    await fireEvent.submit(form!);
    expect(onSubmit).toHaveBeenCalledTimes(1);

    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });
});
