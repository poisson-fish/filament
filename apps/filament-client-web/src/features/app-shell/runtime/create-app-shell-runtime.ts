import {
  createEffect,
  onCleanup,
  untrack,
} from "solid-js";
import {
  channelKindFromInput,
  type ChannelId,
  type GuildId,
} from "../../../domain/chat";
import { useAuth } from "../../../lib/auth-context";
import {
  canRequestAudioCapturePermission,
  enumerateAudioDevices,
  reconcileVoiceDevicePreferences,
  requestAudioCapturePermission,
  saveVoiceDevicePreferences,
  type MediaDeviceId,
  type VoiceDevicePreferences,
} from "../../../lib/voice-device-settings";
import { saveWorkspaceCache } from "../../../lib/workspace-cache";
import { buildPanelHostPropGroups } from "../adapters/panel-host-props";
import { createAttachmentController } from "../controllers/attachment-controller";
import {
  createFriendshipController,
} from "../controllers/friendship-controller";
import { createGatewayController } from "../controllers/gateway-controller";
import {
  createIdentityResolutionController,
} from "../controllers/identity-resolution-controller";
import {
  createMessageHistoryController,
} from "../controllers/message-history-controller";
import { createMessageListController } from "../controllers/message-list-controller";
import {
  createMessageActionsController,
  createMessageMediaPreviewController,
} from "../controllers/message-controller";
import { createModerationController } from "../controllers/moderation-controller";
import {
  createOverlayPanelAuthorizationController,
  createOverlayPanelEscapeController,
  openOverlayPanelWithDefaults,
} from "../controllers/overlay-controller";
import { createProfileController } from "../controllers/profile-controller";
import { createProfileOverlayController } from "../controllers/profile-overlay-controller";
import { createPublicDirectoryController } from "../controllers/public-directory-controller";
import { createReactionPickerController } from "../controllers/reaction-picker-controller";
import { createRoleManagementController } from "../controllers/role-management-controller";
import { createSearchController } from "../controllers/search-controller";
import {
  createVoiceOperationsController,
} from "../controllers/voice-operations-controller";
import {
  createVoiceSessionLifecycleController,
  resolveVoiceDevicePreferenceStatus,
  unavailableVoiceDeviceError,
} from "../controllers/voice-controller";
import {
  createChannelPermissionsController,
  createWorkspaceBootstrapController,
  createWorkspaceSelectionController,
} from "../controllers/workspace-controller";
import {
  mapError,
  mapRtcError,
  userIdFromVoiceIdentity,
} from "../helpers";
import { createAppShellSelectors } from "../selectors/create-app-shell-selectors";
import { createDiagnosticsState } from "../state/diagnostics-state";
import { createMessageState } from "../state/message-state";
import { createOverlayState } from "../state/overlay-state";
import { createProfileState } from "../state/profile-state";
import {
  createVoiceState,
  DEFAULT_VOICE_SESSION_CAPABILITIES,
} from "../state/voice-state";
import { createWorkspaceState } from "../state/workspace-state";
import type {
  OverlayPanel,
  SettingsCategory,
} from "../types";
import { createAppShellRuntimeLabels } from "./runtime-labels";
import { createSessionDiagnosticsController } from "./session-diagnostics-controller";
import { createWorkspaceChannelOperationsController } from "./workspace-channel-operations-controller";

export type AppShellAuthContext = ReturnType<typeof useAuth>;

