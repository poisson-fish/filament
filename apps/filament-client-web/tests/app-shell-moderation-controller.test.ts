import { createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  channelIdFromInput,
  guildIdFromInput,
  roleFromInput,
  userIdFromInput,
} from "../src/domain/chat";
import { createModerationController } from "../src/features/app-shell/controllers/moderation-controller";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});
const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
const CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const TARGET_USER_ID = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");
const SESSION_USER_ID = userIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAY");

describe("app shell moderation controller", () => {
  it("runs member and override moderation flows via shared controller state", async () => {
    const [session] = createSignal(SESSION);
    const [activeGuildId] = createSignal(GUILD_ID);
    const [activeChannelId] = createSignal(CHANNEL_ID);
    const [moderationUserIdInput] = createSignal(TARGET_USER_ID);
    const [moderationRoleInput] = createSignal(roleFromInput("moderator"));
    const [overrideRoleInput] = createSignal(roleFromInput("member"));
    const [overrideAllowCsv] = createSignal("create_message,manage_roles");
    const [overrideDenyCsv] = createSignal("ban_member");
    const [isModerating, setModerating] = createSignal(false);
    const [moderationError, setModerationError] = createSignal("");
    const [moderationStatus, setModerationStatus] = createSignal("");

    const fetchMeMock = vi.fn(async () => ({
      userId: SESSION_USER_ID,
      username: "alice",
      aboutMarkdown: "",
      aboutMarkdownTokens: [],
      avatarVersion: 0,
    }));
    const addGuildMemberMock = vi.fn(async () => ({ accepted: true as const }));
    const updateGuildMemberRoleMock = vi.fn(async () => ({ accepted: true as const }));
    const kickGuildMemberMock = vi.fn(async () => ({ accepted: true as const }));
    const banGuildMemberMock = vi.fn(async () => ({ accepted: true as const }));
    const setChannelRoleOverrideMock = vi.fn(async () => ({ accepted: true as const }));
    const setLegacyChannelOverrideMock = vi.fn();

    const controller = createModerationController(
      {
        session,
        activeGuildId,
        activeChannelId,
        moderationUserIdInput,
        moderationRoleInput,
        overrideRoleInput,
        overrideAllowCsv,
        overrideDenyCsv,
        isModerating,
        setModerating,
        setModerationError,
        setModerationStatus,
        setLegacyChannelOverride: setLegacyChannelOverrideMock,
      },
      {
        fetchMe: fetchMeMock,
        addGuildMember: addGuildMemberMock,
        updateGuildMemberRole: updateGuildMemberRoleMock,
        kickGuildMember: kickGuildMemberMock,
        banGuildMember: banGuildMemberMock,
        setChannelRoleOverride: setChannelRoleOverrideMock,
      },
    );

    await controller.runMemberAction("role");

    expect(fetchMeMock).toHaveBeenCalledTimes(1);
    expect(updateGuildMemberRoleMock).toHaveBeenCalledTimes(1);
    expect(updateGuildMemberRoleMock).toHaveBeenCalledWith(
      SESSION,
      GUILD_ID,
      TARGET_USER_ID,
      "moderator",
    );
    expect(moderationStatus()).toBe("Member role updated to moderator.");
    expect(moderationError()).toBe("");
    expect(isModerating()).toBe(false);

    const preventDefault = vi.fn();
    await controller.applyOverride({
      preventDefault,
    } as unknown as SubmitEvent);

    expect(preventDefault).toHaveBeenCalledTimes(1);
    expect(setChannelRoleOverrideMock).toHaveBeenCalledTimes(1);
    expect(setChannelRoleOverrideMock).toHaveBeenCalledWith(
      SESSION,
      GUILD_ID,
      CHANNEL_ID,
      "member",
      {
        allow: ["create_message", "manage_roles"],
        deny: ["ban_member"],
      },
    );
    expect(moderationStatus()).toBe("Channel role override updated.");
    expect(moderationError()).toBe("");
    expect(isModerating()).toBe(false);
    expect(setLegacyChannelOverrideMock).toHaveBeenCalledWith(
      GUILD_ID,
      CHANNEL_ID,
      "member",
      ["create_message", "manage_roles"],
      ["ban_member"],
      null,
    );

    const actorAction = vi.fn(async () => undefined);
    await controller.runModerationAction(actorAction);
    expect(actorAction).toHaveBeenCalledWith(SESSION_USER_ID, "alice");
    expect(fetchMeMock).toHaveBeenCalledTimes(2);
  });
});
