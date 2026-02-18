import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  WorkspaceCreatePanel,
  type WorkspaceCreatePanelProps,
} from "../src/features/app-shell/components/panels/WorkspaceCreatePanel";

function workspaceCreatePanelPropsFixture(
  overrides: Partial<WorkspaceCreatePanelProps> = {},
): WorkspaceCreatePanelProps {
  return {
    createGuildName: "Security Ops",
    createGuildVisibility: "private",
    createChannelName: "alerts",
    createChannelKind: "text",
    isCreatingWorkspace: false,
    canDismissWorkspaceCreateForm: true,
    workspaceError: "",
    onSubmit: vi.fn((event: SubmitEvent) => event.preventDefault()),
    onCreateGuildNameInput: vi.fn(),
    onCreateGuildVisibilityChange: vi.fn(),
    onCreateChannelNameInput: vi.fn(),
    onCreateChannelKindChange: vi.fn(),
    onCancel: vi.fn(),
    ...overrides,
  };
}

describe("app shell workspace create panel", () => {
  it("renders utility classes and does not depend on legacy helper hooks", () => {
    render(() =>
      <WorkspaceCreatePanel
        {...workspaceCreatePanelPropsFixture({
          workspaceError: "workspace already exists",
        })}
      />,
    );

    const workspaceNameInput = screen.getByLabelText("Workspace name");
    expect(workspaceNameInput).toHaveClass("border-line-soft");
    expect(workspaceNameInput).toHaveClass("bg-bg-2");
    expect(workspaceNameInput.closest("label")).toHaveClass("grid");

    const createButton = screen.getByRole("button", { name: "Create workspace" });
    expect(createButton).toHaveClass("flex-1");
    expect(createButton).toHaveClass("border-line-soft");
    expect(screen.getByRole("button", { name: "Cancel" }).closest("div")).toHaveClass("flex");
    expect(screen.getByText("workspace already exists")).toHaveClass("text-danger");

    expect(document.querySelector(".member-group")).toBeNull();
    expect(document.querySelector(".inline-form")).toBeNull();
    expect(document.querySelector(".button-row")).toBeNull();
    expect(document.querySelector(".status")).toBeNull();
  });

  it("keeps handlers wired for submit/cancel and all form input bindings", async () => {
    const onSubmit = vi.fn((event: SubmitEvent) => event.preventDefault());
    const onCreateGuildNameInput = vi.fn();
    const onCreateGuildVisibilityChange = vi.fn();
    const onCreateChannelNameInput = vi.fn();
    const onCreateChannelKindChange = vi.fn();
    const onCancel = vi.fn();

    render(() =>
      <WorkspaceCreatePanel
        {...workspaceCreatePanelPropsFixture({
          onSubmit,
          onCreateGuildNameInput,
          onCreateGuildVisibilityChange,
          onCreateChannelNameInput,
          onCreateChannelKindChange,
          onCancel,
        })}
      />,
    );

    await fireEvent.input(screen.getByLabelText("Workspace name"), {
      target: { value: "Blue Team" },
    });
    expect(onCreateGuildNameInput).toHaveBeenCalledWith("Blue Team");

    await fireEvent.change(screen.getByLabelText("Visibility"), {
      target: { value: "public" },
    });
    expect(onCreateGuildVisibilityChange).toHaveBeenCalledWith("public");

    await fireEvent.input(screen.getByLabelText("First channel"), {
      target: { value: "incident-bridge" },
    });
    expect(onCreateChannelNameInput).toHaveBeenCalledWith("incident-bridge");

    await fireEvent.change(screen.getByLabelText("Channel type"), {
      target: { value: "voice" },
    });
    expect(onCreateChannelKindChange).toHaveBeenCalledWith("voice");

    const form = screen.getByRole("button", { name: "Create workspace" }).closest("form");
    expect(form).not.toBeNull();
    await fireEvent.submit(form!);
    expect(onSubmit).toHaveBeenCalledTimes(1);

    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onCancel).toHaveBeenCalledTimes(1);
  });

  it("omits cancel action when workspace-create dismissal is disabled", () => {
    render(() =>
      <WorkspaceCreatePanel
        {...workspaceCreatePanelPropsFixture({
          canDismissWorkspaceCreateForm: false,
        })}
      />,
    );

    expect(screen.queryByRole("button", { name: "Cancel" })).toBeNull();
  });
});
