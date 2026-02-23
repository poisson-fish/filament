import { createSignal } from "solid-js";
import { describe, expect, it } from "vitest";
import {
  guildIdFromInput,
  guildNameFromInput,
  type WorkspaceRecord,
} from "../src/domain/chat";
import { createOverlayPanelActions } from "../src/features/app-shell/runtime/overlay-panel-actions";
import type { OverlayPanel } from "../src/features/app-shell/types";

describe("app shell overlay panel actions", () => {
  it("opens workspace settings with active workspace defaults and clears state", () => {
    const activeWorkspace: WorkspaceRecord = {
      guildId: guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAZ"),
      guildName: guildNameFromInput("Security Ops"),
      visibility: "public",
      channels: [],
    };
    const [workspaceSettingsName, setWorkspaceSettingsName] = createSignal("old");
    const [workspaceSettingsVisibility, setWorkspaceSettingsVisibility] = createSignal<
      "private" | "public"
    >("private");
    const [workspaceSettingsStatus, setWorkspaceSettingsStatus] = createSignal("pending");
    const [workspaceSettingsError, setWorkspaceSettingsError] = createSignal("stale");
    const [activeOverlayPanel, setActiveOverlayPanel] =
      createSignal<OverlayPanel | null>(null);
    const [activeWorkspaceSettingsSection, setActiveWorkspaceSettingsSection] =
      createSignal<"profile" | "simulator" | "members" | "roles">("profile");

    const actions = createOverlayPanelActions({
      activeWorkspace: () => activeWorkspace,
      canCloseActivePanel: () => true,
      setWorkspaceSettingsName,
      setWorkspaceSettingsVisibility,
      setWorkspaceSettingsStatus,
      setWorkspaceSettingsError,
      setActiveOverlayPanel,
      setWorkspaceError: () => "",
      setChannelCreateError: () => "",
      setActiveSettingsCategory: () => "profile",
      setActiveVoiceSettingsSubmenu: () => "audio-devices",
      setActiveWorkspaceSettingsSection,
    });

    actions.openWorkspaceSettingsPanel("roles");

    expect(workspaceSettingsName()).toBe("Security Ops");
    expect(workspaceSettingsVisibility()).toBe("public");
    expect(workspaceSettingsStatus()).toBe("");
    expect(workspaceSettingsError()).toBe("");
    expect(activeOverlayPanel()).toBe("workspace-settings");
    expect(activeWorkspaceSettingsSection()).toBe("roles");
  });

  it("resets voice submenu when opening voice settings category", () => {
    const [activeSettingsCategory, setActiveSettingsCategory] = createSignal<"voice" | "profile">(
      "profile",
    );
    const [activeVoiceSettingsSubmenu, setActiveVoiceSettingsSubmenu] = createSignal<
      "audio-devices"
    >("audio-devices");
    const [activeWorkspaceSettingsSection, setActiveWorkspaceSettingsSection] =
      createSignal<"profile" | "simulator" | "members" | "roles">("profile");

    const actions = createOverlayPanelActions({
      activeWorkspace: () => undefined,
      canCloseActivePanel: () => true,
      setWorkspaceSettingsName: () => "",
      setWorkspaceSettingsVisibility: () => "private",
      setWorkspaceSettingsStatus: () => "",
      setWorkspaceSettingsError: () => "",
      setActiveOverlayPanel: () => null,
      setWorkspaceError: () => "",
      setChannelCreateError: () => "",
      setActiveSettingsCategory,
      setActiveVoiceSettingsSubmenu,
      setActiveWorkspaceSettingsSection,
    });

    actions.openSettingsCategory("voice");

    expect(activeSettingsCategory()).toBe("voice");
    expect(activeVoiceSettingsSubmenu()).toBe("audio-devices");
    expect(activeWorkspaceSettingsSection()).toBe("profile");
  });

  it("does not close panel when close is not allowed", () => {
    const [activeOverlayPanel, setActiveOverlayPanel] = createSignal<OverlayPanel | null>(
      "workspace-create",
    );
    const [activeWorkspaceSettingsSection, setActiveWorkspaceSettingsSection] =
      createSignal<"profile" | "simulator" | "members" | "roles">("profile");

    const actions = createOverlayPanelActions({
      activeWorkspace: () => undefined,
      canCloseActivePanel: () => false,
      setWorkspaceSettingsName: () => "",
      setWorkspaceSettingsVisibility: () => "private",
      setWorkspaceSettingsStatus: () => "",
      setWorkspaceSettingsError: () => "",
      setActiveOverlayPanel,
      setWorkspaceError: () => "",
      setChannelCreateError: () => "",
      setActiveSettingsCategory: () => "profile",
      setActiveVoiceSettingsSubmenu: () => "audio-devices",
      setActiveWorkspaceSettingsSection,
    });

    actions.closeOverlayPanel();

    expect(activeOverlayPanel()).toBe("workspace-create");
    expect(activeWorkspaceSettingsSection()).toBe("profile");
  });
});
