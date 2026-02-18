import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import { MemberRail } from "../src/features/app-shell/components/MemberRail";

function memberRailPropsFixture(
  overrides: Partial<Parameters<typeof MemberRail>[0]> = {},
): Parameters<typeof MemberRail>[0] {
  return {
    profileLoading: false,
    profileErrorText: "",
    profile: {
      userId: "01ARZ3NDEKTSV4RRFFQ69G5FAZ",
      username: "operator",
    },
    showUnauthorizedWorkspaceNote: false,
    canAccessActiveChannel: true,
    hasRoleManagementAccess: true,
    onlineMembers: ["remote.user"],
    hasModerationAccess: true,
    displayUserLabel: (userId) => (userId === "remote.user" ? "Remote User" : userId),
    onOpenPanel: () => undefined,
    ...overrides,
  };
}

describe("app shell member rail", () => {
  it("renders with Uno utility classes and no legacy internal class hooks", () => {
    render(() => <MemberRail {...memberRailPropsFixture()} />);

    const rail = document.querySelector("aside.member-rail");
    expect(rail).not.toBeNull();
    expect(rail).toHaveClass("grid");
    expect(rail).toHaveClass("bg-bg-0");

    expect(screen.getByRole("heading", { name: "Workspace Tools" })).toHaveClass("m-0");
    expect(screen.getByText("Username")).toHaveClass("uppercase");

    const moderationButton = screen.getByRole("button", {
      name: "Open moderation panel",
    });
    expect(moderationButton).toHaveClass("rounded-[0.62rem]");
    expect(moderationButton).toHaveClass("enabled:hover:bg-bg-4");
    expect(screen.getByText("Remote User").previousElementSibling).toHaveClass("bg-presence-online");

    expect(document.querySelector(".profile-card")).toBeNull();
    expect(document.querySelector(".member-group")).toBeNull();
    expect(document.querySelector(".presence")).toBeNull();
    expect(document.querySelector(".group-label")).toBeNull();
    expect(document.querySelector(".ops-launch-grid")).toBeNull();
  });

  it("keeps panel launch callbacks and visibility gates intact", async () => {
    const onOpenPanel = vi.fn();

    render(() => (
      <MemberRail
        {...memberRailPropsFixture({
          onOpenPanel,
          profile: null,
          canAccessActiveChannel: false,
          hasRoleManagementAccess: false,
          hasModerationAccess: false,
          showUnauthorizedWorkspaceNote: true,
          onlineMembers: [],
        })}
      />
    ));

    expect(
      screen.getByText("No authorized workspace/channel selected for operator actions."),
    ).toBeInTheDocument();

    expect(screen.queryByText("ONLINE (0)")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open search panel" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open attachments panel" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open moderation panel" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Open role management panel" })).not.toBeInTheDocument();

    await fireEvent.click(screen.getByRole("button", { name: "Open directory panel" }));
    await fireEvent.click(screen.getByRole("button", { name: "Open friendships panel" }));
    await fireEvent.click(screen.getByRole("button", { name: "Open utility panel" }));

    expect(onOpenPanel.mock.calls).toEqual([
      ["public-directory"],
      ["friendships"],
      ["utility"],
    ]);
  });
});
