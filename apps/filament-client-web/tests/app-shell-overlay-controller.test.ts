import { createSignal } from "solid-js";
import { describe, expect, it } from "vitest";
import {
  openOverlayPanelWithDefaults,
  isOverlayPanelAuthorized,
  overlayPanelClassName,
  overlayPanelTitle,
  sanitizeOverlayPanel,
} from "../src/features/app-shell/controllers/overlay-controller";

describe("app shell overlay controller", () => {
  it("enforces panel authorization boundaries", () => {
    const noAccess = {
      canAccessActiveChannel: false,
      canManageWorkspaceChannels: false,
      hasRoleManagementAccess: false,
      hasModerationAccess: false,
    };

    expect(isOverlayPanelAuthorized("workspace-create", noAccess)).toBe(true);
    expect(isOverlayPanelAuthorized("channel-create", noAccess)).toBe(false);
    expect(isOverlayPanelAuthorized("search", noAccess)).toBe(false);
    expect(isOverlayPanelAuthorized("moderation", noAccess)).toBe(false);
    expect(isOverlayPanelAuthorized("role-management", noAccess)).toBe(false);
  });

  it("sanitizes unauthorized panels to null", () => {
    expect(
      sanitizeOverlayPanel("channel-create", {
        canAccessActiveChannel: true,
        canManageWorkspaceChannels: false,
        hasRoleManagementAccess: true,
        hasModerationAccess: true,
      }),
    ).toBeNull();

    expect(
      sanitizeOverlayPanel("utility", {
        canAccessActiveChannel: false,
        canManageWorkspaceChannels: false,
        hasRoleManagementAccess: false,
        hasModerationAccess: false,
      }),
    ).toBe("utility");
  });

  it("keeps panel title and class mappings stable", () => {
    expect(overlayPanelTitle("public-directory")).toBe("Public workspace directory");
    expect(overlayPanelTitle("role-management")).toBe("Role management");
    expect(overlayPanelTitle("client-settings")).toBe("Client settings");
    expect(overlayPanelTitle("workspace-settings")).toBe("Workspace settings");
    expect(overlayPanelClassName("workspace-create")).toBe("panel-window panel-window-compact");
    expect(overlayPanelClassName("client-settings")).toBe("panel-window panel-window-wide");
    expect(overlayPanelClassName("workspace-settings")).toBe("panel-window panel-window-wide");
    expect(overlayPanelClassName("utility")).toBe("panel-window");
  });

  it("applies deterministic defaults when opening panels", () => {
    const [panel, setPanel] = createSignal<ReturnType<typeof sanitizeOverlayPanel>>(null);
    const [workspaceError, setWorkspaceError] = createSignal("workspace-error");
    const [channelError, setChannelError] = createSignal("channel-error");
    const [settingsCategory, setSettingsCategory] = createSignal<"voice" | "profile">("profile");
    const [settingsSubmenu, setSettingsSubmenu] = createSignal<"audio-devices">("audio-devices");

    openOverlayPanelWithDefaults("workspace-create", {
      setPanel,
      setWorkspaceError,
      setChannelCreateError: setChannelError,
      setActiveSettingsCategory: setSettingsCategory,
      setActiveVoiceSettingsSubmenu: setSettingsSubmenu,
    });
    expect(panel()).toBe("workspace-create");
    expect(workspaceError()).toBe("");

    openOverlayPanelWithDefaults("channel-create", {
      setPanel,
      setWorkspaceError,
      setChannelCreateError: setChannelError,
      setActiveSettingsCategory: setSettingsCategory,
      setActiveVoiceSettingsSubmenu: setSettingsSubmenu,
    });
    expect(panel()).toBe("channel-create");
    expect(channelError()).toBe("");

    openOverlayPanelWithDefaults("client-settings", {
      setPanel,
      setWorkspaceError,
      setChannelCreateError: setChannelError,
      setActiveSettingsCategory: setSettingsCategory,
      setActiveVoiceSettingsSubmenu: setSettingsSubmenu,
    });
    expect(panel()).toBe("client-settings");
    expect(settingsCategory()).toBe("voice");
    expect(settingsSubmenu()).toBe("audio-devices");
  });
});
