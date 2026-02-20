import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  channelIdFromInput,
  channelNameFromInput,
  guildIdFromInput,
  guildNameFromInput,
  type WorkspaceRecord,
} from "../src/domain/chat";
import { ServerRail, type ServerRailProps } from "../src/features/app-shell/components/ServerRail";

const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");

function workspaceFixture(): WorkspaceRecord {
  return {
    guildId: GUILD_ID,
    guildName: guildNameFromInput("Security Ops"),
    visibility: "private",
    channels: [
      {
        channelId: CHANNEL_ID,
        name: channelNameFromInput("incident-room"),
        kind: "text",
      },
    ],
  };
}

function serverRailPropsFixture(
  overrides: Partial<ServerRailProps> = {},
): ServerRailProps {
  return {
    workspaces: [workspaceFixture()],
    activeGuildId: null,
    isCreatingWorkspace: false,
    onSelectWorkspace: () => undefined,
    onOpenPanel: () => undefined,
    ...overrides,
  };
}

describe("app shell server rail", () => {
  it("renders with Uno utility classes and without legacy internal hooks", () => {
    render(() => (
      <ServerRail
        {...serverRailPropsFixture({
          activeGuildId: GUILD_ID,
        })}
      />
    ));

    const rail = document.querySelector("aside.server-rail");
    expect(rail).not.toBeNull();
    expect(rail).toHaveClass("grid");
    expect(rail).toHaveClass("grid-rows-[auto_minmax(0,1fr)_auto]");
    expect(rail).toHaveClass("justify-items-center");
    expect(rail).toHaveClass("px-[0.08rem]");
    expect(rail).toHaveClass("bg-bg-1");

    const workspaceButton = screen.getByRole("button", { name: "S" });
    expect(workspaceButton).toHaveClass("h-[2.72rem]");
    expect(workspaceButton).toHaveClass("w-[2.72rem]");
    expect(workspaceButton).toHaveClass("bg-brand");
    expect(workspaceButton).toHaveClass("rounded-[0.9rem]");
    expect(workspaceButton).toHaveClass("border-brand");

    const actionButton = screen.getByRole("button", {
      name: "Open public workspace directory panel",
    });
    expect(actionButton).toHaveClass("bg-bg-2");
    expect(actionButton).toHaveClass("hover:bg-bg-3");

    expect(document.querySelector(".rail-label")).toBeNull();
    expect(document.querySelector(".server-list")).toBeNull();
    expect(document.querySelector(".server-rail-footer")).toBeNull();
    expect(document.querySelector(".server-action")).toBeNull();
  });

  it("routes workspace and panel actions while honoring create-workspace disabling", async () => {
    const onSelectWorkspace = vi.fn();
    const onOpenPanel = vi.fn();

    render(() => (
      <ServerRail
        {...serverRailPropsFixture({
          isCreatingWorkspace: true,
          onSelectWorkspace,
          onOpenPanel,
        })}
      />
    ));

    await fireEvent.click(screen.getByRole("button", { name: "S" }));
    expect(onSelectWorkspace).toHaveBeenCalledOnce();
    expect(onSelectWorkspace).toHaveBeenCalledWith(GUILD_ID, CHANNEL_ID);

    const createButton = screen.getByRole("button", {
      name: "Open workspace create panel",
    });
    expect(createButton).toBeDisabled();
    await fireEvent.click(createButton);
    expect(onOpenPanel).not.toHaveBeenCalled();

    await fireEvent.click(
      screen.getByRole("button", {
        name: "Open friendships panel",
      }),
    );
    expect(onOpenPanel).toHaveBeenCalledWith("friendships");
  });
});
