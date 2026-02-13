import type { Accessor } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import type { createMessageState } from "../state/message-state";
import type { createOverlayState } from "../state/overlay-state";
import type { createWorkspaceState } from "../state/workspace-state";
import {
  createWorkspaceChannelOperationsController,
} from "./workspace-channel-operations-controller";

export interface WorkspaceChannelOperationsActionsOptions {
  session: Accessor<AuthSession | null>;
  workspaceChannelState: ReturnType<typeof createWorkspaceState>["workspaceChannel"];
  messageState: ReturnType<typeof createMessageState>;
  overlayState: ReturnType<typeof createOverlayState>;
}

export function createWorkspaceChannelOperationsActions(
  options: WorkspaceChannelOperationsActionsOptions,
) {
  return createWorkspaceChannelOperationsController({
    session: options.session,
    activeGuildId: options.workspaceChannelState.activeGuildId,
    createGuildName: options.workspaceChannelState.createGuildName,
    createGuildVisibility: options.workspaceChannelState.createGuildVisibility,
    createChannelName: options.workspaceChannelState.createChannelName,
    createChannelKind: options.workspaceChannelState.createChannelKind,
    isCreatingWorkspace: options.workspaceChannelState.isCreatingWorkspace,
    isCreatingChannel: options.workspaceChannelState.isCreatingChannel,
    newChannelName: options.workspaceChannelState.newChannelName,
    newChannelKind: options.workspaceChannelState.newChannelKind,
    setWorkspaces: options.workspaceChannelState.setWorkspaces,
    setActiveGuildId: options.workspaceChannelState.setActiveGuildId,
    setActiveChannelId: options.workspaceChannelState.setActiveChannelId,
    setCreateChannelKind: options.workspaceChannelState.setCreateChannelKind,
    setWorkspaceError: options.workspaceChannelState.setWorkspaceError,
    setCreatingWorkspace: options.workspaceChannelState.setCreatingWorkspace,
    setMessageStatus: options.messageState.setMessageStatus,
    setActiveOverlayPanel: options.overlayState.setActiveOverlayPanel,
    setChannelCreateError: options.workspaceChannelState.setChannelCreateError,
    setCreatingChannel: options.workspaceChannelState.setCreatingChannel,
    setNewChannelName: options.workspaceChannelState.setNewChannelName,
    setNewChannelKind: options.workspaceChannelState.setNewChannelKind,
  });
}