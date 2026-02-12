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
  enumerateAudioDevices,
  reconcileVoiceDevicePreferences,
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
  const messageState = createMessageState();
  const profileState = createProfileState();
  const voiceState = createVoiceState();
  const diagnosticsState = createDiagnosticsState();
  const overlayState = createOverlayState();

  const selectors = createAppShellSelectors({
    workspaces: workspaceState.workspaces,
    activeGuildId: workspaceState.activeGuildId,
    activeChannelId: workspaceState.activeChannelId,
    channelPermissions: workspaceState.channelPermissions,
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
    activeGuildId: workspaceState.activeGuildId,
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

  const refreshAudioDeviceInventory = async (): Promise<void> => {
    if (voiceState.isRefreshingAudioDevices()) {
      return;
    }
    voiceState.setRefreshingAudioDevices(true);
    voiceState.setAudioDevicesError("");
    try {
      const inventory = await enumerateAudioDevices();
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
      setWorkspaceError: workspaceState.setWorkspaceError,
      setChannelCreateError: workspaceState.setChannelCreateError,
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
    activeGuildId: workspaceState.activeGuildId,
    activeChannelId: workspaceState.activeChannelId,
    setWorkspaces: workspaceState.setWorkspaces,
    setActiveGuildId: workspaceState.setActiveGuildId,
    setActiveChannelId: workspaceState.setActiveChannelId,
    setWorkspaceBootstrapDone: workspaceState.setWorkspaceBootstrapDone,
  });

  createWorkspaceSelectionController({
    workspaces: workspaceState.workspaces,
    activeGuildId: workspaceState.activeGuildId,
    activeChannelId: workspaceState.activeChannelId,
    setActiveGuildId: workspaceState.setActiveGuildId,
    setActiveChannelId: workspaceState.setActiveChannelId,
  });

  createChannelPermissionsController({
    session: auth.session,
    activeGuildId: workspaceState.activeGuildId,
    activeChannelId: workspaceState.activeChannelId,
    setWorkspaces: workspaceState.setWorkspaces,
    setChannelPermissions: workspaceState.setChannelPermissions,
  });

  createOverlayPanelAuthorizationController({
    panel: overlayState.activeOverlayPanel,
    context: () => ({
      canAccessActiveChannel: selectors.canAccessActiveChannel(),
      canManageWorkspaceChannels: selectors.canManageWorkspaceChannels(),
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
    activeGuildId: workspaceState.activeGuildId,
    activeChannelId: workspaceState.activeChannelId,
    messages: messageState.messages,
  });

  const messageActions = createMessageActionsController({
    session: auth.session,
    activeGuildId: workspaceState.activeGuildId,
    activeChannelId: workspaceState.activeChannelId,
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
    activeGuildId: workspaceState.activeGuildId,
    activeChannelId: workspaceState.activeChannelId,
    searchQuery: workspaceState.searchQuery,
    isSearching: workspaceState.isSearching,
    setSearching: workspaceState.setSearching,
    setSearchError: workspaceState.setSearchError,
    setSearchResults: workspaceState.setSearchResults,
    isRunningSearchOps: workspaceState.isRunningSearchOps,
    setRunningSearchOps: workspaceState.setRunningSearchOps,
    setSearchOpsStatus: workspaceState.setSearchOpsStatus,
  });

  const attachmentActions = createAttachmentController({
    session: auth.session,
    activeGuildId: workspaceState.activeGuildId,
    activeChannelId: workspaceState.activeChannelId,
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
    activeGuildId: workspaceState.activeGuildId,
    activeChannelId: workspaceState.activeChannelId,
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
    publicGuildSearchQuery: workspaceState.publicGuildSearchQuery,
    isSearchingPublicGuilds: workspaceState.isSearchingPublicGuilds,
    setSearchingPublicGuilds: workspaceState.setSearchingPublicGuilds,
    setPublicGuildSearchError: workspaceState.setPublicGuildSearchError,
    setPublicGuildDirectory: workspaceState.setPublicGuildDirectory,
  });

  const friendshipActions = createFriendshipController({
    session: auth.session,
    friendRecipientUserIdInput: workspaceState.friendRecipientUserIdInput,
    isRunningFriendAction: workspaceState.isRunningFriendAction,
    setFriends: workspaceState.setFriends,
    setFriendRequests: workspaceState.setFriendRequests,
    setRunningFriendAction: workspaceState.setRunningFriendAction,
    setFriendStatus: workspaceState.setFriendStatus,
    setFriendError: workspaceState.setFriendError,
    setFriendRecipientUserIdInput: workspaceState.setFriendRecipientUserIdInput,
  });

  const labels = createAppShellRuntimeLabels({
    resolvedUsernames: profileState.resolvedUsernames,
  });

  createIdentityResolutionController({
    session: auth.session,
    messages: messageState.messages,
    onlineMembers: profileState.onlineMembers,
    voiceRosterEntries: selectors.voiceRosterEntries,
    searchResults: workspaceState.searchResults,
    profile: profileController.profile,
    selectedProfile: profileController.selectedProfile,
    friends: workspaceState.friends,
    friendRequests: workspaceState.friendRequests,
    setResolvedUsernames: profileState.setResolvedUsernames,
    setAvatarVersionByUserId: profileState.setAvatarVersionByUserId,
  });

  createEffect(() => {
    if (!workspaceState.workspaceBootstrapDone()) {
      return;
    }
    saveWorkspaceCache(workspaceState.workspaces());
  });

  createEffect(() => {
    if (!workspaceState.workspaceBootstrapDone()) {
      return;
    }
    if (workspaceState.workspaces().length === 0) {
      overlayState.setActiveOverlayPanel("workspace-create");
    }
  });

  const messageHistoryActions = createMessageHistoryController({
    session: auth.session,
    activeGuildId: workspaceState.activeGuildId,
    activeChannelId: workspaceState.activeChannelId,
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
    setSearchResults: workspaceState.setSearchResults,
    setSearchError: workspaceState.setSearchError,
    setSearchOpsStatus: workspaceState.setSearchOpsStatus,
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
    void untrack(() => refreshAudioDeviceInventory());
  });

  createGatewayController({
    session: auth.session,
    activeGuildId: workspaceState.activeGuildId,
    activeChannelId: workspaceState.activeChannelId,
    canAccessActiveChannel: selectors.canAccessActiveChannel,
    setGatewayOnline: profileState.setGatewayOnline,
    setOnlineMembers: profileState.setOnlineMembers,
    setMessages: messageState.setMessages,
    isMessageListNearBottom: messageListController.isMessageListNearBottom,
    scrollMessageListToBottom: messageListController.scrollMessageListToBottom,
  });

  const workspaceChannelOperations =
    createWorkspaceChannelOperationsController({
      session: auth.session,
      activeGuildId: workspaceState.activeGuildId,
      createGuildName: workspaceState.createGuildName,
      createGuildVisibility: workspaceState.createGuildVisibility,
      createChannelName: workspaceState.createChannelName,
      createChannelKind: workspaceState.createChannelKind,
      isCreatingWorkspace: workspaceState.isCreatingWorkspace,
      isCreatingChannel: workspaceState.isCreatingChannel,
      newChannelName: workspaceState.newChannelName,
      newChannelKind: workspaceState.newChannelKind,
      setWorkspaces: workspaceState.setWorkspaces,
      setActiveGuildId: workspaceState.setActiveGuildId,
      setActiveChannelId: workspaceState.setActiveChannelId,
      setCreateChannelKind: workspaceState.setCreateChannelKind,
      setWorkspaceError: workspaceState.setWorkspaceError,
      setCreatingWorkspace: workspaceState.setCreatingWorkspace,
      setMessageStatus: messageState.setMessageStatus,
      setActiveOverlayPanel: overlayState.setActiveOverlayPanel,
      setChannelCreateError: workspaceState.setChannelCreateError,
      setCreatingChannel: workspaceState.setCreatingChannel,
      setNewChannelName: workspaceState.setNewChannelName,
      setNewChannelKind: workspaceState.setNewChannelKind,
    });

  createVoiceSessionLifecycleController({
    session: auth.session,
    workspaces: workspaceState.workspaces,
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
      createGuildName: workspaceState.createGuildName(),
      createGuildVisibility: workspaceState.createGuildVisibility(),
      createChannelName: workspaceState.createChannelName(),
      createChannelKind: workspaceState.createChannelKind(),
      isCreatingWorkspace: workspaceState.isCreatingWorkspace(),
      canDismissWorkspaceCreateForm: selectors.canDismissWorkspaceCreateForm(),
      workspaceError: workspaceState.workspaceError(),
      onCreateWorkspaceSubmit: workspaceChannelOperations.createWorkspace,
      setCreateGuildName: workspaceState.setCreateGuildName,
      setCreateGuildVisibility: workspaceState.setCreateGuildVisibility,
      setCreateChannelName: workspaceState.setCreateChannelName,
      setCreateChannelKind: workspaceState.setCreateChannelKind,
      onCancelWorkspaceCreate: closeOverlayPanel,
      newChannelName: workspaceState.newChannelName(),
      newChannelKind: workspaceState.newChannelKind(),
      isCreatingChannel: workspaceState.isCreatingChannel(),
      channelCreateError: workspaceState.channelCreateError(),
      onCreateChannelSubmit: workspaceChannelOperations.createNewChannel,
      setNewChannelName: workspaceState.setNewChannelName,
      setNewChannelKind: workspaceState.setNewChannelKind,
      onCancelChannelCreate: closeOverlayPanel,
      publicGuildSearchQuery: workspaceState.publicGuildSearchQuery(),
      isSearchingPublicGuilds: workspaceState.isSearchingPublicGuilds(),
      publicGuildSearchError: workspaceState.publicGuildSearchError(),
      publicGuildDirectory: workspaceState.publicGuildDirectory(),
      onSubmitPublicGuildSearch: publicDirectoryActions.runPublicGuildSearch,
      setPublicGuildSearchQuery: workspaceState.setPublicGuildSearchQuery,
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
      onRefreshAudioDeviceInventory: refreshAudioDeviceInventory,
      setProfileDraftUsername: profileState.setProfileDraftUsername,
      setProfileDraftAbout: profileState.setProfileDraftAbout,
      setSelectedProfileAvatarFile: profileState.setSelectedProfileAvatarFile,
      onSaveProfileSettings: profileController.saveProfileSettings,
      onUploadProfileAvatar: profileController.uploadProfileAvatar,
      friendRecipientUserIdInput: workspaceState.friendRecipientUserIdInput(),
      friendRequests: workspaceState.friendRequests(),
      friends: workspaceState.friends(),
      isRunningFriendAction: workspaceState.isRunningFriendAction(),
      friendStatus: workspaceState.friendStatus(),
      friendError: workspaceState.friendError(),
      onSubmitFriendRequest: friendshipActions.submitFriendRequest,
      setFriendRecipientUserIdInput: workspaceState.setFriendRecipientUserIdInput,
      onAcceptIncomingFriendRequest: (requestId) =>
        friendshipActions.acceptIncomingFriendRequest(requestId),
      onDismissFriendRequest: (requestId) =>
        friendshipActions.dismissFriendRequest(requestId),
      onRemoveFriendship: (friendUserId) =>
        friendshipActions.removeFriendship(friendUserId),
      searchQuery: workspaceState.searchQuery(),
      isSearching: workspaceState.isSearching(),
      hasActiveWorkspace: Boolean(selectors.activeWorkspace()),
      canManageSearchMaintenance: selectors.canManageSearchMaintenance(),
      isRunningSearchOps: workspaceState.isRunningSearchOps(),
      searchOpsStatus: workspaceState.searchOpsStatus(),
      searchError: workspaceState.searchError(),
      searchResults: workspaceState.searchResults(),
      onSubmitSearch: searchActions.runSearch,
      setSearchQuery: workspaceState.setSearchQuery,
      onRebuildSearch: searchActions.rebuildSearch,
      onReconcileSearch: searchActions.reconcileSearch,
      displayUserLabel: labels.displayUserLabel,
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
      moderationUserIdInput: diagnosticsState.moderationUserIdInput(),
      moderationRoleInput: diagnosticsState.moderationRoleInput(),
      overrideRoleInput: diagnosticsState.overrideRoleInput(),
      overrideAllowCsv: diagnosticsState.overrideAllowCsv(),
      overrideDenyCsv: diagnosticsState.overrideDenyCsv(),
      isModerating: diagnosticsState.isModerating(),
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
      echoInput: diagnosticsState.echoInput(),
      healthStatus: diagnosticsState.healthStatus(),
      diagError: diagnosticsState.diagError(),
      isCheckingHealth: diagnosticsState.isCheckingHealth(),
      isEchoing: diagnosticsState.isEchoing(),
      setEchoInput: diagnosticsState.setEchoInput,
      onRunHealthCheck: sessionDiagnostics.runHealthCheck,
      onRunEcho: sessionDiagnostics.runEcho,
    });

  const openTextChannelCreatePanel = (): void => {
    workspaceState.setNewChannelKind(channelKindFromInput("text"));
    openOverlayPanel("channel-create");
  };

  const openVoiceChannelCreatePanel = (): void => {
    workspaceState.setNewChannelKind(channelKindFromInput("voice"));
    openOverlayPanel("channel-create");
  };

  const onSelectWorkspace = (
    guildId: GuildId,
    firstChannelId: ChannelId | null,
  ): void => {
    workspaceState.setActiveGuildId(guildId);
    workspaceState.setActiveChannelId(firstChannelId);
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
