import type { Accessor, Setter } from "solid-js";
import {
  guildNameFromInput,
  guildVisibilityFromInput,
  type GuildId,
  type GuildName,
  type GuildVisibility,
  type WorkspaceRecord,
} from "../../../domain/chat";
import type { AuthSession } from "../../../domain/auth";
import { updateGuild } from "../../../lib/api";
import { mapError } from "../helpers";

export interface WorkspaceSettingsActionsOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  canManageRoles: () => boolean;
  workspaceSettingsName: Accessor<string>;
  workspaceSettingsVisibility: Accessor<GuildVisibility>;
  setSavingWorkspaceSettings: Setter<boolean>;
  setWorkspaceSettingsStatus: Setter<string>;
  setWorkspaceSettingsError: Setter<string>;
  setWorkspaces: Setter<WorkspaceRecord[]>;
  setWorkspaceSettingsName: Setter<string>;
  setWorkspaceSettingsVisibility: Setter<GuildVisibility>;
}

export interface WorkspaceSettingsActionsDependencies {
  updateGuild: typeof updateGuild;
  mapError: (error: unknown, fallback: string) => string;
}

const DEFAULT_WORKSPACE_SETTINGS_ACTIONS_DEPENDENCIES: WorkspaceSettingsActionsDependencies = {
  updateGuild,
  mapError,
};

export function createWorkspaceSettingsActions(
  options: WorkspaceSettingsActionsOptions,
  dependencies: Partial<WorkspaceSettingsActionsDependencies> = {},
) {
  const deps = {
    ...DEFAULT_WORKSPACE_SETTINGS_ACTIONS_DEPENDENCIES,
    ...dependencies,
  };

  const saveWorkspaceSettings = async (): Promise<void> => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId) {
      return;
    }
    if (!options.canManageRoles()) {
      options.setWorkspaceSettingsError(
        "You do not have permission to update workspace settings.",
      );
      options.setWorkspaceSettingsStatus("");
      return;
    }

    let nextName: GuildName;
    let nextVisibility: GuildVisibility;
    try {
      nextName = guildNameFromInput(options.workspaceSettingsName());
      nextVisibility = guildVisibilityFromInput(options.workspaceSettingsVisibility());
    } catch (error) {
      options.setWorkspaceSettingsError(
        deps.mapError(error, "Unable to validate workspace settings."),
      );
      options.setWorkspaceSettingsStatus("");
      return;
    }

    options.setSavingWorkspaceSettings(true);
    options.setWorkspaceSettingsStatus("");
    options.setWorkspaceSettingsError("");
    try {
      const updatedGuild = await deps.updateGuild(session, guildId, {
        name: nextName,
        visibility: nextVisibility,
      });
      options.setWorkspaces((existing) =>
        existing.map((workspace) =>
          workspace.guildId === guildId
            ? {
                ...workspace,
                guildName: updatedGuild.name,
                visibility: updatedGuild.visibility,
              }
            : workspace,
        ),
      );
      options.setWorkspaceSettingsName(updatedGuild.name);
      options.setWorkspaceSettingsVisibility(updatedGuild.visibility);
      options.setWorkspaceSettingsStatus("Workspace settings saved.");
    } catch (error) {
      options.setWorkspaceSettingsError(
        deps.mapError(error, "Unable to save workspace settings."),
      );
    } finally {
      options.setSavingWorkspaceSettings(false);
    }
  };

  return {
    saveWorkspaceSettings,
  };
}