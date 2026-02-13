import { createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  guildIdFromInput,
  guildNameFromInput,
  type WorkspaceRecord,
} from "../src/domain/chat";
import { createWorkspaceSettingsActions } from "../src/features/app-shell/runtime/workspace-settings-actions";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3600,
});
const GUILD_ID = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAW");

describe("app shell workspace settings actions", () => {
  it("saves workspace settings and updates active workspace record", async () => {
    const [session] = createSignal(SESSION);
    const [activeGuildId] = createSignal<typeof GUILD_ID | null>(GUILD_ID);
    const [workspaceSettingsName, setWorkspaceSettingsName] = createSignal("Security Ops Updated");
    const [workspaceSettingsVisibility, setWorkspaceSettingsVisibility] = createSignal<
      "private" | "public"
    >("public");
    const [isSavingWorkspaceSettings, setSavingWorkspaceSettings] = createSignal(false);
    const [workspaceSettingsStatus, setWorkspaceSettingsStatus] = createSignal("");
    const [workspaceSettingsError, setWorkspaceSettingsError] = createSignal("");
    const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([
      {
        guildId: GUILD_ID,
        guildName: guildNameFromInput("Security Ops"),
        visibility: "private",
        channels: [],
      },
    ]);

    const updateGuildMock = vi.fn(async () => ({
      guildId: GUILD_ID,
      name: guildNameFromInput("Security Ops Updated"),
      visibility: "public" as const,
    }));

    const actions = createWorkspaceSettingsActions(
      {
        session,
        activeGuildId,
        canManageRoles: () => true,
        workspaceSettingsName,
        workspaceSettingsVisibility,
        setSavingWorkspaceSettings,
        setWorkspaceSettingsStatus,
        setWorkspaceSettingsError,
        setWorkspaces,
        setWorkspaceSettingsName,
        setWorkspaceSettingsVisibility,
      },
      {
        updateGuild: updateGuildMock,
      },
    );

    await actions.saveWorkspaceSettings();

    expect(updateGuildMock).toHaveBeenCalledTimes(1);
    expect(workspaceSettingsName()).toBe("Security Ops Updated");
    expect(workspaceSettingsVisibility()).toBe("public");
    expect(workspaceSettingsStatus()).toBe("Workspace settings saved.");
    expect(workspaceSettingsError()).toBe("");
    expect(isSavingWorkspaceSettings()).toBe(false);
    expect(workspaces()[0]?.guildName).toBe("Security Ops Updated");
    expect(workspaces()[0]?.visibility).toBe("public");
  });

  it("blocks save when user lacks permission", async () => {
    const [session] = createSignal(SESSION);
    const [activeGuildId] = createSignal<typeof GUILD_ID | null>(GUILD_ID);
    const [workspaceSettingsName, setWorkspaceSettingsName] = createSignal("Security Ops");
    const [workspaceSettingsVisibility, setWorkspaceSettingsVisibility] = createSignal<
      "private" | "public"
    >("private");
    const [, setSavingWorkspaceSettings] = createSignal(false);
    const [workspaceSettingsStatus, setWorkspaceSettingsStatus] = createSignal("stale");
    const [workspaceSettingsError, setWorkspaceSettingsError] = createSignal("");
    const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([]);

    const updateGuildMock = vi.fn();
    const actions = createWorkspaceSettingsActions(
      {
        session,
        activeGuildId,
        canManageRoles: () => false,
        workspaceSettingsName,
        workspaceSettingsVisibility,
        setSavingWorkspaceSettings,
        setWorkspaceSettingsStatus,
        setWorkspaceSettingsError,
        setWorkspaces,
        setWorkspaceSettingsName,
        setWorkspaceSettingsVisibility,
      },
      {
        updateGuild: updateGuildMock,
      },
    );

    await actions.saveWorkspaceSettings();

    expect(updateGuildMock).not.toHaveBeenCalled();
    expect(workspaceSettingsStatus()).toBe("");
    expect(workspaceSettingsError()).toBe(
      "You do not have permission to update workspace settings.",
    );
  });

  it("reports validation failure and skips API update", async () => {
    const [session] = createSignal(SESSION);
    const [activeGuildId] = createSignal<typeof GUILD_ID | null>(GUILD_ID);
    const [workspaceSettingsName, setWorkspaceSettingsName] = createSignal(" ");
    const [workspaceSettingsVisibility, setWorkspaceSettingsVisibility] = createSignal<
      "private" | "public"
    >("private");
    const [, setSavingWorkspaceSettings] = createSignal(false);
    const [workspaceSettingsStatus, setWorkspaceSettingsStatus] = createSignal("stale");
    const [workspaceSettingsError, setWorkspaceSettingsError] = createSignal("");
    const [workspaces, setWorkspaces] = createSignal<WorkspaceRecord[]>([]);

    const updateGuildMock = vi.fn();
    const actions = createWorkspaceSettingsActions(
      {
        session,
        activeGuildId,
        canManageRoles: () => true,
        workspaceSettingsName,
        workspaceSettingsVisibility,
        setSavingWorkspaceSettings,
        setWorkspaceSettingsStatus,
        setWorkspaceSettingsError,
        setWorkspaces,
        setWorkspaceSettingsName,
        setWorkspaceSettingsVisibility,
      },
      {
        updateGuild: updateGuildMock,
        mapError: () => "Unable to validate workspace settings.",
      },
    );

    await actions.saveWorkspaceSettings();

    expect(updateGuildMock).not.toHaveBeenCalled();
    expect(workspaceSettingsStatus()).toBe("");
    expect(workspaceSettingsError()).toBe("Unable to validate workspace settings.");
  });
});
