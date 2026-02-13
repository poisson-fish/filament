import { createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { guildVisibilityFromInput } from "../src/domain/chat";
import { createWorkspaceSettingsPanelProps } from "../src/features/app-shell/runtime/workspace-settings-panel-props";

describe("app shell workspace settings panel props", () => {
  it("maps workspace settings state and save action", () => {
    const onSaveWorkspaceSettings = vi.fn();

    const panelProps = createWorkspaceSettingsPanelProps({
      hasActiveWorkspace: true,
      canManageWorkspaceSettings: true,
      workspaceName: "Filament",
      workspaceVisibility: guildVisibilityFromInput("private"),
      isSavingWorkspaceSettings: false,
      workspaceSettingsStatus: "ready",
      workspaceSettingsError: "",
      setWorkspaceSettingsName: () => undefined,
      setWorkspaceSettingsVisibility: () => guildVisibilityFromInput("private"),
      setWorkspaceSettingsStatus: () => "",
      setWorkspaceSettingsError: () => "",
      onSaveWorkspaceSettings,
    });

    expect(panelProps.hasActiveWorkspace).toBe(true);
    expect(panelProps.canManageWorkspaceSettings).toBe(true);
    expect(panelProps.workspaceName).toBe("Filament");
    expect(panelProps.workspaceVisibility).toBe("private");

    panelProps.onSaveWorkspaceSettings();
    expect(onSaveWorkspaceSettings).toHaveBeenCalledTimes(1);
  });

  it("clears workspace settings status and error when name changes", () => {
    const [workspaceName, setWorkspaceName] = createSignal("Initial");
    const [workspaceSettingsStatus, setWorkspaceSettingsStatus] = createSignal("saved");
    const [workspaceSettingsError, setWorkspaceSettingsError] = createSignal("error");

    const panelProps = createWorkspaceSettingsPanelProps({
      hasActiveWorkspace: true,
      canManageWorkspaceSettings: true,
      workspaceName: workspaceName(),
      workspaceVisibility: guildVisibilityFromInput("private"),
      isSavingWorkspaceSettings: false,
      workspaceSettingsStatus: workspaceSettingsStatus(),
      workspaceSettingsError: workspaceSettingsError(),
      setWorkspaceSettingsName: setWorkspaceName,
      setWorkspaceSettingsVisibility: () => guildVisibilityFromInput("private"),
      setWorkspaceSettingsStatus,
      setWorkspaceSettingsError,
      onSaveWorkspaceSettings: () => undefined,
    });

    panelProps.setWorkspaceSettingsName("Updated");

    expect(workspaceName()).toBe("Updated");
    expect(workspaceSettingsStatus()).toBe("");
    expect(workspaceSettingsError()).toBe("");
  });

  it("clears workspace settings status and error when visibility changes", () => {
    const [workspaceVisibility, setWorkspaceVisibility] = createSignal(
      guildVisibilityFromInput("private"),
    );
    const [workspaceSettingsStatus, setWorkspaceSettingsStatus] = createSignal("saved");
    const [workspaceSettingsError, setWorkspaceSettingsError] = createSignal("error");

    const panelProps = createWorkspaceSettingsPanelProps({
      hasActiveWorkspace: true,
      canManageWorkspaceSettings: true,
      workspaceName: "Filament",
      workspaceVisibility: workspaceVisibility(),
      isSavingWorkspaceSettings: false,
      workspaceSettingsStatus: workspaceSettingsStatus(),
      workspaceSettingsError: workspaceSettingsError(),
      setWorkspaceSettingsName: () => undefined,
      setWorkspaceSettingsVisibility: setWorkspaceVisibility,
      setWorkspaceSettingsStatus,
      setWorkspaceSettingsError,
      onSaveWorkspaceSettings: () => undefined,
    });

    panelProps.setWorkspaceSettingsVisibility(guildVisibilityFromInput("public"));

    expect(workspaceVisibility()).toBe("public");
    expect(workspaceSettingsStatus()).toBe("");
    expect(workspaceSettingsError()).toBe("");
  });
});