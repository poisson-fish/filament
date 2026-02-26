import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import { userIdFromInput, type ProfileRecord } from "../src/domain/chat";
import { UserProfileOverlay, type UserProfileOverlayProps } from "../src/features/app-shell/components/overlays/UserProfileOverlay";

const PROFILE_USER_ID = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAZ");

function profileFixture(): ProfileRecord {
  return {
    userId: PROFILE_USER_ID,
    username: "owner",
    aboutMarkdown: "hello\n\nteam",
    aboutMarkdownTokens: [
      { type: "paragraph_start" },
      { type: "text", text: "hello" },
      { type: "paragraph_end" },
      { type: "paragraph_start" },
      { type: "text", text: "team" },
      { type: "paragraph_end" },
    ],
    avatarVersion: 1,
    bannerVersion: 1,
  };
}

function overlayPropsFixture(
  overrides: Partial<UserProfileOverlayProps> = {},
): UserProfileOverlayProps {
  return {
    selectedProfileUserId: PROFILE_USER_ID,
    selectedProfileLoading: false,
    selectedProfileError: "",
    selectedProfile: profileFixture(),
    avatarUrlForUser: () => null,
    bannerUrlForUser: () => null,
    onClose: () => undefined,
    ...overrides,
  };
}

describe("app shell user profile overlay", () => {
  it("renders with Uno utility classes and no legacy profile-view hooks", () => {
    render(() => <UserProfileOverlay {...overlayPropsFixture()} />);

    const backdrop = screen.getByRole("presentation");
    expect(backdrop).toHaveClass("fixed");
    expect(backdrop).toHaveClass("bg-black/72");

    const dialog = screen.getByRole("dialog", { name: "User profile panel" });
    expect(dialog).toHaveClass("max-w-[28rem]");
    expect(dialog).toHaveClass("bg-bg-1");
    expect(dialog).toHaveClass("shadow-panel");

    const closeButton = screen.getByRole("button", { name: "Close" });
    expect(closeButton).toHaveClass("rounded-[0.58rem]");
    expect(closeButton).toHaveClass("enabled:hover:bg-bg-4");

    expect(screen.getByText("owner")).toHaveClass("text-[1rem]");
    expect(screen.getByText(PROFILE_USER_ID)).toHaveClass("font-code");
    expect(screen.getByText("hello").parentElement).toHaveClass("text-ink-1");

    expect(document.querySelector(".profile-view-panel")).toBeNull();
    expect(document.querySelector(".profile-view-body")).toBeNull();
    expect(document.querySelector(".profile-view-header")).toBeNull();
    expect(document.querySelector(".profile-view-avatar")).toBeNull();
    expect(document.querySelector(".profile-view-markdown")).toBeNull();
  });

  it("keeps loading/error visibility, close interactions, and image fallback behavior", async () => {
    const onClose = vi.fn();
    const avatarUrl = "https://example.test/avatar.png";
    const bannerUrl = "https://example.test/banner.png";

    const first = render(() => (
      <UserProfileOverlay
        {...overlayPropsFixture({
          selectedProfileLoading: true,
          selectedProfile: null,
          onClose,
        })}
      />
    ));

    expect(screen.getByText("Loading profile...")).toBeInTheDocument();
    first.unmount();

    render(() => (
      <UserProfileOverlay
        {...overlayPropsFixture({
          selectedProfileError: "Profile unavailable.",
          avatarUrlForUser: () => avatarUrl,
          bannerUrlForUser: () => bannerUrl,
          onClose,
        })}
      />
    ));

    expect(screen.getByText("Profile unavailable.")).toBeInTheDocument();
    const bannerImage = screen.getByAltText("owner banner");
    await fireEvent.error(bannerImage);
    expect(bannerImage).toHaveStyle({ display: "none" });
    const avatarImage = screen.getByAltText("owner avatar");
    await fireEvent.error(avatarImage);
    expect(avatarImage).toHaveStyle({ display: "none" });

    await fireEvent.click(screen.getByRole("button", { name: "Close" }));
    await fireEvent.click(screen.getByRole("presentation"));
    expect(onClose).toHaveBeenCalledTimes(2);
  });
});
