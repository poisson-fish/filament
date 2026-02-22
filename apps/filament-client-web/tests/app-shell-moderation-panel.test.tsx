import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  ModerationPanel,
  type ModerationPanelProps,
} from "../src/features/app-shell/components/panels/ModerationPanel";

function moderationPanelPropsFixture(
  overrides: Partial<ModerationPanelProps> = {},
): ModerationPanelProps {
  return {
    moderationUserIdInput: "01ARZ3NDEKTSV4RRFFQ69G5FAA",
    moderationRoleInput: "member",
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
        updatedAtUnix: 100,
      },
      {
        role: "moderator",
        label: "moderator",
        hasExplicitOverride: false,
        allow: [],
        deny: [],
        updatedAtUnix: null,
      },
      {
        role: "owner",
        label: "owner",
        hasExplicitOverride: false,
        allow: [],
        deny: [],
        updatedAtUnix: null,
      },
    ],
    channelOverrideEffectivePermissions: {
      member: ["create_message"],
      moderator: ["create_message", "delete_message"],
      owner: ["manage_roles"],
    },
    isModerating: false,
    hasActiveWorkspace: true,
    hasActiveChannel: true,
    canManageRoles: true,
    canBanMembers: true,
    canManageChannelOverrides: true,
    moderationStatus: "",
    moderationError: "",
    onModerationUserIdInput: () => undefined,
    onModerationRoleChange: () => undefined,
    onRunMemberAction: () => undefined,
    onOverrideRoleChange: () => undefined,
    onOverrideAllowInput: () => undefined,
    onOverrideDenyInput: () => undefined,
    onApplyOverride: () => undefined,
    onOpenRoleManagementPanel: () => undefined,
    ...overrides,
  };
}

