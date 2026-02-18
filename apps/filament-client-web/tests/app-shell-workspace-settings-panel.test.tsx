import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import { WorkspaceSettingsPanel, type WorkspaceSettingsPanelProps } from "../src/features/app-shell/components/panels/WorkspaceSettingsPanel";

function workspaceSettingsPanelPropsFixture(
  overrides: Partial<WorkspaceSettingsPanelProps> = {},
): WorkspaceSettingsPanelProps {
  return {
    hasActiveWorkspace: true,
    canManageWorkspaceSettings: true,
    workspaceName: "Filament",
    workspaceVisibility: "private",
    isSavingWorkspaceSettings: false,
    workspaceSettingsStatus: "",
    workspaceSettingsError: "",
    onWorkspaceNameInput: () => undefined,
    onWorkspaceVisibilityChange: () => undefined,
    onSaveWorkspaceSettings: () => undefined,
    ...overrides,
  };
}

describe("app shell workspace settings panel", () => {
  it("renders with utility section labels and without legacy group-label hooks", () => {
    render(() => <WorkspaceSettingsPanel {...workspaceSettingsPanelPropsFixture()} />);

    expect(screen.getByText("WORKSPACE")).toHaveClass("m-0");
    expect(screen.getByText("WORKSPACE")).toHaveClass("uppercase");
    expect(document.querySelector(".group-label")).toBeNull();
  });

  it("wires input and submit callbacks", async () => {
    const onWorkspaceNameInput = vi.fn();
    const onWorkspaceVisibilityChange = vi.fn();
    const onSaveWorkspaceSettings = vi.fn();

    render(() => (
      <WorkspaceSettingsPanel
        {...workspaceSettingsPanelPropsFixture({
          onWorkspaceNameInput,
          onWorkspaceVisibilityChange,
          onSaveWorkspaceSettings,
        })}
      />
    ));

    await fireEvent.input(screen.getByLabelText("Workspace settings name"), {
      target: { value: "Filament Updated" },
    });
    expect(onWorkspaceNameInput).toHaveBeenCalledWith("Filament Updated");

    await fireEvent.change(screen.getByLabelText("Workspace settings visibility"), {
      target: { value: "public" },
    });
    expect(onWorkspaceVisibilityChange).toHaveBeenCalledWith("public");

    const form = screen.getByRole("button", { name: "Save workspace" }).closest("form");
    expect(form).not.toBeNull();
    await fireEvent.submit(form!);
    expect(onSaveWorkspaceSettings).toHaveBeenCalledTimes(1);
  });
});
