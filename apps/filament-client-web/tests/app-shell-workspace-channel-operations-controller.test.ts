import { createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  channelIdFromInput,
  channelNameFromInput,
  guildIdFromInput,
  guildNameFromInput,
  type WorkspaceRecord,
} from "../src/domain/chat";
import { createWorkspaceChannelOperationsController } from "../src/features/app-shell/runtime/workspace-channel-operations-controller";
import type { OverlayPanel } from "../src/features/app-shell/types";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});
const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");
const CHANNEL_ID = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAX");

function submitEventFixture(): SubmitEvent {
  return {
    preventDefault: vi.fn(),
  } as unknown as SubmitEvent;
}

describe("app shell workspace/channel operations controller", () => {
  it("creates a workspace and selects the newly created channel", async () => {
    const [session] = createSignal(SESSION);
    const [activeGuildId, setActiveGuildId] = createSignal<typeof GUILD_ID | null>(null);
    const [createGuildName] = createSignal("Security Ops");
    const [createGuildVisibility] = createSignal<"private" | "public">("private");
    const [createChannelName] = createSignal("incident-room");
    const [createChannelKind, setCreateChannelKind] = createSignal<"text" | "voice">("text");
    const [isCreatingWorkspace, setCreatingWorkspace] = createSignal(false);
    const [isCreatingChannel, setCreatingChannel] = createSignal(false);
    const [newChannelName, setNewChannelName] = createSignal("backend");
    const [newChannelKind, setNewChannelKind] = createSignal<"text" | "voice">("text");
    const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([]);
    const [activeChannelId, setActiveChannelId] = createSignal<typeof CHANNEL_ID | null>(null);
    const [workspaceError, setWorkspaceError] = createSignal("");
    const [messageStatus, setMessageStatus] = createSignal("");
    const [activeOverlayPanel, setActiveOverlayPanel] = createSignal<OverlayPanel | null>(
      "workspace-create",
    );
    const [channelCreateError, setChannelCreateError] = createSignal("");

    const createGuildMock = vi.fn(async () => ({
      guildId: GUILD_ID,
      name: guildNameFromInput("Security Ops"),
      visibility: "private" as const,
    }));
    const createChannelMock = vi.fn(async () => ({
      channelId: CHANNEL_ID,
      name: channelNameFromInput("incident-room"),
      kind: "text" as const,
    }));

    const controller = createWorkspaceChannelOperationsController(
      {
        session,
        activeGuildId,
        createGuildName,
        createGuildVisibility,
        createChannelName,
        createChannelKind,
        isCreatingWorkspace,
        isCreatingChannel,
        newChannelName,
        newChannelKind,
        setWorkspaces,
        setActiveGuildId,
        setActiveChannelId,
        setCreateChannelKind,
        setWorkspaceError,
        setCreatingWorkspace,
        setMessageStatus,
        setActiveOverlayPanel,
        setChannelCreateError,
        setCreatingChannel,
        setNewChannelName,
        setNewChannelKind,
      },
      {
        createGuild: createGuildMock,
        createChannel: createChannelMock,
      },
    );

    await controller.createWorkspace(submitEventFixture());

    expect(createGuildMock).toHaveBeenCalledTimes(1);
    expect(createChannelMock).toHaveBeenCalledTimes(1);
    expect(workspaces()).toHaveLength(1);
    expect(activeGuildId()).toBe(GUILD_ID);
    expect(activeChannelId()).toBe(CHANNEL_ID);
    expect(createChannelKind()).toBe("text");
    expect(messageStatus()).toBe("Workspace created.");
    expect(activeOverlayPanel()).toBeNull();
    expect(workspaceError()).toBe("");
    expect(isCreatingWorkspace()).toBe(false);
    expect(channelCreateError()).toBe("");
  });

  it("creates a channel in the active workspace and resets channel form defaults", async () => {
    const [session] = createSignal(SESSION);
    const [activeGuildId, setActiveGuildId] = createSignal<typeof GUILD_ID | null>(GUILD_ID);
    const [createGuildName] = createSignal("Security Ops");
    const [createGuildVisibility] = createSignal<"private" | "public">("private");
    const [createChannelName] = createSignal("incident-room");
    const [createChannelKind, setCreateChannelKind] = createSignal<"text" | "voice">("text");
    const [isCreatingWorkspace, setCreatingWorkspace] = createSignal(false);
    const [isCreatingChannel, setCreatingChannel] = createSignal(false);
    const [newChannelName, setNewChannelName] = createSignal("triage");
    const [newChannelKind, setNewChannelKind] = createSignal<"text" | "voice">("voice");
    const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([
      {
        guildId: GUILD_ID,
        guildName: guildNameFromInput("Security Ops"),
        visibility: "private" as const,
        channels: [
          {
            channelId: channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAY"),
            name: channelNameFromInput("incident-room"),
            kind: "text" as const,
          },
        ],
      },
    ]);
    const [activeChannelId, setActiveChannelId] = createSignal<typeof CHANNEL_ID | null>(null);
    const [workspaceError, setWorkspaceError] = createSignal("");
    const [messageStatus, setMessageStatus] = createSignal("");
    const [activeOverlayPanel, setActiveOverlayPanel] = createSignal<OverlayPanel | null>(
      "channel-create",
    );
    const [channelCreateError, setChannelCreateError] = createSignal("");

    const createdChannel = {
      channelId: CHANNEL_ID,
      name: channelNameFromInput("triage"),
      kind: "voice" as const,
    };
    const controller = createWorkspaceChannelOperationsController(
      {
        session,
        activeGuildId,
        createGuildName,
        createGuildVisibility,
        createChannelName,
        createChannelKind,
        isCreatingWorkspace,
        isCreatingChannel,
        newChannelName,
        newChannelKind,
        setWorkspaces,
        setActiveGuildId,
        setActiveChannelId,
        setCreateChannelKind,
        setWorkspaceError,
        setCreatingWorkspace,
        setMessageStatus,
        setActiveOverlayPanel,
        setChannelCreateError,
        setCreatingChannel,
        setNewChannelName,
        setNewChannelKind,
      },
      {
        createGuild: vi.fn(),
        createChannel: vi.fn(async () => createdChannel),
      },
    );

    await controller.createNewChannel(submitEventFixture());

    expect(workspaces()[0]?.channels.some((channel) => channel.channelId === CHANNEL_ID)).toBe(
      true,
    );
    expect(activeChannelId()).toBe(CHANNEL_ID);
    expect(newChannelName()).toBe("backend");
    expect(newChannelKind()).toBe("text");
    expect(activeOverlayPanel()).toBeNull();
    expect(messageStatus()).toBe("Channel created.");
    expect(channelCreateError()).toBe("");
    expect(isCreatingChannel()).toBe(false);
    expect(workspaceError()).toBe("");
    expect(createChannelKind()).toBe("text");
  });
});
