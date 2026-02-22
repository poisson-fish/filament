import type { Accessor, Setter } from "solid-js";
import { DomainValidationError } from "../../../domain/auth";
import type { AuthSession } from "../../../domain/auth";
import {
  roleFromInput,
  userIdFromInput,
  type ChannelId,
  type GuildId,
  type PermissionName,
  type RoleName,
  type UserId,
} from "../../../domain/chat";
import {
  addGuildMember,
  banGuildMember,
  fetchMe,
  kickGuildMember,
  setChannelRoleOverride,
  updateGuildMemberRole,
} from "../../../lib/api";
import { mapError, parsePermissionCsv } from "../helpers";

export type ModerationMemberAction = "add" | "role" | "kick" | "ban";

export interface ModerationControllerOptions {
  session: Accessor<AuthSession | null>;
  activeGuildId: Accessor<GuildId | null>;
  activeChannelId: Accessor<ChannelId | null>;
  moderationUserIdInput: Accessor<string>;
  moderationRoleInput: Accessor<RoleName>;
  overrideRoleInput: Accessor<RoleName>;
  overrideAllowCsv: Accessor<string>;
  overrideDenyCsv: Accessor<string>;
  isModerating: Accessor<boolean>;
  setModerating: Setter<boolean>;
  setModerationError: Setter<string>;
  setModerationStatus: Setter<string>;
  setLegacyChannelOverride?: (
    guildId: GuildId,
    channelId: ChannelId,
    role: RoleName,
    allow: ReadonlyArray<PermissionName>,
    deny: ReadonlyArray<PermissionName>,
    updatedAtUnix: number | null,
  ) => void;
}

export interface ModerationControllerDependencies {
  fetchMe: typeof fetchMe;
  addGuildMember: typeof addGuildMember;
  updateGuildMemberRole: typeof updateGuildMemberRole;
  kickGuildMember: typeof kickGuildMember;
  banGuildMember: typeof banGuildMember;
  setChannelRoleOverride: typeof setChannelRoleOverride;
}

export interface ModerationController {
  runModerationAction: (
    action: (sessionUserId: UserId, sessionUsername: string) => Promise<void>,
  ) => Promise<void>;
  runMemberAction: (action: ModerationMemberAction) => Promise<void>;
  applyOverride: (event: SubmitEvent) => Promise<void>;
}

const DEFAULT_MODERATION_CONTROLLER_DEPENDENCIES: ModerationControllerDependencies = {
  fetchMe,
  addGuildMember,
  updateGuildMemberRole,
  kickGuildMember,
  banGuildMember,
  setChannelRoleOverride,
};

export function createModerationController(
  options: ModerationControllerOptions,
  dependencies: Partial<ModerationControllerDependencies> = {},
): ModerationController {
  const deps = {
    ...DEFAULT_MODERATION_CONTROLLER_DEPENDENCIES,
    ...dependencies,
  };

  const runModerationAction = async (
    action: (sessionUserId: UserId, sessionUsername: string) => Promise<void>,
  ) => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId || options.isModerating()) {
      return;
    }

    options.setModerationError("");
    options.setModerationStatus("");
    options.setModerating(true);
    try {
      const me = await deps.fetchMe(session);
      await action(me.userId, me.username);
    } catch (error) {
      options.setModerationError(mapError(error, "Moderation action failed."));
    } finally {
      options.setModerating(false);
    }
  };

  const runMemberAction = async (action: ModerationMemberAction) => {
    const session = options.session();
    const guildId = options.activeGuildId();
    if (!session || !guildId) {
      options.setModerationError("Select a workspace first.");
      return;
    }

    let targetUserId: UserId;
    try {
      targetUserId = userIdFromInput(options.moderationUserIdInput().trim());
    } catch (error) {
      options.setModerationError(mapError(error, "Target user ID is invalid."));
      return;
    }

    await runModerationAction(async () => {
      if (action === "add") {
        await deps.addGuildMember(session, guildId, targetUserId);
        options.setModerationStatus("Member add request accepted.");
        return;
      }
      if (action === "role") {
        const role = roleFromInput(options.moderationRoleInput());
        await deps.updateGuildMemberRole(session, guildId, targetUserId, role);
        options.setModerationStatus(`Member role updated to ${role}.`);
        return;
      }
      if (action === "kick") {
        await deps.kickGuildMember(session, guildId, targetUserId);
        options.setModerationStatus("Member kicked.");
        return;
      }
      await deps.banGuildMember(session, guildId, targetUserId);
      options.setModerationStatus("Member banned.");
    });
  };

  const applyOverride = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = options.session();
    const guildId = options.activeGuildId();
    const channelId = options.activeChannelId();
    if (!session || !guildId || !channelId || options.isModerating()) {
      return;
    }

    try {
      const allow = parsePermissionCsv(options.overrideAllowCsv());
      const deny = parsePermissionCsv(options.overrideDenyCsv());
      if (allow.some((permission) => deny.includes(permission))) {
        throw new DomainValidationError(
          "Allow and deny permission sets cannot overlap.",
        );
      }

      options.setModerating(true);
      options.setModerationError("");
      options.setModerationStatus("");
      await deps.setChannelRoleOverride(
        session,
        guildId,
        channelId,
        roleFromInput(options.overrideRoleInput()),
        {
          allow,
          deny,
        },
      );
      options.setLegacyChannelOverride?.(
        guildId,
        channelId,
        roleFromInput(options.overrideRoleInput()),
        allow,
        deny,
        null,
      );
      options.setModerationStatus("Channel role override updated.");
    } catch (error) {
      options.setModerationError(mapError(error, "Unable to set channel override."));
    } finally {
      options.setModerating(false);
    }
  };

  return {
    runModerationAction,
    runMemberAction,
    applyOverride,
  };
}
