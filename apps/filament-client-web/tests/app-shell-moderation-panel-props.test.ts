import { describe, expect, it, vi } from "vitest";
import { createModerationPanelProps } from "../src/features/app-shell/runtime/moderation-panel-props";

describe("app shell moderation panel props", () => {
  it("maps moderation values and handlers", async () => {
    const setModerationUserIdInput = vi.fn();
    const setModerationRoleInput = vi.fn();
    const onRunMemberAction = vi.fn();
    const setOverrideRoleInput = vi.fn();
    const setOverrideAllowCsv = vi.fn();
    const setOverrideDenyCsv = vi.fn();
    const onApplyOverride = vi.fn();
    const onOpenRoleManagementPanel = vi.fn();

    const panelProps = createModerationPanelProps({
      moderationUserIdInput: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
      moderationRoleInput: "moderator",
      overrideRoleInput: "member",
      overrideAllowCsv: "create_message",
      overrideDenyCsv: "delete_message",
      channelOverrideEntities: [
        {
          role: "member",
          label: "@everyone",
          hasExplicitOverride: true,
          allow: ["create_message"],
          deny: [],
          updatedAtUnix: 123,
        },
      ],
      channelOverrideEffectivePermissions: {
        member: ["create_message"],
        moderator: ["delete_message"],
        owner: ["manage_roles"],
      },
      isModerating: false,
      hasActiveWorkspace: true,
      hasActiveChannel: true,
      canManageRoles: true,
      canBanMembers: true,
      canManageChannelOverrides: true,
      moderationStatus: "ready",
      moderationError: "",
      setModerationUserIdInput,
      setModerationRoleInput,
      onRunMemberAction,
      setOverrideRoleInput,
      setOverrideAllowCsv,
      setOverrideDenyCsv,
      onApplyOverride,
      onOpenRoleManagementPanel,
    });

    expect(panelProps.hasActiveWorkspace).toBe(true);
    expect(panelProps.hasActiveChannel).toBe(true);
    expect(panelProps.canManageRoles).toBe(true);
    expect(panelProps.overrideAllowCsv).toBe("create_message");
    expect(panelProps.channelOverrideEntities).toHaveLength(1);
    expect(panelProps.channelOverrideEffectivePermissions.member).toEqual(["create_message"]);

    panelProps.setModerationUserIdInput("01ARZ3NDEKTSV4RRFFQ69G5FAB");
    expect(setModerationUserIdInput).toHaveBeenCalledWith(
      "01ARZ3NDEKTSV4RRFFQ69G5FAB",
    );

    panelProps.setModerationRoleInput("member");
    expect(setModerationRoleInput).toHaveBeenCalledTimes(1);

    await panelProps.onRunMemberAction("ban");
    expect(onRunMemberAction).toHaveBeenCalledWith("ban");

    panelProps.setOverrideRoleInput("moderator");
    expect(setOverrideRoleInput).toHaveBeenCalledTimes(1);

    panelProps.setOverrideAllowCsv("manage_workspace_roles");
    expect(setOverrideAllowCsv).toHaveBeenCalledWith("manage_workspace_roles");

    panelProps.setOverrideDenyCsv("kick_member");
    expect(setOverrideDenyCsv).toHaveBeenCalledWith("kick_member");

    const submitEvent = {
      preventDefault: vi.fn(),
    } as unknown as SubmitEvent;

    await panelProps.onApplyOverride(submitEvent);
    expect(onApplyOverride).toHaveBeenCalledWith(submitEvent);

    panelProps.onOpenRoleManagementPanel();
    expect(onOpenRoleManagementPanel).toHaveBeenCalledTimes(1);
  });
});
