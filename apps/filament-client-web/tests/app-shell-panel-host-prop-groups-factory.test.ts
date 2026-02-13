import { describe, expect, it, vi } from "vitest";

const runtimeMocks = vi.hoisted(() => ({
  createWorkspaceChannelCreatePanelHostStateOptions: vi.fn(),
  createSupportPanelHostStateOptions: vi.fn(),
  createCollaborationPanelHostStateOptions: vi.fn(),
  createPanelHostPropGroupsOptions: vi.fn(),
  createPanelHostPropGroups: vi.fn(),
}));

vi.mock(
  "../src/features/app-shell/runtime/workspace-channel-create-panel-host-state-options",
  () => ({
    createWorkspaceChannelCreatePanelHostStateOptions:
      runtimeMocks.createWorkspaceChannelCreatePanelHostStateOptions,
  }),
);

vi.mock("../src/features/app-shell/runtime/support-panel-host-state-options", () => ({
  createSupportPanelHostStateOptions:
    runtimeMocks.createSupportPanelHostStateOptions,
}));

vi.mock(
  "../src/features/app-shell/runtime/collaboration-panel-host-state-options",
  () => ({
    createCollaborationPanelHostStateOptions:
      runtimeMocks.createCollaborationPanelHostStateOptions,
  }),
);

vi.mock("../src/features/app-shell/runtime/panel-host-prop-groups-options", () => ({
  createPanelHostPropGroupsOptions:
    runtimeMocks.createPanelHostPropGroupsOptions,
}));

vi.mock("../src/features/app-shell/runtime/panel-host-prop-groups", () => ({
  createPanelHostPropGroups: runtimeMocks.createPanelHostPropGroups,
}));

import { createPanelHostPropGroupsFactory } from "../src/features/app-shell/runtime/panel-host-prop-groups-factory";

describe("app shell panel-host prop groups factory", () => {
  it("builds panel-host prop groups from host-state option builders", () => {
    const workspaceChannelCreateStateOptions = {
      workspaceChannelState: {},
      selectors: {},
      workspaceChannelOperations: {},
      closeOverlayPanel: vi.fn(),
    };

    const supportStateOptions = {
      discoveryState: {},
      overlayState: {},
      voiceState: {},
      profileState: {},
      workspaceChannelState: {},
      diagnosticsState: {},
      selectors: {},
      publicDirectoryActions: {},
      profileController: {},
      roleManagementActions: {},
      sessionDiagnostics: {},
      openSettingsCategory: vi.fn(),
      setVoiceDevicePreference: vi.fn(),
      refreshAudioDeviceInventory: vi.fn(async () => undefined),
      saveWorkspaceSettings: vi.fn(async () => undefined),
      openOverlayPanel: vi.fn(),
    };

    const collaborationStateOptions = {
      friendshipsState: {},
      discoveryState: {},
      messageState: {},
      diagnosticsState: {},
      selectors: {},
      friendshipActions: {},
      searchActions: {},
      attachmentActions: {},
      moderationActions: {},
      labels: {},
      openOverlayPanel: vi.fn(),
    };

    const workspaceChannelCreate = { workspaceCreate: {}, channelCreate: {} };
    const support = {
      publicDirectory: {},
      settings: {},
      workspaceSettings: {},
      roleManagement: {},
      utility: {},
    };
    const collaboration = {
      friendships: {},
      search: {},
      attachments: {},
      moderation: {},
    };
    const groupedStateOptions = {
      workspaceChannelCreate,
      support,
      collaboration,
    };
    const groupedProps = { workspaceCreate: {}, channelCreate: {} };

    runtimeMocks.createWorkspaceChannelCreatePanelHostStateOptions.mockReturnValue(
      workspaceChannelCreate,
    );
    runtimeMocks.createSupportPanelHostStateOptions.mockReturnValue(support);
    runtimeMocks.createCollaborationPanelHostStateOptions.mockReturnValue(
      collaboration,
    );
    runtimeMocks.createPanelHostPropGroupsOptions.mockReturnValue(
      groupedStateOptions,
    );
    runtimeMocks.createPanelHostPropGroups.mockReturnValue(groupedProps);

    const buildPanelHostPropGroups = createPanelHostPropGroupsFactory({
      workspaceChannelCreate:
        workspaceChannelCreateStateOptions as unknown as Parameters<
          typeof createPanelHostPropGroupsFactory
        >[0]["workspaceChannelCreate"],
      support: supportStateOptions as unknown as Parameters<
        typeof createPanelHostPropGroupsFactory
      >[0]["support"],
      collaboration: collaborationStateOptions as unknown as Parameters<
        typeof createPanelHostPropGroupsFactory
      >[0]["collaboration"],
    });

    const result = buildPanelHostPropGroups();

    expect(
      runtimeMocks.createWorkspaceChannelCreatePanelHostStateOptions,
    ).toHaveBeenCalledWith(workspaceChannelCreateStateOptions);
    expect(runtimeMocks.createSupportPanelHostStateOptions).toHaveBeenCalledWith(
      supportStateOptions,
    );
    expect(
      runtimeMocks.createCollaborationPanelHostStateOptions,
    ).toHaveBeenCalledWith(collaborationStateOptions);
    expect(runtimeMocks.createPanelHostPropGroupsOptions).toHaveBeenCalledWith({
      workspaceChannelCreate,
      support,
      collaboration,
    });
    expect(runtimeMocks.createPanelHostPropGroups).toHaveBeenCalledWith(
      groupedStateOptions,
    );
    expect(result).toBe(groupedProps);
  });
});