import { createEffect, onCleanup, type Accessor, type Setter } from "solid-js";
import {
  DEFAULT_SETTINGS_CATEGORY,
  DEFAULT_VOICE_SETTINGS_SUBMENU,
} from "../config/settings-menu";
import type {
  OverlayPanel,
  SettingsCategory,
  VoiceSettingsSubmenu,
} from "../types";

export interface OverlayAuthorizationContext {
  canAccessActiveChannel: boolean;
  canManageWorkspaceChannels: boolean;
  hasRoleManagementAccess: boolean;
  hasModerationAccess: boolean;
}

export interface OverlayPanelOpenOptions {
  setPanel: Setter<OverlayPanel | null>;
  setWorkspaceError: Setter<string>;
  setChannelCreateError: Setter<string>;
  setActiveSettingsCategory: Setter<SettingsCategory>;
  setActiveVoiceSettingsSubmenu: Setter<VoiceSettingsSubmenu>;
}

export interface OverlayPanelAuthorizationControllerOptions {
  panel: Accessor<OverlayPanel | null>;
  context: Accessor<OverlayAuthorizationContext>;
  setPanel: Setter<OverlayPanel | null>;
}

export interface OverlayPanelEscapeControllerOptions {
  panel: Accessor<OverlayPanel | null>;
  onEscape: () => void;
}

export function overlayPanelTitle(panel: OverlayPanel): string {
  switch (panel) {
    case "workspace-create":
      return "Create workspace";
    case "channel-create":
      return "Create channel";
    case "client-settings":
      return "Client settings";
    case "workspace-settings":
      return "Workspace settings";
    case "public-directory":
      return "Public workspace directory";
    case "friendships":
      return "Friendships";
    case "search":
      return "Search";
    case "attachments":
      return "Attachments";
    case "moderation":
      return "Moderation";
    case "role-management":
      return "Role management";
    case "utility":
      return "Utility";
  }
}

export function overlayPanelClassName(panel: OverlayPanel): string {
  if (panel === "workspace-create" || panel === "channel-create") {
    return "panel-window panel-window-compact";
  }
  if (panel === "client-settings" || panel === "workspace-settings") {
    return "panel-window panel-window-wide";
  }
  if (
    panel === "public-directory" ||
    panel === "friendships"
  ) {
    return "panel-window panel-window-medium";
  }
  return "panel-window";
}

export function isOverlayPanelAuthorized(
  panel: OverlayPanel,
  context: OverlayAuthorizationContext,
): boolean {
  if (panel === "channel-create") {
    return context.canManageWorkspaceChannels;
  }
  if (panel === "moderation") {
    return context.hasModerationAccess;
  }
  if (panel === "role-management") {
    return context.hasRoleManagementAccess;
  }
  if (panel === "search" || panel === "attachments") {
    return context.canAccessActiveChannel;
  }
  return true;
}

export function sanitizeOverlayPanel(
  panel: OverlayPanel | null,
  context: OverlayAuthorizationContext,
): OverlayPanel | null {
  if (!panel) {
    return null;
  }
  if (!isOverlayPanelAuthorized(panel, context)) {
    return null;
  }
  return panel;
}

export function openOverlayPanelWithDefaults(
  panel: OverlayPanel,
  options: OverlayPanelOpenOptions,
): void {
  if (panel === "workspace-create") {
    options.setWorkspaceError("");
  }
  if (panel === "channel-create") {
    options.setChannelCreateError("");
  }
  if (panel === "client-settings") {
    options.setActiveSettingsCategory(DEFAULT_SETTINGS_CATEGORY);
    options.setActiveVoiceSettingsSubmenu(DEFAULT_VOICE_SETTINGS_SUBMENU);
  }
  options.setPanel(panel);
}

export function createOverlayPanelAuthorizationController(
  options: OverlayPanelAuthorizationControllerOptions,
): void {
  createEffect(() => {
    const panel = options.panel();
    const sanitized = sanitizeOverlayPanel(panel, options.context());
    if (sanitized !== panel) {
      options.setPanel(sanitized);
    }
  });
}

export function createOverlayPanelEscapeController(
  options: OverlayPanelEscapeControllerOptions,
): void {
  createEffect(() => {
    if (!options.panel()) {
      return;
    }

    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        options.onEscape();
      }
    };

    window.addEventListener("keydown", onKeydown);
    onCleanup(() => window.removeEventListener("keydown", onKeydown));
  });
}
