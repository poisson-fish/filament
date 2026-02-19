import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  channelIdFromInput,
  channelNameFromInput,
  type ChannelRecord,
} from "../src/domain/chat";
import { ChatHeader } from "../src/features/app-shell/components/ChatHeader";

const CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");

function channelFixture(kind: "text" | "voice"): ChannelRecord {
  return {
    channelId: CHANNEL_ID,
    name: channelNameFromInput(kind === "voice" ? "war-room" : "incident-room"),
    kind,
  };
}

function chatHeaderPropsFixture(
  overrides: Partial<Parameters<typeof ChatHeader>[0]> = {},
): Parameters<typeof ChatHeader>[0] {
  return {
    activeChannel: channelFixture("text"),
    gatewayOnline: true,
    canShowVoiceHeaderControls: true,
    isVoiceSessionActive: false,
    voiceConnectionState: "connected",
    isChannelRailCollapsed: false,
    isMemberRailCollapsed: false,
    isRefreshingSession: false,
    onToggleChannelRail: () => undefined,
    onToggleMemberRail: () => undefined,
    onOpenPanel: () => undefined,
    onRefreshMessages: () => undefined,
    onRefreshSession: () => undefined,
    onLogout: () => undefined,
    ...overrides,
  };
}

describe("app shell chat header", () => {
  it("renders with Uno utility classes and without legacy internal hooks", () => {
    render(() => <ChatHeader {...chatHeaderPropsFixture()} />);

    const header = document.querySelector("header.chat-header");
    expect(header).not.toBeNull();
    expect(header).toHaveClass("flex");
    expect(header).toHaveClass("border-b");

    const title = screen.getByRole("heading", { name: "incident-room" });
    expect(title).toHaveClass("m-0");
    expect(title).toHaveClass("text-ink-0");
    expect(title.parentElement).toHaveClass("items-center");
    expect(title.previousElementSibling).toHaveClass("icon-mask");

    const gatewayBadge = screen.getByText("Live");
    expect(gatewayBadge).toHaveClass("rounded-full");
    expect(gatewayBadge).toHaveClass("text-bg-0");

    const voiceBadge = screen.getByText("Voice connected");
    expect(voiceBadge).toHaveClass("text-ink-2");
    expect(voiceBadge).toHaveClass("items-center");

    const channelsToggle = screen.getByRole("button", { name: "Hide channels" });
    expect(channelsToggle).toHaveClass("h-[2.1rem]");
    expect(channelsToggle).toHaveClass("hover:bg-bg-4");

    const logoutButton = screen.getByRole("button", { name: "Logout" });
    expect(logoutButton).toHaveClass("bg-danger-panel");
    expect(logoutButton).toHaveClass("text-danger-ink");

    expect(document.querySelector(".header-actions")).toBeNull();
    expect(document.querySelector(".gateway-badge")).toBeNull();
    expect(document.querySelector(".voice-badge")).toBeNull();
    expect(document.querySelector(".header-icon-button")).toBeNull();
    expect(document.querySelector(".logout")).toBeNull();
  });

  it("keeps header actions wired and preserves refresh-session disable semantics", async () => {
    const onToggleChannelRail = vi.fn();
    const onToggleMemberRail = vi.fn();
    const onOpenPanel = vi.fn();
    const onRefreshMessages = vi.fn();
    const onRefreshSession = vi.fn();
    const onLogout = vi.fn();

    const first = render(() => (
      <ChatHeader
        {...chatHeaderPropsFixture({
          onToggleChannelRail,
          onToggleMemberRail,
          onOpenPanel,
          onRefreshMessages,
          onRefreshSession,
          onLogout,
        })}
      />
    ));

    await fireEvent.click(screen.getByRole("button", { name: "Hide channels" }));
    await fireEvent.click(screen.getByRole("button", { name: "Hide workspace tools rail" }));
    await fireEvent.click(screen.getByRole("button", { name: "Directory" }));
    await fireEvent.click(screen.getByRole("button", { name: "Friends" }));
    await fireEvent.click(screen.getByRole("button", { name: "Refresh" }));
    await fireEvent.click(screen.getByRole("button", { name: "Refresh session" }));
    await fireEvent.click(screen.getByRole("button", { name: "Logout" }));

    expect(onToggleChannelRail).toHaveBeenCalledOnce();
    expect(onToggleMemberRail).toHaveBeenCalledOnce();
    expect(onOpenPanel.mock.calls).toEqual([["public-directory"], ["friendships"]]);
    expect(onRefreshMessages).toHaveBeenCalledOnce();
    expect(onRefreshSession).toHaveBeenCalledOnce();
    expect(onLogout).toHaveBeenCalledOnce();

    first.unmount();

    const second = render(() => (
      <ChatHeader
        {...chatHeaderPropsFixture({
          isRefreshingSession: true,
          onRefreshSession,
        })}
      />
    ));

    const disabledRefreshSessionButton = screen.getByRole("button", { name: "Refreshing..." });
    expect(disabledRefreshSessionButton).toBeDisabled();
    await fireEvent.click(disabledRefreshSessionButton);
    expect(onRefreshSession).toHaveBeenCalledTimes(1);

    second.unmount();
  });
});
