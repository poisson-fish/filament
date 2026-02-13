import { describe, expect, it, vi } from "vitest";

const runtimeMocks = vi.hoisted(() => ({
  createWorkspaceChannelCreatePanelGroupsOptions: vi.fn(),
  createSupportPanelPropGroupsOptions: vi.fn(),
  createCollaborationPanelPropGroupsOptions: vi.fn(),
}));

vi.mock(
  "../src/features/app-shell/runtime/workspace-channel-create-panel-groups-options",
  () => ({
    createWorkspaceChannelCreatePanelGroupsOptions:
      runtimeMocks.createWorkspaceChannelCreatePanelGroupsOptions,
  }),
);

vi.mock("../src/features/app-shell/runtime/support-panel-prop-groups-options", () => ({
  createSupportPanelPropGroupsOptions:
    runtimeMocks.createSupportPanelPropGroupsOptions,
}));

vi.mock(
  "../src/features/app-shell/runtime/collaboration-panel-prop-groups-options",
  () => ({
    createCollaborationPanelPropGroupsOptions:
      runtimeMocks.createCollaborationPanelPropGroupsOptions,
  }),
);

import { createPanelHostPropGroupsOptions } from "../src/features/app-shell/runtime/panel-host-prop-groups-options";

describe("app shell panel-host prop group state options", () => {
  it("builds grouped panel-host options from domain option builders", () => {
    const workspaceChannelCreateStateOptions = {
      createGuildName: () => "",
    };
    const supportStateOptions = {
      publicGuildSearchQuery: () => "",
    };
    const collaborationStateOptions = {
      friendRecipientUserIdInput: () => "",
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

    runtimeMocks.createWorkspaceChannelCreatePanelGroupsOptions.mockReturnValue(
      workspaceChannelCreate,
    );
    runtimeMocks.createSupportPanelPropGroupsOptions.mockReturnValue(support);
    runtimeMocks.createCollaborationPanelPropGroupsOptions.mockReturnValue(
      collaboration,
    );

    const options = createPanelHostPropGroupsOptions({
      workspaceChannelCreate:
        workspaceChannelCreateStateOptions as Parameters<
          typeof createPanelHostPropGroupsOptions
        >[0]["workspaceChannelCreate"],
      support: supportStateOptions as Parameters<
        typeof createPanelHostPropGroupsOptions
      >[0]["support"],
      collaboration: collaborationStateOptions as Parameters<
        typeof createPanelHostPropGroupsOptions
      >[0]["collaboration"],
    });

    expect(
      runtimeMocks.createWorkspaceChannelCreatePanelGroupsOptions,
    ).toHaveBeenCalledWith(workspaceChannelCreateStateOptions);
    expect(runtimeMocks.createSupportPanelPropGroupsOptions).toHaveBeenCalledWith(
      supportStateOptions,
    );
    expect(
      runtimeMocks.createCollaborationPanelPropGroupsOptions,
    ).toHaveBeenCalledWith(collaborationStateOptions);

    expect(options).toEqual({
      workspaceChannelCreate,
      support,
      collaboration,
    });
  });
});