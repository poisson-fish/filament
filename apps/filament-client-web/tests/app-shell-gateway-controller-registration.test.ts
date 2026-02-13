import { createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  channelIdFromInput,
  guildIdFromInput,
  type FriendRecord,
  type FriendRequestList,
  type MessageRecord,
  type WorkspaceRecord,
} from "../src/domain/chat";
import type {
  GatewayControllerOptions,
} from "../src/features/app-shell/controllers/gateway-controller";
import type { ReactionView } from "../src/features/app-shell/helpers";
import { registerGatewayController } from "../src/features/app-shell/runtime/gateway-controller-registration";
import type { VoiceParticipantPayload } from "../src/lib/gateway";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});

describe("app shell gateway controller registration", () => {
  it("wires permission refresh callback into gateway controller registration", () => {
    const activeGuildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    const activeChannelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
    const permissionRefreshGuildId = guildIdFromInput(
      "01ARZ3NDEKTSV4RRFFQ69G5FAX",
    );

    const [session] = createSignal(SESSION);
    const [guildId] = createSignal(activeGuildId);
    const [channelId] = createSignal(activeChannelId);
    const [canAccessActiveChannel] = createSignal(true);
    const [, setGatewayOnline] = createSignal(false);
    const [, setOnlineMembers] = createSignal<string[]>([]);
    const [, setWorkspaces] = createSignal<WorkspaceRecord[]>([]);
    const [, setMessages] = createSignal<MessageRecord[]>([]);
    const [, setReactionState] = createSignal<Record<string, ReactionView>>({});
    const [, setResolvedUsernames] = createSignal<Record<string, string>>({});
    const [, setAvatarVersionByUserId] = createSignal<Record<string, number>>({});
    const [, setProfileDraftUsername] = createSignal("");
    const [, setProfileDraftAbout] = createSignal("");
    const [, setFriends] = createSignal<FriendRecord[]>([]);
    const [, setFriendRequests] = createSignal<FriendRequestList>({
      incoming: [],
      outgoing: [],
    });
    const [, setVoiceParticipantsByChannel] = createSignal<
      Record<string, VoiceParticipantPayload[]>
    >({});

    const isMessageListNearBottom = vi.fn(() => true);
    const scrollMessageListToBottom = vi.fn();
    const refreshWorkspacePermissionStateFromGateway = vi.fn(async () => undefined);

    const createGatewayControllerMock = vi.fn((options: GatewayControllerOptions) => {
      options.onWorkspacePermissionsChanged?.(permissionRefreshGuildId);
    });

    registerGatewayController(
      {
        session,
        activeGuildId: guildId,
        activeChannelId: channelId,
        canAccessActiveChannel,
        setGatewayOnline,
        setOnlineMembers,
        setWorkspaces,
        setMessages,
        setReactionState,
        setResolvedUsernames,
        setAvatarVersionByUserId,
        setProfileDraftUsername,
        setProfileDraftAbout,
        setFriends,
        setFriendRequests,
        setVoiceParticipantsByChannel,
        isMessageListNearBottom,
        scrollMessageListToBottom,
        refreshWorkspacePermissionStateFromGateway,
      },
      {
        createGatewayController: createGatewayControllerMock,
      },
    );

    expect(createGatewayControllerMock).toHaveBeenCalledTimes(1);
    expect(refreshWorkspacePermissionStateFromGateway).toHaveBeenCalledTimes(1);
    expect(refreshWorkspacePermissionStateFromGateway).toHaveBeenCalledWith(
      permissionRefreshGuildId,
    );
  });
});