describe("app shell moderation panel", () => {
  it("renders utility classes without legacy helper hooks", () => {
    render(() =>
      <ModerationPanel
        {...moderationPanelPropsFixture({
          moderationStatus: "updated",
          moderationError: "forbidden",
        })}
      />,
    );

    expect(screen.getByLabelText("Target user ULID")).toHaveClass("border-line-soft");
    expect(screen.getByLabelText("Role")).toHaveClass("border-line-soft");
    expect(screen.getByRole("radio", { name: "Create Messages: Inherit" })).toHaveClass(
      "border-line-soft",
    );
    expect(screen.getByRole("button", { name: "Add" })).toHaveClass("flex-1");
    expect(screen.getByRole("button", { name: "Apply channel override" })).toHaveClass(
      "border-line-soft",
    );
    expect(
      screen.getAllByText(
        "/ Inherit keeps workspace defaults. ✓ Allow grants this permission in this channel. X Deny blocks this permission in this channel.",
      ).length,
    ).toBeGreaterThan(0);
    expect(screen.getByText("updated")).toHaveClass("text-ok");
    expect(screen.getByText("forbidden")).toHaveClass("text-danger");

    expect(document.querySelector(".member-group")).toBeNull();
    expect(document.querySelector(".inline-form")).toBeNull();
    expect(document.querySelector(".button-row")).toBeNull();
    expect(document.querySelector(".status")).toBeNull();
  });

  it("keeps moderation actions and override callbacks wired", async () => {
    const onModerationUserIdInput = vi.fn();
    const onModerationRoleChange = vi.fn();
    const onRunMemberAction = vi.fn();
    const onOverrideRoleChange = vi.fn();
    const onOverrideAllowInput = vi.fn();
    const onOverrideDenyInput = vi.fn();
    const onApplyOverride = vi.fn((event: SubmitEvent) => event.preventDefault());
    const onOpenRoleManagementPanel = vi.fn();

    render(() =>
      <ModerationPanel
        {...moderationPanelPropsFixture({
          onModerationUserIdInput,
          onModerationRoleChange,
          onRunMemberAction,
          onOverrideRoleChange,
          onOverrideAllowInput,
          onOverrideDenyInput,
          onApplyOverride,
          onOpenRoleManagementPanel,
        })}
      />,
    );

    await fireEvent.input(screen.getByLabelText("Target user ULID"), {
      target: { value: "01ARZ3NDEKTSV4RRFFQ69G5FAB" },
    });
    expect(onModerationUserIdInput).toHaveBeenCalledWith("01ARZ3NDEKTSV4RRFFQ69G5FAB");

    await fireEvent.change(screen.getByLabelText("Role"), {
      target: { value: "moderator" },
    });
    expect(onModerationRoleChange).toHaveBeenCalledWith("moderator");

    await fireEvent.click(screen.getByRole("button", { name: "Add" }));
    await fireEvent.click(screen.getByRole("button", { name: "Set Role" }));
    await fireEvent.click(screen.getByRole("button", { name: "Kick" }));
    await fireEvent.click(screen.getByRole("button", { name: "Ban" }));
    expect(onRunMemberAction).toHaveBeenNthCalledWith(1, "add");
    expect(onRunMemberAction).toHaveBeenNthCalledWith(2, "role");
    expect(onRunMemberAction).toHaveBeenNthCalledWith(3, "kick");
    expect(onRunMemberAction).toHaveBeenNthCalledWith(4, "ban");

    await fireEvent.click(screen.getByRole("button", { name: /owner/i }));
    expect(onOverrideRoleChange).toHaveBeenCalledWith("owner");

    await fireEvent.click(screen.getByRole("radio", { name: "Create Messages: Deny" }));
    expect(onOverrideAllowInput).toHaveBeenLastCalledWith("");
    expect(onOverrideDenyInput).toHaveBeenLastCalledWith("delete_message,create_message");

    await fireEvent.click(screen.getByRole("radio", { name: "Delete Messages: Inherit" }));
    expect(onOverrideAllowInput).toHaveBeenLastCalledWith("create_message");
    expect(onOverrideDenyInput).toHaveBeenLastCalledWith("");

    await fireEvent.click(screen.getByRole("radio", { name: "Manage Overrides: Allow" }));
    expect(onOverrideAllowInput).toHaveBeenLastCalledWith(
      "manage_channel_overrides,create_message",
    );
    expect(onOverrideDenyInput).toHaveBeenLastCalledWith("delete_message");

    const overrideForm = screen
      .getByRole("button", { name: "Apply channel override" })
      .closest("form");
    expect(overrideForm).not.toBeNull();
    await fireEvent.submit(overrideForm!);
    expect(onApplyOverride).toHaveBeenCalledTimes(1);

    await fireEvent.click(screen.getByRole("button", { name: "Open role management panel" }));
    expect(onOpenRoleManagementPanel).toHaveBeenCalledTimes(1);
  });

  it("renders entity selector with @everyone first and marks active overrides", () => {
    render(() =>
      <ModerationPanel
        {...moderationPanelPropsFixture({
          overrideRoleInput: "moderator",
          channelOverrideEntities: [
            {
              role: "member",
              label: "@everyone",
              hasExplicitOverride: false,
              allow: [],
              deny: [],
              updatedAtUnix: null,
            },
            {
              role: "owner",
              label: "owner",
              hasExplicitOverride: true,
              allow: ["manage_channel_overrides"],
              deny: [],
              updatedAtUnix: 300,
            },
            {
              role: "moderator",
              label: "moderator",
              hasExplicitOverride: true,
              allow: ["create_message"],
              deny: [],
              updatedAtUnix: 200,
            },
          ],
        })}
      />,
    );

    const entities = screen
      .getByLabelText("Channel override entities")
      .querySelectorAll("button");
    expect(entities[0]).toHaveTextContent("@everyone");
    expect(screen.getAllByText("active override")).toHaveLength(2);
    expect(screen.getByRole("button", { name: /moderator/i })).toHaveAttribute(
      "aria-pressed",
      "true",
    );
    expect(screen.getByRole("radio", { name: "Create Messages: Allow" })).toHaveAttribute(
      "aria-checked",
      "true",
    );
    expect(screen.getByLabelText("Create Messages effective allowed")).toBeInTheDocument();
  });

  it("shows denied effective indicator when selected role lacks permission", () => {
    render(() =>
      <ModerationPanel
        {...moderationPanelPropsFixture({
          overrideRoleInput: "member",
          channelOverrideEffectivePermissions: {
            member: [],
            moderator: ["create_message"],
            owner: ["manage_roles"],
          },
        })}
      />,
    );

    expect(screen.getByLabelText("Create Messages effective denied")).toBeInTheDocument();
    expect(screen.getByLabelText("Delete Messages effective denied")).toBeInTheDocument();
  });

  it("hides gated controls when moderation permissions are missing", () => {
    render(() =>
      <ModerationPanel
        {...moderationPanelPropsFixture({
          canManageRoles: false,
          canBanMembers: false,
          canManageChannelOverrides: false,
        })}
      />,
    );

    expect(screen.queryByRole("button", { name: "Add" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Kick" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Apply channel override" })).toBeNull();
    expect(
      screen.getByRole("button", { name: "Open role management panel" }),
    ).toBeInTheDocument();
  });
});
