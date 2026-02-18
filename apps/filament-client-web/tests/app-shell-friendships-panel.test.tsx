import { fireEvent, render, screen } from "@solidjs/testing-library";
import { describe, expect, it, vi } from "vitest";
import {
  friendListFromResponse,
  friendRequestListFromResponse,
  userIdFromInput,
} from "../src/domain/chat";
import {
  FriendshipsPanel,
  type FriendshipsPanelProps,
} from "../src/features/app-shell/components/panels/FriendshipsPanel";

const ALICE_USER_ID = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const BOB_USER_ID = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const CAROL_USER_ID = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");

function friendshipsPanelPropsFixture(
  overrides: Partial<FriendshipsPanelProps> = {},
): FriendshipsPanelProps {
  return {
    friendRecipientUserIdInput: BOB_USER_ID,
    friendRequests: friendRequestListFromResponse({
      incoming: [
        {
          request_id: "01ARZ3NDEKTSV4RRFFQ69G5FAY",
          sender_user_id: BOB_USER_ID,
          sender_username: "bob",
          recipient_user_id: ALICE_USER_ID,
          recipient_username: "alice",
          created_at_unix: 10,
        },
      ],
      outgoing: [
        {
          request_id: "01ARZ3NDEKTSV4RRFFQ69G5FAZ",
          sender_user_id: ALICE_USER_ID,
          sender_username: "alice",
          recipient_user_id: CAROL_USER_ID,
          recipient_username: "carol",
          created_at_unix: 20,
        },
      ],
    }),
    friends: friendListFromResponse({
      friends: [
        {
          user_id: BOB_USER_ID,
          username: "bob",
          created_at_unix: 30,
        },
      ],
    }),
    isRunningFriendAction: false,
    friendStatus: "",
    friendError: "",
    onSubmitFriendRequest: vi.fn((event: SubmitEvent) => event.preventDefault()),
    onFriendRecipientInput: vi.fn(),
    onAcceptIncomingFriendRequest: vi.fn(),
    onDismissFriendRequest: vi.fn(),
    onRemoveFriendship: vi.fn(),
    ...overrides,
  };
}

describe("app shell friendships panel", () => {
  it("renders with Uno utility classes and without legacy internal class hooks", () => {
    render(() => <FriendshipsPanel {...friendshipsPanelPropsFixture()} />);

    const panel = screen.getByLabelText("friendships");
    expect(panel).toHaveClass("public-directory");
    expect(panel).toHaveClass("grid");

    const userIdInput = screen.getByLabelText("User ID");
    expect(userIdInput).toHaveClass("rounded-[0.62rem]");
    expect(userIdInput).toHaveClass("border-line-soft");

    const submitButton = screen.getByRole("button", { name: "Send request" });
    expect(submitButton).toHaveClass("border-brand/45");
    expect(submitButton).toHaveClass("enabled:hover:bg-brand/24");

    const acceptButton = screen.getByRole("button", { name: "Accept" });
    expect(acceptButton).toHaveClass("flex-1");

    const [incomingSenderId] = screen.getAllByText(BOB_USER_ID);
    expect(incomingSenderId).toHaveClass("font-code");
    expect(incomingSenderId).toHaveClass("text-[0.78rem]");

    expect(document.querySelector(".inline-form")).toBeNull();
    expect(document.querySelector(".button-row")).toBeNull();
    expect(document.querySelector(".group-label")).toBeNull();
    expect(document.querySelector(".stacked-meta")).toBeNull();
    expect(document.querySelector(".mono")).toBeNull();
  });

  it("keeps request and friendship actions wired", async () => {
    const onSubmitFriendRequest = vi.fn((event: SubmitEvent) => event.preventDefault());
    const onFriendRecipientInput = vi.fn();
    const onAcceptIncomingFriendRequest = vi.fn();
    const onDismissFriendRequest = vi.fn();
    const onRemoveFriendship = vi.fn();

    render(() => (
      <FriendshipsPanel
        {...friendshipsPanelPropsFixture({
          onSubmitFriendRequest,
          onFriendRecipientInput,
          onAcceptIncomingFriendRequest,
          onDismissFriendRequest,
          onRemoveFriendship,
        })}
      />
    ));

    await fireEvent.input(screen.getByLabelText("User ID"), {
      target: { value: CAROL_USER_ID },
    });
    expect(onFriendRecipientInput).toHaveBeenCalledWith(CAROL_USER_ID);

    const requestForm = screen.getByRole("button", { name: "Send request" }).closest("form");
    expect(requestForm).not.toBeNull();
    await fireEvent.submit(requestForm!);
    expect(onSubmitFriendRequest).toHaveBeenCalledTimes(1);

    await fireEvent.click(screen.getByRole("button", { name: "Accept" }));
    expect(onAcceptIncomingFriendRequest).toHaveBeenCalledWith("01ARZ3NDEKTSV4RRFFQ69G5FAY");

    await fireEvent.click(screen.getByRole("button", { name: "Ignore" }));
    await fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onDismissFriendRequest.mock.calls).toEqual([
      ["01ARZ3NDEKTSV4RRFFQ69G5FAY"],
      ["01ARZ3NDEKTSV4RRFFQ69G5FAZ"],
    ]);

    await fireEvent.click(screen.getByRole("button", { name: "Remove" }));
    expect(onRemoveFriendship).toHaveBeenCalledWith(BOB_USER_ID);
  });
});
