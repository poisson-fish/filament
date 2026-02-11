import type { OverlayPanel } from "../types";

export interface OverlayAuthorizationContext {
  canAccessActiveChannel: boolean;
  canManageWorkspaceChannels: boolean;
  hasModerationAccess: boolean;
}

export function overlayPanelTitle(panel: OverlayPanel): string {
  switch (panel) {
    case "workspace-create":
      return "Create workspace";
    case "channel-create":
      return "Create channel";
    case "settings":
      return "Settings";
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
    case "utility":
      return "Utility";
  }
}

export function overlayPanelClassName(panel: OverlayPanel): string {
  if (panel === "workspace-create" || panel === "channel-create") {
    return "panel-window panel-window-compact";
  }
  if (panel === "settings" || panel === "public-directory" || panel === "friendships") {
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