export function createAppShellRuntime(auth: AppShellAuthContext) {
  let composerAttachmentInputRef: HTMLInputElement | undefined;
  const setComposerAttachmentInputRef = (
    value: HTMLInputElement | undefined,
  ): void => {
    composerAttachmentInputRef = value;
  };

  const workspaceState = createWorkspaceState();
  const workspaceChannelState = workspaceState.workspaceChannel;
  const friendshipsState = workspaceState.friendships;
  const discoveryState = workspaceState.discovery;
  const messageState = createMessageState();
  const profileState = createProfileState();
  const voiceState = createVoiceState();
  const diagnosticsState = createDiagnosticsState();
  const overlayState = createOverlayState();

  const selectors = createAppShellSelectors({
    workspaces: workspaceChannelState.workspaces,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    channelPermissions: workspaceChannelState.channelPermissions,
    voiceSessionChannelKey: voiceState.voiceSessionChannelKey,
    attachmentByChannel: messageState.attachmentByChannel,
    rtcSnapshot: voiceState.rtcSnapshot,
    voiceSessionCapabilities: voiceState.voiceSessionCapabilities,
    voiceSessionStartedAtUnixMs: voiceState.voiceSessionStartedAtUnixMs,
    voiceDurationClockUnixMs: voiceState.voiceDurationClockUnixMs,
    activeOverlayPanel: overlayState.activeOverlayPanel,
  });

  const openSettingsCategory = (category: SettingsCategory): void => {
    overlayState.setActiveSettingsCategory(category);
    if (category === "voice") {
      overlayState.setActiveVoiceSettingsSubmenu("audio-devices");
    }
  };

  const reactionPickerController = createReactionPickerController({
    openReactionPickerMessageId: messageState.openReactionPickerMessageId,
    setOpenReactionPickerMessageId: messageState.setOpenReactionPickerMessageId,
    setReactionPickerOverlayPosition: messageState.setReactionPickerOverlayPosition,
    trackPositionDependencies: () => {
      void messageState.messages();
      void voiceState.voiceStatus();
      void voiceState.voiceError();
      void selectors.voiceRosterEntries().length;
    },
  });

  const messageListController = createMessageListController({
    nextBefore: messageState.nextBefore,
    isLoadingOlder: messageState.isLoadingOlder,
    openReactionPickerMessageId: messageState.openReactionPickerMessageId,
    setShowLoadOlderButton: messageState.setShowLoadOlderButton,
    updateReactionPickerOverlayPosition:
      reactionPickerController.updateReactionPickerOverlayPosition,
  });

  const voiceOperationsController = createVoiceOperationsController({
    session: auth.session,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannel: selectors.activeChannel,
    canPublishVoiceCamera: selectors.canPublishVoiceCamera,
    canPublishVoiceScreenShare: selectors.canPublishVoiceScreenShare,
    canSubscribeVoiceStreams: selectors.canSubscribeVoiceStreams,
    canToggleVoiceCamera: selectors.canToggleVoiceCamera,
    canToggleVoiceScreenShare: selectors.canToggleVoiceScreenShare,
    isJoiningVoice: voiceState.isJoiningVoice,
    isLeavingVoice: voiceState.isLeavingVoice,
    isTogglingVoiceMic: voiceState.isTogglingVoiceMic,
    isTogglingVoiceCamera: voiceState.isTogglingVoiceCamera,
    isTogglingVoiceScreenShare: voiceState.isTogglingVoiceScreenShare,
    voiceDevicePreferences: voiceState.voiceDevicePreferences,
    setRtcSnapshot: voiceState.setRtcSnapshot,
    setVoiceStatus: voiceState.setVoiceStatus,
    setVoiceError: voiceState.setVoiceError,
    setJoiningVoice: voiceState.setJoiningVoice,
    setLeavingVoice: voiceState.setLeavingVoice,
    setTogglingVoiceMic: voiceState.setTogglingVoiceMic,
    setTogglingVoiceCamera: voiceState.setTogglingVoiceCamera,
    setTogglingVoiceScreenShare: voiceState.setTogglingVoiceScreenShare,
    setVoiceSessionChannelKey: voiceState.setVoiceSessionChannelKey,
    setVoiceSessionStartedAtUnixMs: voiceState.setVoiceSessionStartedAtUnixMs,
    setVoiceDurationClockUnixMs: voiceState.setVoiceDurationClockUnixMs,
    setVoiceSessionCapabilities: voiceState.setVoiceSessionCapabilities,
    setAudioDevicesError: voiceState.setAudioDevicesError,
    defaultVoiceSessionCapabilities: DEFAULT_VOICE_SESSION_CAPABILITIES,
  });

  const {
    releaseRtcClient,
    joinVoiceChannel,
    leaveVoiceChannel,
    toggleVoiceMicrophone,
    toggleVoiceCamera,
    toggleVoiceScreenShare,
  } = voiceOperationsController;

  const persistVoiceDevicePreferences = (next: VoiceDevicePreferences): void => {
    voiceState.setVoiceDevicePreferences(next);
    try {
      saveVoiceDevicePreferences(next);
    } catch {
      voiceState.setAudioDevicesError(
        "Unable to persist audio device preferences in local storage.",
      );
    }
  };

  const refreshAudioDeviceInventory = async (
    requestPermissionPrompt = false,
  ): Promise<void> => {
    if (voiceState.isRefreshingAudioDevices()) {
      return;
    }
    voiceState.setRefreshingAudioDevices(true);
    voiceState.setAudioDevicesError("");
    try {
      let inventory = await enumerateAudioDevices();
      if (
        requestPermissionPrompt &&
        inventory.audioInputs.length === 0 &&
        canRequestAudioCapturePermission()
      ) {
        await requestAudioCapturePermission();
        inventory = await enumerateAudioDevices();
      }
      voiceState.setAudioInputDevices(inventory.audioInputs);
      voiceState.setAudioOutputDevices(inventory.audioOutputs);
      voiceState.setAudioDevicesStatus(
        `Detected ${inventory.audioInputs.length} microphone(s) and ${inventory.audioOutputs.length} speaker(s).`,
      );
      const current = voiceState.voiceDevicePreferences();
      const reconciled = reconcileVoiceDevicePreferences(current, inventory);
      if (
        current.audioInputDeviceId !== reconciled.audioInputDeviceId ||
        current.audioOutputDeviceId !== reconciled.audioOutputDeviceId
      ) {
        persistVoiceDevicePreferences(reconciled);
        voiceState.setAudioDevicesStatus(
          "Some saved audio devices are no longer available. Reverted to system defaults.",
        );
      }
    } catch (error) {
      voiceState.setAudioInputDevices([]);
      voiceState.setAudioOutputDevices([]);
      voiceState.setAudioDevicesStatus("");
      voiceState.setAudioDevicesError(
        mapError(error, "Unable to enumerate audio devices."),
      );
    } finally {
      voiceState.setRefreshingAudioDevices(false);
    }
  };

  const setVoiceDevicePreference = async (
    kind: "audioinput" | "audiooutput",
    nextValue: string,
  ): Promise<void> => {
    const options =
      kind === "audioinput"
        ? voiceState.audioInputDevices()
        : voiceState.audioOutputDevices();
    if (nextValue.length > 0 && !options.some((entry) => entry.deviceId === nextValue)) {
      voiceState.setAudioDevicesError(unavailableVoiceDeviceError(kind));
      return;
    }

    const nextDeviceId = nextValue.length > 0 ? (nextValue as MediaDeviceId) : null;
    const next: VoiceDevicePreferences =
      kind === "audioinput"
        ? {
            ...voiceState.voiceDevicePreferences(),
            audioInputDeviceId: nextDeviceId,
          }
        : {
            ...voiceState.voiceDevicePreferences(),
            audioOutputDeviceId: nextDeviceId,
          };
    voiceState.setAudioDevicesError("");
    persistVoiceDevicePreferences(next);

    const client = voiceOperationsController.peekRtcClient();
    if (!client || !selectors.isVoiceSessionActive()) {
      voiceState.setAudioDevicesStatus(
        resolveVoiceDevicePreferenceStatus(kind, false, nextDeviceId),
      );
      return;
    }

    try {
      if (kind === "audioinput") {
        await client.setAudioInputDevice(next.audioInputDeviceId);
      } else {
        await client.setAudioOutputDevice(next.audioOutputDeviceId);
      }
      voiceState.setAudioDevicesStatus(
        resolveVoiceDevicePreferenceStatus(kind, true, nextDeviceId),
      );
    } catch (error) {
      voiceState.setAudioDevicesError(
        mapRtcError(
          error,
          kind === "audioinput"
            ? "Unable to apply microphone selection."
            : "Unable to apply speaker selection.",
        ),
      );
    }
  };

  const openOverlayPanel = (panel: OverlayPanel): void => {
    openOverlayPanelWithDefaults(panel, {
      setPanel: overlayState.setActiveOverlayPanel,
      setWorkspaceError: workspaceChannelState.setWorkspaceError,
      setChannelCreateError: workspaceChannelState.setChannelCreateError,
      setActiveSettingsCategory: overlayState.setActiveSettingsCategory,
      setActiveVoiceSettingsSubmenu: overlayState.setActiveVoiceSettingsSubmenu,
    });
  };

  const closeOverlayPanel = (): void => {
    if (!selectors.canCloseActivePanel()) {
      return;
    }
    overlayState.setActiveOverlayPanel(null);
  };

  createWorkspaceBootstrapController({
    session: auth.session,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    setWorkspaces: workspaceChannelState.setWorkspaces,
    setActiveGuildId: workspaceChannelState.setActiveGuildId,
    setActiveChannelId: workspaceChannelState.setActiveChannelId,
    setWorkspaceBootstrapDone: workspaceChannelState.setWorkspaceBootstrapDone,
  });

  createWorkspaceSelectionController({
    workspaces: workspaceChannelState.workspaces,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    setActiveGuildId: workspaceChannelState.setActiveGuildId,
    setActiveChannelId: workspaceChannelState.setActiveChannelId,
  });

  createChannelPermissionsController({
    session: auth.session,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    setWorkspaces: workspaceChannelState.setWorkspaces,
    setChannelPermissions: workspaceChannelState.setChannelPermissions,
  });

  createOverlayPanelAuthorizationController({
    panel: overlayState.activeOverlayPanel,
    context: () => ({
      canAccessActiveChannel: selectors.canAccessActiveChannel(),
      canManageWorkspaceChannels: selectors.canManageWorkspaceChannels(),
      hasRoleManagementAccess: selectors.hasRoleManagementAccess(),
      hasModerationAccess: selectors.hasModerationAccess(),
    }),
    setPanel: overlayState.setActiveOverlayPanel,
  });

  createOverlayPanelEscapeController({
    panel: overlayState.activeOverlayPanel,
    onEscape: closeOverlayPanel,
  });

  createProfileOverlayController({
    selectedProfileUserId: profileState.selectedProfileUserId,
    setSelectedProfileUserId: profileState.setSelectedProfileUserId,
  });

  const messageMediaPreviewController = createMessageMediaPreviewController({
    session: auth.session,
    setAuthenticatedSession: auth.setAuthenticatedSession,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    messages: messageState.messages,
  });

  const messageActions = createMessageActionsController({
    session: auth.session,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    activeChannel: selectors.activeChannel,
    canAccessActiveChannel: selectors.canAccessActiveChannel,
    composer: messageState.composer,
    setComposer: messageState.setComposer,
    composerAttachments: messageState.composerAttachments,
    setComposerAttachments: messageState.setComposerAttachments,
    composerAttachmentInputElement: () => composerAttachmentInputRef,
    isSendingMessage: messageState.isSendingMessage,
    setSendingMessage: messageState.setSendingMessage,
    setMessageStatus: messageState.setMessageStatus,
    setMessageError: messageState.setMessageError,
    setMessages: messageState.setMessages,
    setAttachmentByChannel: messageState.setAttachmentByChannel,
    isMessageListNearBottom: messageListController.isMessageListNearBottom,
    scrollMessageListToBottom: messageListController.scrollMessageListToBottom,
    editingMessageId: messageState.editingMessageId,
    setEditingMessageId: messageState.setEditingMessageId,
    editingDraft: messageState.editingDraft,
    setEditingDraft: messageState.setEditingDraft,
    isSavingEdit: messageState.isSavingEdit,
    setSavingEdit: messageState.setSavingEdit,
    deletingMessageId: messageState.deletingMessageId,
    setDeletingMessageId: messageState.setDeletingMessageId,
    reactionState: messageState.reactionState,
    setReactionState: messageState.setReactionState,
    pendingReactionByKey: messageState.pendingReactionByKey,
    setPendingReactionByKey: messageState.setPendingReactionByKey,
    openReactionPickerMessageId: messageState.openReactionPickerMessageId,
    setOpenReactionPickerMessageId: messageState.setOpenReactionPickerMessageId,
  });

  const searchActions = createSearchController({
    session: auth.session,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    searchQuery: discoveryState.searchQuery,
    isSearching: discoveryState.isSearching,
    setSearching: discoveryState.setSearching,
    setSearchError: discoveryState.setSearchError,
    setSearchResults: discoveryState.setSearchResults,
    isRunningSearchOps: discoveryState.isRunningSearchOps,
    setRunningSearchOps: discoveryState.setRunningSearchOps,
    setSearchOpsStatus: discoveryState.setSearchOpsStatus,
  });

  const attachmentActions = createAttachmentController({
    session: auth.session,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    selectedAttachment: messageState.selectedAttachment,
    attachmentFilename: messageState.attachmentFilename,
    isUploadingAttachment: messageState.isUploadingAttachment,
    downloadingAttachmentId: messageState.downloadingAttachmentId,
    deletingAttachmentId: messageState.deletingAttachmentId,
    setAttachmentStatus: messageState.setAttachmentStatus,
    setAttachmentError: messageState.setAttachmentError,
    setUploadingAttachment: messageState.setUploadingAttachment,
    setDownloadingAttachmentId: messageState.setDownloadingAttachmentId,
    setDeletingAttachmentId: messageState.setDeletingAttachmentId,
    setSelectedAttachment: messageState.setSelectedAttachment,
    setAttachmentFilename: messageState.setAttachmentFilename,
    setAttachmentByChannel: messageState.setAttachmentByChannel,
  });

  const moderationActions = createModerationController({
    session: auth.session,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    moderationUserIdInput: diagnosticsState.moderationUserIdInput,
    moderationRoleInput: diagnosticsState.moderationRoleInput,
    overrideRoleInput: diagnosticsState.overrideRoleInput,
    overrideAllowCsv: diagnosticsState.overrideAllowCsv,
    overrideDenyCsv: diagnosticsState.overrideDenyCsv,
    isModerating: diagnosticsState.isModerating,
    setModerating: diagnosticsState.setModerating,
    setModerationError: diagnosticsState.setModerationError,
    setModerationStatus: diagnosticsState.setModerationStatus,
  });

  const roleManagementActions = createRoleManagementController({
    session: auth.session,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    setChannelPermissions: workspaceChannelState.setChannelPermissions,
  });

  const profileController = createProfileController({
    session: auth.session,
    selectedProfileUserId: profileState.selectedProfileUserId,
    avatarVersionByUserId: profileState.avatarVersionByUserId,
    profileDraftUsername: profileState.profileDraftUsername,
    profileDraftAbout: profileState.profileDraftAbout,
    selectedProfileAvatarFile: profileState.selectedProfileAvatarFile,
    isSavingProfile: profileState.isSavingProfile,
    isUploadingProfileAvatar: profileState.isUploadingProfileAvatar,
    setProfileDraftUsername: profileState.setProfileDraftUsername,
    setProfileDraftAbout: profileState.setProfileDraftAbout,
    setSelectedProfileAvatarFile: profileState.setSelectedProfileAvatarFile,
    setProfileSettingsStatus: profileState.setProfileSettingsStatus,
    setProfileSettingsError: profileState.setProfileSettingsError,
    setSavingProfile: profileState.setSavingProfile,
    setUploadingProfileAvatar: profileState.setUploadingProfileAvatar,
    setSelectedProfileUserId: profileState.setSelectedProfileUserId,
    setSelectedProfileError: profileState.setSelectedProfileError,
  });

  const publicDirectoryActions = createPublicDirectoryController({
    session: auth.session,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    publicGuildSearchQuery: discoveryState.publicGuildSearchQuery,
    isSearchingPublicGuilds: discoveryState.isSearchingPublicGuilds,
    publicGuildJoinStatusByGuildId: discoveryState.publicGuildJoinStatusByGuildId,
    setSearchingPublicGuilds: discoveryState.setSearchingPublicGuilds,
    setPublicGuildSearchError: discoveryState.setPublicGuildSearchError,
    setPublicGuildDirectory: discoveryState.setPublicGuildDirectory,
    setPublicGuildJoinStatusByGuildId: discoveryState.setPublicGuildJoinStatusByGuildId,
    setPublicGuildJoinErrorByGuildId: discoveryState.setPublicGuildJoinErrorByGuildId,
    setWorkspaces: workspaceChannelState.setWorkspaces,
    setActiveGuildId: workspaceChannelState.setActiveGuildId,
    setActiveChannelId: workspaceChannelState.setActiveChannelId,
  });

  const friendshipActions = createFriendshipController({
    session: auth.session,
    friendRecipientUserIdInput: friendshipsState.friendRecipientUserIdInput,
    isRunningFriendAction: friendshipsState.isRunningFriendAction,
    setFriends: friendshipsState.setFriends,
    setFriendRequests: friendshipsState.setFriendRequests,
    setRunningFriendAction: friendshipsState.setRunningFriendAction,
    setFriendStatus: friendshipsState.setFriendStatus,
    setFriendError: friendshipsState.setFriendError,
    setFriendRecipientUserIdInput: friendshipsState.setFriendRecipientUserIdInput,
  });

  const labels = createAppShellRuntimeLabels({
    resolvedUsernames: profileState.resolvedUsernames,
  });

  createIdentityResolutionController({
    session: auth.session,
    messages: messageState.messages,
    onlineMembers: profileState.onlineMembers,
    voiceRosterEntries: selectors.voiceRosterEntries,
    searchResults: discoveryState.searchResults,
    profile: profileController.profile,
    selectedProfile: profileController.selectedProfile,
    friends: friendshipsState.friends,
    friendRequests: friendshipsState.friendRequests,
    setResolvedUsernames: profileState.setResolvedUsernames,
    setAvatarVersionByUserId: profileState.setAvatarVersionByUserId,
  });

  createEffect(() => {
    if (!workspaceChannelState.workspaceBootstrapDone()) {
      return;
    }
    saveWorkspaceCache(workspaceChannelState.workspaces());
  });

  createEffect(() => {
    if (!workspaceChannelState.workspaceBootstrapDone()) {
      return;
    }
    if (workspaceChannelState.workspaces().length === 0) {
      overlayState.setActiveOverlayPanel("workspace-create");
    }
  });

  const messageHistoryActions = createMessageHistoryController({
    session: auth.session,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    canAccessActiveChannel: selectors.canAccessActiveChannel,
    nextBefore: messageState.nextBefore,
    isLoadingOlder: messageState.isLoadingOlder,
    setMessages: messageState.setMessages,
    setNextBefore: messageState.setNextBefore,
    setShowLoadOlderButton: messageState.setShowLoadOlderButton,
    setMessageError: messageState.setMessageError,
    setLoadingMessages: messageState.setLoadingMessages,
    setLoadingOlder: messageState.setLoadingOlder,
    setEditingMessageId: messageState.setEditingMessageId,
    setEditingDraft: messageState.setEditingDraft,
    setReactionState: messageState.setReactionState,
    setPendingReactionByKey: messageState.setPendingReactionByKey,
    setOpenReactionPickerMessageId: messageState.setOpenReactionPickerMessageId,
    setSearchResults: discoveryState.setSearchResults,
    setSearchError: discoveryState.setSearchError,
    setSearchOpsStatus: discoveryState.setSearchOpsStatus,
    setAttachmentStatus: messageState.setAttachmentStatus,
    setAttachmentError: messageState.setAttachmentError,
    setVoiceStatus: voiceState.setVoiceStatus,
    setVoiceError: voiceState.setVoiceError,
    captureScrollMetrics: messageListController.captureScrollMetrics,
    restoreScrollAfterPrepend: messageListController.restoreScrollAfterPrepend,
    scrollMessageListToBottom: messageListController.scrollMessageListToBottom,
  });

  createEffect(() => {
    const isVoiceAudioSettingsOpen =
      overlayState.activeOverlayPanel() === "settings" &&
      overlayState.activeSettingsCategory() === "voice" &&
      overlayState.activeVoiceSettingsSubmenu() === "audio-devices";
    if (!isVoiceAudioSettingsOpen) {
      return;
    }
    void untrack(() => refreshAudioDeviceInventory(false));
  });

  createGatewayController({
    session: auth.session,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    canAccessActiveChannel: selectors.canAccessActiveChannel,
    setGatewayOnline: profileState.setGatewayOnline,
    setOnlineMembers: profileState.setOnlineMembers,
    setWorkspaces: workspaceChannelState.setWorkspaces,
    setMessages: messageState.setMessages,
    setReactionState: messageState.setReactionState,
    isMessageListNearBottom: messageListController.isMessageListNearBottom,
    scrollMessageListToBottom: messageListController.scrollMessageListToBottom,
  });

  const workspaceChannelOperations =
    createWorkspaceChannelOperationsController({
      session: auth.session,
      activeGuildId: workspaceChannelState.activeGuildId,
      createGuildName: workspaceChannelState.createGuildName,
      createGuildVisibility: workspaceChannelState.createGuildVisibility,
      createChannelName: workspaceChannelState.createChannelName,
      createChannelKind: workspaceChannelState.createChannelKind,
      isCreatingWorkspace: workspaceChannelState.isCreatingWorkspace,
      isCreatingChannel: workspaceChannelState.isCreatingChannel,
      newChannelName: workspaceChannelState.newChannelName,
      newChannelKind: workspaceChannelState.newChannelKind,
      setWorkspaces: workspaceChannelState.setWorkspaces,
      setActiveGuildId: workspaceChannelState.setActiveGuildId,
      setActiveChannelId: workspaceChannelState.setActiveChannelId,
      setCreateChannelKind: workspaceChannelState.setCreateChannelKind,
      setWorkspaceError: workspaceChannelState.setWorkspaceError,
      setCreatingWorkspace: workspaceChannelState.setCreatingWorkspace,
      setMessageStatus: messageState.setMessageStatus,
      setActiveOverlayPanel: overlayState.setActiveOverlayPanel,
      setChannelCreateError: workspaceChannelState.setChannelCreateError,
      setCreatingChannel: workspaceChannelState.setCreatingChannel,
      setNewChannelName: workspaceChannelState.setNewChannelName,
      setNewChannelKind: workspaceChannelState.setNewChannelKind,
    });

  createVoiceSessionLifecycleController({
    session: auth.session,
    workspaces: workspaceChannelState.workspaces,
    rtcSnapshot: voiceState.rtcSnapshot,
    isVoiceSessionActive: selectors.isVoiceSessionActive,
    voiceSessionChannelKey: voiceState.voiceSessionChannelKey,
    voiceSessionStartedAtUnixMs: voiceState.voiceSessionStartedAtUnixMs,
    isJoiningVoice: voiceState.isJoiningVoice,
    isLeavingVoice: voiceState.isLeavingVoice,
    leaveVoiceChannel: () => leaveVoiceChannel(),
    setVoiceDurationClockUnixMs: voiceState.setVoiceDurationClockUnixMs,
    setVoiceSessionChannelKey: voiceState.setVoiceSessionChannelKey,
    setVoiceSessionStartedAtUnixMs: voiceState.setVoiceSessionStartedAtUnixMs,
    setVoiceSessionCapabilities: voiceState.setVoiceSessionCapabilities,
    defaultVoiceSessionCapabilities: DEFAULT_VOICE_SESSION_CAPABILITIES,
    setVoiceStatus: voiceState.setVoiceStatus,
    setVoiceError: voiceState.setVoiceError,
  });

  const sessionDiagnostics = createSessionDiagnosticsController({
    session: auth.session,
    setAuthenticatedSession: auth.setAuthenticatedSession,
    clearAuthenticatedSession: auth.clearAuthenticatedSession,
    leaveVoiceChannel: () => leaveVoiceChannel(),
    releaseRtcClient: () => releaseRtcClient(),
    isRefreshingSession: diagnosticsState.isRefreshingSession,
    setRefreshingSession: diagnosticsState.setRefreshingSession,
    setSessionStatus: diagnosticsState.setSessionStatus,
    setSessionError: diagnosticsState.setSessionError,
    isCheckingHealth: diagnosticsState.isCheckingHealth,
    setCheckingHealth: diagnosticsState.setCheckingHealth,
    setHealthStatus: diagnosticsState.setHealthStatus,
    setDiagError: diagnosticsState.setDiagError,
    isEchoing: diagnosticsState.isEchoing,
    setEchoing: diagnosticsState.setEchoing,
    echoInput: diagnosticsState.echoInput,
  });

  onCleanup(() => {
    void releaseRtcClient();
  });

  const panelHostPropGroups = () =>
    buildPanelHostPropGroups({
      workspaceCreate: {
        createGuildName: workspaceChannelState.createGuildName(),
        createGuildVisibility: workspaceChannelState.createGuildVisibility(),
        createChannelName: workspaceChannelState.createChannelName(),
        createChannelKind: workspaceChannelState.createChannelKind(),
        isCreatingWorkspace: workspaceChannelState.isCreatingWorkspace(),
        canDismissWorkspaceCreateForm: selectors.canDismissWorkspaceCreateForm(),
        workspaceError: workspaceChannelState.workspaceError(),
        onCreateWorkspaceSubmit: workspaceChannelOperations.createWorkspace,
        setCreateGuildName: workspaceChannelState.setCreateGuildName,
        setCreateGuildVisibility: workspaceChannelState.setCreateGuildVisibility,
        setCreateChannelName: workspaceChannelState.setCreateChannelName,
        setCreateChannelKind: workspaceChannelState.setCreateChannelKind,
        onCancelWorkspaceCreate: closeOverlayPanel,
      },
      channelCreate: {
        newChannelName: workspaceChannelState.newChannelName(),
        newChannelKind: workspaceChannelState.newChannelKind(),
        isCreatingChannel: workspaceChannelState.isCreatingChannel(),
        channelCreateError: workspaceChannelState.channelCreateError(),
        onCreateChannelSubmit: workspaceChannelOperations.createNewChannel,
        setNewChannelName: workspaceChannelState.setNewChannelName,
        setNewChannelKind: workspaceChannelState.setNewChannelKind,
        onCancelChannelCreate: closeOverlayPanel,
      },
      publicDirectory: {
        publicGuildSearchQuery: discoveryState.publicGuildSearchQuery(),
        isSearchingPublicGuilds: discoveryState.isSearchingPublicGuilds(),
        publicGuildSearchError: discoveryState.publicGuildSearchError(),
        publicGuildDirectory: discoveryState.publicGuildDirectory(),
        publicGuildJoinStatusByGuildId: discoveryState.publicGuildJoinStatusByGuildId(),
        publicGuildJoinErrorByGuildId: discoveryState.publicGuildJoinErrorByGuildId(),
        onSubmitPublicGuildSearch: publicDirectoryActions.runPublicGuildSearch,
        onJoinGuildFromDirectory: (guildId) =>
          publicDirectoryActions.joinGuildFromDirectory(guildId),
        setPublicGuildSearchQuery: discoveryState.setPublicGuildSearchQuery,
      },
      settings: {
        activeSettingsCategory: overlayState.activeSettingsCategory(),
        activeVoiceSettingsSubmenu: overlayState.activeVoiceSettingsSubmenu(),
        voiceDevicePreferences: voiceState.voiceDevicePreferences(),
        audioInputDevices: voiceState.audioInputDevices(),
        audioOutputDevices: voiceState.audioOutputDevices(),
        isRefreshingAudioDevices: voiceState.isRefreshingAudioDevices(),
        audioDevicesStatus: voiceState.audioDevicesStatus(),
        audioDevicesError: voiceState.audioDevicesError(),
        profile: profileController.profile() ?? null,
        profileDraftUsername: profileState.profileDraftUsername(),
        profileDraftAbout: profileState.profileDraftAbout(),
        profileAvatarUrl: profileController.profile()
          ? profileController.avatarUrlForUser(profileController.profile()!.userId)
          : null,
        selectedAvatarFilename: profileState.selectedProfileAvatarFile()?.name ?? "",
        isSavingProfile: profileState.isSavingProfile(),
        isUploadingProfileAvatar: profileState.isUploadingProfileAvatar(),
        profileSettingsStatus: profileState.profileSettingsStatus(),
        profileSettingsError: profileState.profileSettingsError(),
        onOpenSettingsCategory: openSettingsCategory,
        onOpenVoiceSettingsSubmenu: overlayState.setActiveVoiceSettingsSubmenu,
        onSetVoiceDevicePreference: (kind, value) =>
          setVoiceDevicePreference(kind, value),
        onRefreshAudioDeviceInventory: () => refreshAudioDeviceInventory(true),
        setProfileDraftUsername: profileState.setProfileDraftUsername,
        setProfileDraftAbout: profileState.setProfileDraftAbout,
        setSelectedProfileAvatarFile: profileState.setSelectedProfileAvatarFile,
        onSaveProfileSettings: profileController.saveProfileSettings,
        onUploadProfileAvatar: profileController.uploadProfileAvatar,
      },
      friendships: {
        friendRecipientUserIdInput: friendshipsState.friendRecipientUserIdInput(),
        friendRequests: friendshipsState.friendRequests(),
        friends: friendshipsState.friends(),
        isRunningFriendAction: friendshipsState.isRunningFriendAction(),
        friendStatus: friendshipsState.friendStatus(),
        friendError: friendshipsState.friendError(),
        onSubmitFriendRequest: friendshipActions.submitFriendRequest,
        setFriendRecipientUserIdInput: friendshipsState.setFriendRecipientUserIdInput,
        onAcceptIncomingFriendRequest: (requestId) =>
          friendshipActions.acceptIncomingFriendRequest(requestId),
        onDismissFriendRequest: (requestId) =>
          friendshipActions.dismissFriendRequest(requestId),
        onRemoveFriendship: (friendUserId) =>
          friendshipActions.removeFriendship(friendUserId),
      },
      search: {
        searchQuery: discoveryState.searchQuery(),
        isSearching: discoveryState.isSearching(),
        hasActiveWorkspace: Boolean(selectors.activeWorkspace()),
        canManageSearchMaintenance: selectors.canManageSearchMaintenance(),
        isRunningSearchOps: discoveryState.isRunningSearchOps(),
        searchOpsStatus: discoveryState.searchOpsStatus(),
        searchError: discoveryState.searchError(),
        searchResults: discoveryState.searchResults(),
        onSubmitSearch: searchActions.runSearch,
        setSearchQuery: discoveryState.setSearchQuery,
        onRebuildSearch: searchActions.rebuildSearch,
        onReconcileSearch: searchActions.reconcileSearch,
        displayUserLabel: labels.displayUserLabel,
      },
      attachments: {
        attachmentFilename: messageState.attachmentFilename(),
        activeAttachments: selectors.activeAttachments(),
        isUploadingAttachment: messageState.isUploadingAttachment(),
        hasActiveChannel: Boolean(selectors.activeChannel()),
        attachmentStatus: messageState.attachmentStatus(),
        attachmentError: messageState.attachmentError(),
        downloadingAttachmentId: messageState.downloadingAttachmentId(),
        deletingAttachmentId: messageState.deletingAttachmentId(),
        onSubmitUploadAttachment: attachmentActions.uploadAttachment,
        setSelectedAttachment: messageState.setSelectedAttachment,
        setAttachmentFilename: messageState.setAttachmentFilename,
        onDownloadAttachment: (record) => attachmentActions.downloadAttachment(record),
        onRemoveAttachment: (record) => attachmentActions.removeAttachment(record),
      },
      moderation: {
        moderationUserIdInput: diagnosticsState.moderationUserIdInput(),
        moderationRoleInput: diagnosticsState.moderationRoleInput(),
        overrideRoleInput: diagnosticsState.overrideRoleInput(),
        overrideAllowCsv: diagnosticsState.overrideAllowCsv(),
        overrideDenyCsv: diagnosticsState.overrideDenyCsv(),
        isModerating: diagnosticsState.isModerating(),
        hasActiveWorkspace: Boolean(selectors.activeWorkspace()),
        hasActiveChannel: Boolean(selectors.activeChannel()),
        canManageRoles: selectors.canManageRoles(),
        canBanMembers: selectors.canBanMembers(),
        canManageChannelOverrides: selectors.canManageChannelOverrides(),
        moderationStatus: diagnosticsState.moderationStatus(),
        moderationError: diagnosticsState.moderationError(),
        setModerationUserIdInput: diagnosticsState.setModerationUserIdInput,
        setModerationRoleInput: diagnosticsState.setModerationRoleInput,
        onRunMemberAction: (action) => moderationActions.runMemberAction(action),
        setOverrideRoleInput: diagnosticsState.setOverrideRoleInput,
        setOverrideAllowCsv: diagnosticsState.setOverrideAllowCsv,
        setOverrideDenyCsv: diagnosticsState.setOverrideDenyCsv,
        onApplyOverride: moderationActions.applyOverride,
        onOpenRoleManagementPanel: () => openOverlayPanel("role-management"),
      },
      roleManagement: {
        hasActiveWorkspace: Boolean(selectors.activeWorkspace()),
        canManageWorkspaceRoles: selectors.canManageWorkspaceRoles(),
        canManageMemberRoles: selectors.canManageMemberRoles(),
        roles: roleManagementActions.roles(),
        isLoadingRoles: roleManagementActions.isLoadingRoles(),
        isMutatingRoles: roleManagementActions.isMutatingRoles(),
        roleManagementStatus: roleManagementActions.roleManagementStatus(),
        roleManagementError: roleManagementActions.roleManagementError(),
        targetUserIdInput: diagnosticsState.moderationUserIdInput(),
        setTargetUserIdInput: diagnosticsState.setModerationUserIdInput,
        onRefreshRoles: roleManagementActions.refreshRoles,
        onCreateRole: roleManagementActions.createRole,
        onUpdateRole: roleManagementActions.updateRole,
        onDeleteRole: roleManagementActions.deleteRole,
        onReorderRoles: roleManagementActions.reorderRoles,
        onAssignRole: roleManagementActions.assignRoleToMember,
        onUnassignRole: roleManagementActions.unassignRoleFromMember,
        onOpenModerationPanel: () => openOverlayPanel("moderation"),
      },
      utility: {
        echoInput: diagnosticsState.echoInput(),
        healthStatus: diagnosticsState.healthStatus(),
        diagError: diagnosticsState.diagError(),
        isCheckingHealth: diagnosticsState.isCheckingHealth(),
        isEchoing: diagnosticsState.isEchoing(),
        setEchoInput: diagnosticsState.setEchoInput,
        onRunHealthCheck: sessionDiagnostics.runHealthCheck,
        onRunEcho: sessionDiagnostics.runEcho,
      },
    });

  const openTextChannelCreatePanel = (): void => {
    workspaceChannelState.setNewChannelKind(channelKindFromInput("text"));
    openOverlayPanel("channel-create");
  };

  const openVoiceChannelCreatePanel = (): void => {
    workspaceChannelState.setNewChannelKind(channelKindFromInput("voice"));
    openOverlayPanel("channel-create");
  };

  const onSelectWorkspace = (
    guildId: GuildId,
    firstChannelId: ChannelId | null,
  ): void => {
    workspaceChannelState.setActiveGuildId(guildId);
    workspaceChannelState.setActiveChannelId(firstChannelId);
  };

  return {
    workspaceState,
    messageState,
    profileState,
    voiceState,
    diagnosticsState,
    overlayState,
    selectors,
    reactionPickerController,
    messageListController,
    messageMediaPreviewController,
    messageActions,
    searchActions,
    attachmentActions,
    moderationActions,
    roleManagementActions,
    profileController,
    publicDirectoryActions,
    friendshipActions,
    workspaceChannelOperations,
    messageHistoryActions,
    sessionDiagnostics,
    panelHostPropGroups,
    openOverlayPanel,
    closeOverlayPanel,
    openSettingsCategory,
    refreshAudioDeviceInventory,
    setVoiceDevicePreference,
    releaseRtcClient,
    joinVoiceChannel,
    leaveVoiceChannel,
    toggleVoiceMicrophone,
    toggleVoiceCamera,
    toggleVoiceScreenShare,
    setComposerAttachmentInputRef,
    actorLookupId: labels.actorLookupId,
    actorLabel: labels.actorLabel,
    displayUserLabel: labels.displayUserLabel,
    voiceParticipantLabel: labels.voiceParticipantLabel,
    userIdFromVoiceIdentity,
    openTextChannelCreatePanel,
    openVoiceChannelCreatePanel,
    onSelectWorkspace,
  };
}
