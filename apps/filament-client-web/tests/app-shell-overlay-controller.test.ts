import { describe, expect, it } from "vitest";
import {
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
      hasModerationAccess: false,
    };

    expect(isOverlayPanelAuthorized("workspace-create", noAccess)).toBe(true);
    expect(isOverlayPanelAuthorized("channel-create", noAccess)).toBe(false);
    expect(isOverlayPanelAuthorized("search", noAccess)).toBe(false);
    expect(isOverlayPanelAuthorized("moderation", noAccess)).toBe(false);
  });

  it("sanitizes unauthorized panels to null", () => {
    expect(
      sanitizeOverlayPanel("channel-create", {
        canAccessActiveChannel: true,
        canManageWorkspaceChannels: false,
        hasModerationAccess: true,
      }),
    ).toBeNull();

    expect(
      sanitizeOverlayPanel("utility", {
        canAccessActiveChannel: false,
        canManageWorkspaceChannels: false,
        hasModerationAccess: false,
      }),
    ).toBe("utility");
  });

  it("keeps panel title and class mappings stable", () => {
    expect(overlayPanelTitle("public-directory")).toBe("Public workspace directory");
    expect(overlayPanelClassName("workspace-create")).toBe("panel-window panel-window-compact");
    expect(overlayPanelClassName("settings")).toBe("panel-window panel-window-medium");
    expect(overlayPanelClassName("utility")).toBe("panel-window");
  });
});
