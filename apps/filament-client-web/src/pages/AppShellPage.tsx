import {
  createEffect,
  onCleanup,
  untrack,
} from "solid-js";
import {
  channelKindFromInput,
  channelNameFromInput,
  guildVisibilityFromInput,
  guildNameFromInput,
  type WorkspaceRecord,
} from "../domain/chat";
import {
  createChannel,
  createGuild,
  echoMessage,
  fetchHealth,
  logoutAuthSession,
  refreshAuthSession,
} from "../lib/api";
import { useAuth } from "../lib/auth-context";
import {
  mapError,
  mapRtcError,
  profileErrorMessage,
  shortActor,
  upsertWorkspace,
  userIdFromVoiceIdentity,
} from "../features/app-shell/helpers";
import { AppShellLayout } from "../features/app-shell/components/layout/AppShellLayout";
import { ChatColumn } from "../features/app-shell/components/layout/ChatColumn";
import { ChannelRail } from "../features/app-shell/components/ChannelRail";
import { ChatHeader } from "../features/app-shell/components/ChatHeader";
import { MemberRail } from "../features/app-shell/components/MemberRail";
import { MessageComposer } from "../features/app-shell/components/messages/MessageComposer";
import { MessageList } from "../features/app-shell/components/messages/MessageList";
import { ReactionPickerPortal } from "../features/app-shell/components/messages/ReactionPickerPortal";
import { ServerRail } from "../features/app-shell/components/ServerRail";
import { PanelHost } from "../features/app-shell/components/panels/PanelHost";
import { UserProfileOverlay } from "../features/app-shell/components/overlays/UserProfileOverlay";
import { buildPanelHostPropGroups } from "../features/app-shell/adapters/panel-host-props";
import { OPENMOJI_REACTION_OPTIONS } from "../features/app-shell/config/reaction-options";
import {
  ADD_REACTION_ICON_URL,
  DELETE_MESSAGE_ICON_URL,
  EDIT_MESSAGE_ICON_URL,
} from "../features/app-shell/config/ui-constants";
import { createAttachmentController } from "../features/app-shell/controllers/attachment-controller";
import { createMessageListController } from "../features/app-shell/controllers/message-list-controller";
import { createModerationController } from "../features/app-shell/controllers/moderation-controller";
import {
  createFriendshipController,
} from "../features/app-shell/controllers/friendship-controller";
import { createGatewayController } from "../features/app-shell/controllers/gateway-controller";
import {
  createIdentityResolutionController,
} from "../features/app-shell/controllers/identity-resolution-controller";
import {
  createMessageHistoryController,
} from "../features/app-shell/controllers/message-history-controller";
import {
  createMessageActionsController,
  createMessageMediaPreviewController,
} from "../features/app-shell/controllers/message-controller";
import {
  createOverlayPanelAuthorizationController,
  createOverlayPanelEscapeController,
  openOverlayPanelWithDefaults,
  overlayPanelClassName,
  overlayPanelTitle,
} from "../features/app-shell/controllers/overlay-controller";
import { createProfileController } from "../features/app-shell/controllers/profile-controller";
import { createProfileOverlayController } from "../features/app-shell/controllers/profile-overlay-controller";
import { createPublicDirectoryController } from "../features/app-shell/controllers/public-directory-controller";
import { createReactionPickerController } from "../features/app-shell/controllers/reaction-picker-controller";
import {
  createVoiceOperationsController,
} from "../features/app-shell/controllers/voice-operations-controller";
import {
  createVoiceSessionLifecycleController,
  resolveVoiceDevicePreferenceStatus,
  unavailableVoiceDeviceError,
} from "../features/app-shell/controllers/voice-controller";
import { createSearchController } from "../features/app-shell/controllers/search-controller";
import {
  createChannelPermissionsController,
  createWorkspaceBootstrapController,
  createWorkspaceSelectionController,
} from "../features/app-shell/controllers/workspace-controller";
import { createDiagnosticsState } from "../features/app-shell/state/diagnostics-state";
import { createMessageState } from "../features/app-shell/state/message-state";
import { createOverlayState } from "../features/app-shell/state/overlay-state";
import { createProfileState } from "../features/app-shell/state/profile-state";
import {
  createVoiceState,
  DEFAULT_VOICE_SESSION_CAPABILITIES,
} from "../features/app-shell/state/voice-state";
import { createWorkspaceState } from "../features/app-shell/state/workspace-state";
import { createAppShellSelectors } from "../features/app-shell/selectors/create-app-shell-selectors";
import {
  type OverlayPanel,
  type SettingsCategory,
} from "../features/app-shell/types";
import {
  enumerateAudioDevices,
  reconcileVoiceDevicePreferences,
  saveVoiceDevicePreferences,
  type MediaDeviceId,
  type VoiceDevicePreferences,
} from "../lib/voice-device-settings";
import { clearWorkspaceCache, saveWorkspaceCache } from "../lib/workspace-cache";

export function AppShellPage() {
  const auth = useAuth();
  let composerAttachmentInputRef: HTMLInputElement | undefined;

  const {
    workspaces,
    setWorkspaces,
    activeGuildId,
    setActiveGuildId,
    activeChannelId,
    setActiveChannelId,
    workspaceBootstrapDone,
    setWorkspaceBootstrapDone,
    createGuildName,
    setCreateGuildName,
    createGuildVisibility,
    setCreateGuildVisibility,
    createChannelName,
    setCreateChannelName,
    createChannelKind,
    setCreateChannelKind,
    isCreatingWorkspace,
    setCreatingWorkspace,
    workspaceError,
    setWorkspaceError,
    publicGuildSearchQuery,
    setPublicGuildSearchQuery,
    isSearchingPublicGuilds,
    setSearchingPublicGuilds,
    publicGuildSearchError,
    setPublicGuildSearchError,
    publicGuildDirectory,
    setPublicGuildDirectory,
    friendRecipientUserIdInput,
    setFriendRecipientUserIdInput,
    friends,
    setFriends,
    friendRequests,
    setFriendRequests,
    isRunningFriendAction,
    setRunningFriendAction,
    friendStatus,
    setFriendStatus,
    friendError,
    setFriendError,
    newChannelName,
    setNewChannelName,
    newChannelKind,
    setNewChannelKind,
    isCreatingChannel,
    setCreatingChannel,
    channelCreateError,
    setChannelCreateError,
    searchQuery,
    setSearchQuery,
    searchError,
    setSearchError,
    isSearching,
    setSearching,
    searchResults,
    setSearchResults,
    isRunningSearchOps,
    setRunningSearchOps,
    searchOpsStatus,
    setSearchOpsStatus,
    channelPermissions,
    setChannelPermissions,
  } = createWorkspaceState();
  const {
    composer,
    setComposer,
    messageStatus,
    setMessageStatus,
    messageError,
    setMessageError,
    isLoadingMessages,
    setLoadingMessages,
    isLoadingOlder,
    setLoadingOlder,
    isSendingMessage,
    setSendingMessage,
    messages,
    setMessages,
    nextBefore,
    setNextBefore,
    showLoadOlderButton,
    setShowLoadOlderButton,
    reactionState,
    setReactionState,
    pendingReactionByKey,
    setPendingReactionByKey,
    openReactionPickerMessageId,
    setOpenReactionPickerMessageId,
    reactionPickerOverlayPosition,
    setReactionPickerOverlayPosition,
    editingMessageId,
    setEditingMessageId,
    editingDraft,
    setEditingDraft,
    isSavingEdit,
    setSavingEdit,
    deletingMessageId,
    setDeletingMessageId,
    composerAttachments,
    setComposerAttachments,
    attachmentByChannel,
    setAttachmentByChannel,
    selectedAttachment,
    setSelectedAttachment,
    attachmentFilename,
    setAttachmentFilename,
    attachmentStatus,
    setAttachmentStatus,
    attachmentError,
    setAttachmentError,
    isUploadingAttachment,
    setUploadingAttachment,
    downloadingAttachmentId,
    setDownloadingAttachmentId,
    deletingAttachmentId,
    setDeletingAttachmentId,
  } = createMessageState();
  const {
    gatewayOnline,
    setGatewayOnline,
    onlineMembers,
    setOnlineMembers,
    resolvedUsernames,
    setResolvedUsernames,
    avatarVersionByUserId,
    setAvatarVersionByUserId,
    profileDraftUsername,
    setProfileDraftUsername,
    profileDraftAbout,
    setProfileDraftAbout,
    selectedProfileAvatarFile,
    setSelectedProfileAvatarFile,
    profileSettingsStatus,
    setProfileSettingsStatus,
    profileSettingsError,
    setProfileSettingsError,
    isSavingProfile,
    setSavingProfile,
    isUploadingProfileAvatar,
    setUploadingProfileAvatar,
    selectedProfileUserId,
    setSelectedProfileUserId,
    selectedProfileError,
    setSelectedProfileError,
  } = createProfileState();
  const {
    rtcSnapshot,
    setRtcSnapshot,
    voiceStatus,
    setVoiceStatus,
    voiceError,
    setVoiceError,
    isJoiningVoice,
    setJoiningVoice,
    isLeavingVoice,
    setLeavingVoice,
    isTogglingVoiceMic,
    setTogglingVoiceMic,
    isTogglingVoiceCamera,
    setTogglingVoiceCamera,
    isTogglingVoiceScreenShare,
    setTogglingVoiceScreenShare,
    voiceSessionChannelKey,
    setVoiceSessionChannelKey,
    voiceSessionStartedAtUnixMs,
    setVoiceSessionStartedAtUnixMs,
    voiceDurationClockUnixMs,
    setVoiceDurationClockUnixMs,
    voiceSessionCapabilities,
    setVoiceSessionCapabilities,
    voiceDevicePreferences,
    setVoiceDevicePreferences,
    audioInputDevices,
    setAudioInputDevices,
    audioOutputDevices,
    setAudioOutputDevices,
    isRefreshingAudioDevices,
    setRefreshingAudioDevices,
    audioDevicesStatus,
    setAudioDevicesStatus,
    audioDevicesError,
    setAudioDevicesError,
  } = createVoiceState();
  const {
    moderationUserIdInput,
    setModerationUserIdInput,
    moderationRoleInput,
    setModerationRoleInput,
    isModerating,
    setModerating,
    moderationStatus,
    setModerationStatus,
    moderationError,
    setModerationError,
    overrideRoleInput,
    setOverrideRoleInput,
    overrideAllowCsv,
    setOverrideAllowCsv,
    overrideDenyCsv,
    setOverrideDenyCsv,
    isRefreshingSession,
    setRefreshingSession,
    sessionStatus,
    setSessionStatus,
    sessionError,
    setSessionError,
    healthStatus,
    setHealthStatus,
    echoInput,
    setEchoInput,
    diagError,
    setDiagError,
    isCheckingHealth,
    setCheckingHealth,
    isEchoing,
    setEchoing,
  } = createDiagnosticsState();
  const {
    activeOverlayPanel,
    setActiveOverlayPanel,
    activeSettingsCategory,
    setActiveSettingsCategory,
    activeVoiceSettingsSubmenu,
    setActiveVoiceSettingsSubmenu,
    isChannelRailCollapsed,
    setChannelRailCollapsed,
    isMemberRailCollapsed,
    setMemberRailCollapsed,
  } = createOverlayState();

  const {
    activeWorkspace,
    activeChannel,
    activeTextChannels,
    activeVoiceChannels,
    canAccessActiveChannel,
    canPublishVoiceCamera,
    canPublishVoiceScreenShare,
    canSubscribeVoiceStreams,
    canManageWorkspaceChannels,
    canManageSearchMaintenance,
    canManageRoles,
    canManageChannelOverrides,
    canBanMembers,
    canDeleteMessages,
    hasModerationAccess,
    canDismissWorkspaceCreateForm,
    activeVoiceSessionLabel,
    activeAttachments,
    voiceConnectionState,
    isVoiceSessionActive,
    isVoiceSessionForChannel,
    canToggleVoiceCamera,
    canToggleVoiceScreenShare,
    canShowVoiceHeaderControls,
    voiceRosterEntries,
    voiceStreamPermissionHints,
    voiceSessionDurationLabel,
    canCloseActivePanel,
  } = createAppShellSelectors({
    workspaces,
    activeGuildId,
    activeChannelId,
    channelPermissions,
    voiceSessionChannelKey,
    attachmentByChannel,
    rtcSnapshot,
    voiceSessionCapabilities,
    voiceSessionStartedAtUnixMs,
    voiceDurationClockUnixMs,
    activeOverlayPanel,
  });

  const openSettingsCategory = (category: SettingsCategory): void => {
    setActiveSettingsCategory(category);
    if (category === "voice") {
      setActiveVoiceSettingsSubmenu("audio-devices");
    }
  };

  const reactionPickerController = createReactionPickerController({
    openReactionPickerMessageId,
    setOpenReactionPickerMessageId,
    setReactionPickerOverlayPosition,
    trackPositionDependencies: () => {
      void messages();
      void voiceStatus();
      void voiceError();
      void voiceRosterEntries().length;
    },
  });

  const messageListController = createMessageListController({
    nextBefore,
    isLoadingOlder,
    openReactionPickerMessageId,
    setShowLoadOlderButton,
    updateReactionPickerOverlayPosition:
      reactionPickerController.updateReactionPickerOverlayPosition,
  });

  const persistVoiceDevicePreferences = (next: VoiceDevicePreferences): void => {
    setVoiceDevicePreferences(next);
    try {
      saveVoiceDevicePreferences(next);
    } catch {
      setAudioDevicesError("Unable to persist audio device preferences in local storage.");
    }
  };

  const refreshAudioDeviceInventory = async (): Promise<void> => {
    if (isRefreshingAudioDevices()) {
      return;
    }
    setRefreshingAudioDevices(true);
    setAudioDevicesError("");
    try {
      const inventory = await enumerateAudioDevices();
      setAudioInputDevices(inventory.audioInputs);
      setAudioOutputDevices(inventory.audioOutputs);
      setAudioDevicesStatus(
        `Detected ${inventory.audioInputs.length} microphone(s) and ${inventory.audioOutputs.length} speaker(s).`,
      );
      const current = voiceDevicePreferences();
      const reconciled = reconcileVoiceDevicePreferences(current, inventory);
      if (
        current.audioInputDeviceId !== reconciled.audioInputDeviceId ||
        current.audioOutputDeviceId !== reconciled.audioOutputDeviceId
      ) {
        persistVoiceDevicePreferences(reconciled);
        setAudioDevicesStatus(
          "Some saved audio devices are no longer available. Reverted to system defaults.",
        );
      }
    } catch (error) {
      setAudioInputDevices([]);
      setAudioOutputDevices([]);
      setAudioDevicesStatus("");
      setAudioDevicesError(mapError(error, "Unable to enumerate audio devices."));
    } finally {
      setRefreshingAudioDevices(false);
    }
  };

  const setVoiceDevicePreference = async (
    kind: "audioinput" | "audiooutput",
    nextValue: string,
  ): Promise<void> => {
    const options = kind === "audioinput" ? audioInputDevices() : audioOutputDevices();
    if (nextValue.length > 0 && !options.some((entry) => entry.deviceId === nextValue)) {
      setAudioDevicesError(unavailableVoiceDeviceError(kind));
      return;
    }

    const nextDeviceId = nextValue.length > 0 ? (nextValue as MediaDeviceId) : null;
    const next: VoiceDevicePreferences =
      kind === "audioinput"
        ? {
            ...voiceDevicePreferences(),
            audioInputDeviceId: nextDeviceId,
          }
        : {
            ...voiceDevicePreferences(),
            audioOutputDeviceId: nextDeviceId,
          };
    setAudioDevicesError("");
    persistVoiceDevicePreferences(next);

    const client = voiceOperationsController.peekRtcClient();
    if (!client || !isVoiceSessionActive()) {
      setAudioDevicesStatus(
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
      setAudioDevicesStatus(
        resolveVoiceDevicePreferenceStatus(kind, true, nextDeviceId),
      );
    } catch (error) {
      setAudioDevicesError(
        mapRtcError(
          error,
          kind === "audioinput"
            ? "Unable to apply microphone selection."
            : "Unable to apply speaker selection.",
        ),
      );
    }
  };

  const openOverlayPanel = (panel: OverlayPanel) => {
    openOverlayPanelWithDefaults(panel, {
      setPanel: setActiveOverlayPanel,
      setWorkspaceError,
      setChannelCreateError,
      setActiveSettingsCategory,
      setActiveVoiceSettingsSubmenu,
    });
  };

  const closeOverlayPanel = () => {
    if (!canCloseActivePanel()) {
      return;
    }
    setActiveOverlayPanel(null);
  };

  createWorkspaceBootstrapController({
    session: auth.session,
    activeGuildId,
    activeChannelId,
    setWorkspaces,
    setActiveGuildId,
    setActiveChannelId,
    setWorkspaceBootstrapDone,
  });

  createWorkspaceSelectionController({
    workspaces,
    activeGuildId,
    activeChannelId,
    setActiveGuildId,
    setActiveChannelId,
  });

  createChannelPermissionsController({
    session: auth.session,
    activeGuildId,
    activeChannelId,
    setWorkspaces,
    setChannelPermissions,
  });

  createOverlayPanelAuthorizationController({
    panel: activeOverlayPanel,
    context: () => ({
      canAccessActiveChannel: canAccessActiveChannel(),
      canManageWorkspaceChannels: canManageWorkspaceChannels(),
      hasModerationAccess: hasModerationAccess(),
    }),
    setPanel: setActiveOverlayPanel,
  });

  createOverlayPanelEscapeController({
    panel: activeOverlayPanel,
    onEscape: closeOverlayPanel,
  });

  createProfileOverlayController({
    selectedProfileUserId,
    setSelectedProfileUserId,
  });

  const {
    messageMediaByAttachmentId,
    loadingMediaPreviewIds,
    failedMediaPreviewIds,
    retryMediaPreview,
  } = createMessageMediaPreviewController({
    session: auth.session,
    setAuthenticatedSession: auth.setAuthenticatedSession,
    activeGuildId,
    activeChannelId,
    messages,
  });

  const voiceOperationsController = createVoiceOperationsController({
    session: auth.session,
    activeGuildId,
    activeChannel,
    canPublishVoiceCamera,
    canPublishVoiceScreenShare,
    canSubscribeVoiceStreams,
    canToggleVoiceCamera,
    canToggleVoiceScreenShare,
    isJoiningVoice,
    isLeavingVoice,
    isTogglingVoiceMic,
    isTogglingVoiceCamera,
    isTogglingVoiceScreenShare,
    voiceDevicePreferences,
    setRtcSnapshot,
    setVoiceStatus,
    setVoiceError,
    setJoiningVoice,
    setLeavingVoice,
    setTogglingVoiceMic,
    setTogglingVoiceCamera,
    setTogglingVoiceScreenShare,
    setVoiceSessionChannelKey,
    setVoiceSessionStartedAtUnixMs,
    setVoiceDurationClockUnixMs,
    setVoiceSessionCapabilities,
    setAudioDevicesError,
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

  const actorLookupId = (actorId: string): string => userIdFromVoiceIdentity(actorId) ?? actorId;
  const actorLabel = (actorId: string): string => {
    const lookupId = actorLookupId(actorId);
    return resolvedUsernames()[lookupId] ?? shortActor(lookupId);
  };
  const displayUserLabel = (userId: string): string => actorLabel(userId);
  const voiceParticipantLabel = (identity: string, isLocal: boolean): string => {
    const label = actorLabel(identity);
    return isLocal ? `${label} (you)` : label;
  };

  const {
    sendMessage,
    openComposerAttachmentPicker,
    onComposerAttachmentInput,
    removeComposerAttachment,
    beginEditMessage,
    cancelEditMessage,
    saveEditMessage,
    removeMessage,
    toggleReactionPicker,
    toggleMessageReaction,
    addReactionFromPicker,
  } = createMessageActionsController({
    session: auth.session,
    activeGuildId,
    activeChannelId,
    activeChannel,
    canAccessActiveChannel,
    composer,
    setComposer,
    composerAttachments,
    setComposerAttachments,
    composerAttachmentInputElement: () => composerAttachmentInputRef,
    isSendingMessage,
    setSendingMessage,
    setMessageStatus,
    setMessageError,
    setMessages,
    setAttachmentByChannel,
    isMessageListNearBottom: messageListController.isMessageListNearBottom,
    scrollMessageListToBottom: messageListController.scrollMessageListToBottom,
    editingMessageId,
    setEditingMessageId,
    editingDraft,
    setEditingDraft,
    isSavingEdit,
    setSavingEdit,
    deletingMessageId,
    setDeletingMessageId,
    reactionState,
    setReactionState,
    pendingReactionByKey,
    setPendingReactionByKey,
    openReactionPickerMessageId,
    setOpenReactionPickerMessageId,
  });

  const { runSearch, rebuildSearch, reconcileSearch } = createSearchController({
    session: auth.session,
    activeGuildId,
    activeChannelId,
    searchQuery,
    isSearching,
    setSearching,
    setSearchError,
    setSearchResults,
    isRunningSearchOps,
    setRunningSearchOps,
    setSearchOpsStatus,
  });

  const { uploadAttachment, downloadAttachment, removeAttachment } =
    createAttachmentController({
      session: auth.session,
      activeGuildId,
      activeChannelId,
      selectedAttachment,
      attachmentFilename,
      isUploadingAttachment,
      downloadingAttachmentId,
      deletingAttachmentId,
      setAttachmentStatus,
      setAttachmentError,
      setUploadingAttachment,
      setDownloadingAttachmentId,
      setDeletingAttachmentId,
      setSelectedAttachment,
      setAttachmentFilename,
      setAttachmentByChannel,
    });

  const { runMemberAction, applyOverride } = createModerationController({
    session: auth.session,
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
  });

  const {
    profile,
    selectedProfile,
    avatarUrlForUser,
    openUserProfile,
    saveProfileSettings,
    uploadProfileAvatar,
  } = createProfileController({
    session: auth.session,
    selectedProfileUserId,
    avatarVersionByUserId,
    profileDraftUsername,
    profileDraftAbout,
    selectedProfileAvatarFile,
    isSavingProfile,
    isUploadingProfileAvatar,
    setProfileDraftUsername,
    setProfileDraftAbout,
    setSelectedProfileAvatarFile,
    setProfileSettingsStatus,
    setProfileSettingsError,
    setSavingProfile,
    setUploadingProfileAvatar,
    setSelectedProfileUserId,
    setSelectedProfileError,
  });

  const { runPublicGuildSearch } = createPublicDirectoryController({
    session: auth.session,
    publicGuildSearchQuery,
    isSearchingPublicGuilds,
    setSearchingPublicGuilds,
    setPublicGuildSearchError,
    setPublicGuildDirectory,
  });

  const {
    submitFriendRequest,
    acceptIncomingFriendRequest,
    dismissFriendRequest,
    removeFriendship,
  } = createFriendshipController({
    session: auth.session,
    friendRecipientUserIdInput,
    isRunningFriendAction,
    setFriends,
    setFriendRequests,
    setRunningFriendAction,
    setFriendStatus,
    setFriendError,
    setFriendRecipientUserIdInput,
  });

  createIdentityResolutionController({
    session: auth.session,
    messages,
    onlineMembers,
    voiceRosterEntries,
    searchResults,
    profile,
    selectedProfile,
    friends,
    friendRequests,
    setResolvedUsernames,
    setAvatarVersionByUserId,
  });

  createEffect(() => {
    if (!workspaceBootstrapDone()) {
      return;
    }
    saveWorkspaceCache(workspaces());
  });

  createEffect(() => {
    if (!workspaceBootstrapDone()) {
      return;
    }
    if (workspaces().length === 0) {
      setActiveOverlayPanel("workspace-create");
    }
  });

  const { refreshMessages, loadOlderMessages } = createMessageHistoryController({
    session: auth.session,
    activeGuildId,
    activeChannelId,
    canAccessActiveChannel,
    nextBefore,
    isLoadingOlder,
    setMessages,
    setNextBefore,
    setShowLoadOlderButton,
    setMessageError,
    setLoadingMessages,
    setLoadingOlder,
    setEditingMessageId,
    setEditingDraft,
    setReactionState,
    setPendingReactionByKey,
    setOpenReactionPickerMessageId,
    setSearchResults,
    setSearchError,
    setSearchOpsStatus,
    setAttachmentStatus,
    setAttachmentError,
    setVoiceStatus,
    setVoiceError,
    captureScrollMetrics: messageListController.captureScrollMetrics,
    restoreScrollAfterPrepend: messageListController.restoreScrollAfterPrepend,
    scrollMessageListToBottom: messageListController.scrollMessageListToBottom,
  });

  createEffect(() => {
    const isVoiceAudioSettingsOpen =
      activeOverlayPanel() === "settings" &&
      activeSettingsCategory() === "voice" &&
      activeVoiceSettingsSubmenu() === "audio-devices";
    if (!isVoiceAudioSettingsOpen) {
      return;
    }
    // Avoid tracking internal state reads from refreshAudioDeviceInventory in this effect,
    // otherwise toggling refresh flags can cause a self-sustaining rerun loop.
    void untrack(() => refreshAudioDeviceInventory());
  });

  createGatewayController({
    session: auth.session,
    activeGuildId,
    activeChannelId,
    canAccessActiveChannel,
    setGatewayOnline,
    setOnlineMembers,
    setMessages,
    isMessageListNearBottom: messageListController.isMessageListNearBottom,
    scrollMessageListToBottom: messageListController.scrollMessageListToBottom,
  });

  onCleanup(() => {
    void releaseRtcClient();
  });

  const createWorkspace = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    if (!session) {
      setWorkspaceError("Missing auth session.");
      return;
    }
    if (isCreatingWorkspace()) {
      return;
    }

    setWorkspaceError("");
    setCreatingWorkspace(true);
    try {
      const guild = await createGuild(session, {
        name: guildNameFromInput(createGuildName()),
        visibility: guildVisibilityFromInput(createGuildVisibility()),
      });
      const channel = await createChannel(session, guild.guildId, {
        name: channelNameFromInput(createChannelName()),
        kind: channelKindFromInput(createChannelKind()),
      });
      const createdWorkspace: WorkspaceRecord = {
        guildId: guild.guildId,
        guildName: guild.name,
        visibility: guild.visibility,
        channels: [channel],
      };
      setWorkspaces((existing) => [...existing, createdWorkspace]);
      setActiveGuildId(createdWorkspace.guildId);
      setActiveChannelId(channel.channelId);
      setCreateChannelKind("text");
      setMessageStatus("Workspace created.");
      setActiveOverlayPanel(null);
    } catch (error) {
      setWorkspaceError(mapError(error, "Unable to create workspace."));
    } finally {
      setCreatingWorkspace(false);
    }
  };

  const createNewChannel = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    const guildId = activeGuildId();
    if (!session || !guildId) {
      setChannelCreateError("Select a workspace first.");
      return;
    }
    if (isCreatingChannel()) {
      return;
    }

    setChannelCreateError("");
    setCreatingChannel(true);
    try {
      const created = await createChannel(session, guildId, {
        name: channelNameFromInput(newChannelName()),
        kind: channelKindFromInput(newChannelKind()),
      });
      setWorkspaces((existing) =>
        upsertWorkspace(existing, guildId, (workspace) => {
          if (workspace.channels.some((channel) => channel.channelId === created.channelId)) {
            return workspace;
          }
          return {
            ...workspace,
            channels: [...workspace.channels, created],
          };
        }),
      );
      setActiveChannelId(created.channelId);
      setActiveOverlayPanel(null);
      setNewChannelName("backend");
      setNewChannelKind("text");
      setMessageStatus("Channel created.");
    } catch (error) {
      setChannelCreateError(mapError(error, "Unable to create channel."));
    } finally {
      setCreatingChannel(false);
    }
  };

  createVoiceSessionLifecycleController({
    session: auth.session,
    workspaces,
    rtcSnapshot,
    isVoiceSessionActive,
    voiceSessionChannelKey,
    voiceSessionStartedAtUnixMs,
    isJoiningVoice,
    isLeavingVoice,
    leaveVoiceChannel: () => leaveVoiceChannel(),
    setVoiceDurationClockUnixMs,
    setVoiceSessionChannelKey,
    setVoiceSessionStartedAtUnixMs,
    setVoiceSessionCapabilities,
    defaultVoiceSessionCapabilities: DEFAULT_VOICE_SESSION_CAPABILITIES,
    setVoiceStatus,
    setVoiceError,
  });

  const refreshSession = async () => {
    const session = auth.session();
    if (!session || isRefreshingSession()) {
      return;
    }

    setRefreshingSession(true);
    setSessionError("");
    setSessionStatus("");
    try {
      const next = await refreshAuthSession(session.refreshToken);
      auth.setAuthenticatedSession(next);
      setSessionStatus("Session refreshed.");
    } catch (error) {
      setSessionError(mapError(error, "Unable to refresh session."));
    } finally {
      setRefreshingSession(false);
    }
  };

  const logout = async () => {
    await leaveVoiceChannel();
    await releaseRtcClient();
    const session = auth.session();
    if (session) {
      try {
        await logoutAuthSession(session.refreshToken);
      } catch {
        // best-effort logout; local session will still be cleared
      }
    }
    auth.clearAuthenticatedSession();
    clearWorkspaceCache();
  };

  const runHealthCheck = async () => {
    if (isCheckingHealth()) {
      return;
    }
    setCheckingHealth(true);
    setDiagError("");
    try {
      const health = await fetchHealth();
      setHealthStatus(`Health: ${health.status}`);
    } catch (error) {
      setDiagError(mapError(error, "Health check failed."));
    } finally {
      setCheckingHealth(false);
    }
  };

  const runEcho = async (event: SubmitEvent) => {
    event.preventDefault();
    if (isEchoing()) {
      return;
    }

    setEchoing(true);
    setDiagError("");
    try {
      const echoed = await echoMessage({ message: echoInput() });
      setHealthStatus(`Echo: ${echoed.slice(0, 60)}`);
    } catch (error) {
      setDiagError(mapError(error, "Echo request failed."));
    } finally {
      setEchoing(false);
    }
  };

  const panelHostPropGroups = () =>
    buildPanelHostPropGroups({
      createGuildName: createGuildName(),
      createGuildVisibility: createGuildVisibility(),
      createChannelName: createChannelName(),
      createChannelKind: createChannelKind(),
      isCreatingWorkspace: isCreatingWorkspace(),
      canDismissWorkspaceCreateForm: canDismissWorkspaceCreateForm(),
      workspaceError: workspaceError(),
      onCreateWorkspaceSubmit: createWorkspace,
      setCreateGuildName,
      setCreateGuildVisibility,
      setCreateChannelName,
      setCreateChannelKind,
      onCancelWorkspaceCreate: closeOverlayPanel,
      newChannelName: newChannelName(),
      newChannelKind: newChannelKind(),
      isCreatingChannel: isCreatingChannel(),
      channelCreateError: channelCreateError(),
      onCreateChannelSubmit: createNewChannel,
      setNewChannelName,
      setNewChannelKind,
      onCancelChannelCreate: closeOverlayPanel,
      publicGuildSearchQuery: publicGuildSearchQuery(),
      isSearchingPublicGuilds: isSearchingPublicGuilds(),
      publicGuildSearchError: publicGuildSearchError(),
      publicGuildDirectory: publicGuildDirectory(),
      onSubmitPublicGuildSearch: runPublicGuildSearch,
      setPublicGuildSearchQuery,
      activeSettingsCategory: activeSettingsCategory(),
      activeVoiceSettingsSubmenu: activeVoiceSettingsSubmenu(),
      voiceDevicePreferences: voiceDevicePreferences(),
      audioInputDevices: audioInputDevices(),
      audioOutputDevices: audioOutputDevices(),
      isRefreshingAudioDevices: isRefreshingAudioDevices(),
      audioDevicesStatus: audioDevicesStatus(),
      audioDevicesError: audioDevicesError(),
      profile: profile() ?? null,
      profileDraftUsername: profileDraftUsername(),
      profileDraftAbout: profileDraftAbout(),
      profileAvatarUrl: profile() ? avatarUrlForUser(profile()!.userId) : null,
      selectedAvatarFilename: selectedProfileAvatarFile()?.name ?? "",
      isSavingProfile: isSavingProfile(),
      isUploadingProfileAvatar: isUploadingProfileAvatar(),
      profileSettingsStatus: profileSettingsStatus(),
      profileSettingsError: profileSettingsError(),
      onOpenSettingsCategory: openSettingsCategory,
      onOpenVoiceSettingsSubmenu: setActiveVoiceSettingsSubmenu,
      onSetVoiceDevicePreference: (kind, value) =>
        setVoiceDevicePreference(kind, value),
      onRefreshAudioDeviceInventory: refreshAudioDeviceInventory,
      setProfileDraftUsername,
      setProfileDraftAbout,
      setSelectedProfileAvatarFile,
      onSaveProfileSettings: saveProfileSettings,
      onUploadProfileAvatar: uploadProfileAvatar,
      friendRecipientUserIdInput: friendRecipientUserIdInput(),
      friendRequests: friendRequests(),
      friends: friends(),
      isRunningFriendAction: isRunningFriendAction(),
      friendStatus: friendStatus(),
      friendError: friendError(),
      onSubmitFriendRequest: submitFriendRequest,
      setFriendRecipientUserIdInput,
      onAcceptIncomingFriendRequest: (requestId) =>
        acceptIncomingFriendRequest(requestId),
      onDismissFriendRequest: (requestId) => dismissFriendRequest(requestId),
      onRemoveFriendship: (friendUserId) => removeFriendship(friendUserId),
      searchQuery: searchQuery(),
      isSearching: isSearching(),
      hasActiveWorkspace: Boolean(activeWorkspace()),
      canManageSearchMaintenance: canManageSearchMaintenance(),
      isRunningSearchOps: isRunningSearchOps(),
      searchOpsStatus: searchOpsStatus(),
      searchError: searchError(),
      searchResults: searchResults(),
      onSubmitSearch: runSearch,
      setSearchQuery,
      onRebuildSearch: rebuildSearch,
      onReconcileSearch: reconcileSearch,
      displayUserLabel,
      attachmentFilename: attachmentFilename(),
      activeAttachments: activeAttachments(),
      isUploadingAttachment: isUploadingAttachment(),
      hasActiveChannel: Boolean(activeChannel()),
      attachmentStatus: attachmentStatus(),
      attachmentError: attachmentError(),
      downloadingAttachmentId: downloadingAttachmentId(),
      deletingAttachmentId: deletingAttachmentId(),
      onSubmitUploadAttachment: uploadAttachment,
      setSelectedAttachment,
      setAttachmentFilename,
      onDownloadAttachment: (record) => downloadAttachment(record),
      onRemoveAttachment: (record) => removeAttachment(record),
      moderationUserIdInput: moderationUserIdInput(),
      moderationRoleInput: moderationRoleInput(),
      overrideRoleInput: overrideRoleInput(),
      overrideAllowCsv: overrideAllowCsv(),
      overrideDenyCsv: overrideDenyCsv(),
      isModerating: isModerating(),
      canManageRoles: canManageRoles(),
      canBanMembers: canBanMembers(),
      canManageChannelOverrides: canManageChannelOverrides(),
      moderationStatus: moderationStatus(),
      moderationError: moderationError(),
      setModerationUserIdInput,
      setModerationRoleInput,
      onRunMemberAction: (action) => runMemberAction(action),
      setOverrideRoleInput,
      setOverrideAllowCsv,
      setOverrideDenyCsv,
      onApplyOverride: applyOverride,
      echoInput: echoInput(),
      healthStatus: healthStatus(),
      diagError: diagError(),
      isCheckingHealth: isCheckingHealth(),
      isEchoing: isEchoing(),
      setEchoInput,
      onRunHealthCheck: runHealthCheck,
      onRunEcho: runEcho,
    });

  return (
    <AppShellLayout
      isChannelRailCollapsed={isChannelRailCollapsed()}
      isMemberRailCollapsed={isMemberRailCollapsed()}
      serverRail={
        <ServerRail
          workspaces={workspaces()}
          activeGuildId={activeGuildId()}
          isCreatingWorkspace={isCreatingWorkspace()}
          onSelectWorkspace={(guildId, firstChannelId) => {
            setActiveGuildId(guildId);
            setActiveChannelId(firstChannelId);
          }}
          onOpenPanel={openOverlayPanel}
        />
      }
      channelRail={
        <ChannelRail
          activeWorkspace={activeWorkspace()}
          activeChannel={activeChannel()}
          activeChannelId={activeChannelId()}
          activeTextChannels={activeTextChannels()}
          activeVoiceChannels={activeVoiceChannels()}
          canManageWorkspaceChannels={canManageWorkspaceChannels()}
          canShowVoiceHeaderControls={canShowVoiceHeaderControls()}
          isVoiceSessionActive={isVoiceSessionActive()}
          isVoiceSessionForChannel={isVoiceSessionForChannel}
          voiceSessionDurationLabel={voiceSessionDurationLabel()}
          voiceRosterEntries={voiceRosterEntries()}
          voiceStreamPermissionHints={voiceStreamPermissionHints()}
          activeVoiceSessionLabel={activeVoiceSessionLabel()}
          rtcSnapshot={rtcSnapshot()}
          canToggleVoiceCamera={canToggleVoiceCamera()}
          canToggleVoiceScreenShare={canToggleVoiceScreenShare()}
          isJoiningVoice={isJoiningVoice()}
          isLeavingVoice={isLeavingVoice()}
          isTogglingVoiceMic={isTogglingVoiceMic()}
          isTogglingVoiceCamera={isTogglingVoiceCamera()}
          isTogglingVoiceScreenShare={isTogglingVoiceScreenShare()}
          currentUserId={profile()?.userId ?? null}
          currentUserLabel={profile()?.username}
          currentUserStatusLabel={gatewayOnline() ? "Online" : "Offline"}
          resolveAvatarUrl={avatarUrlForUser}
          userIdFromVoiceIdentity={userIdFromVoiceIdentity}
          actorLabel={actorLabel}
          voiceParticipantLabel={voiceParticipantLabel}
          onOpenUserProfile={openUserProfile}
          onOpenSettings={() => openOverlayPanel("settings")}
          onCreateTextChannel={() => {
            setNewChannelKind(channelKindFromInput("text"));
            openOverlayPanel("channel-create");
          }}
          onCreateVoiceChannel={() => {
            setNewChannelKind(channelKindFromInput("voice"));
            openOverlayPanel("channel-create");
          }}
          onSelectChannel={(channelId) => setActiveChannelId(channelId)}
          onJoinVoice={() => void joinVoiceChannel()}
          onToggleVoiceMicrophone={() => void toggleVoiceMicrophone()}
          onToggleVoiceCamera={() => void toggleVoiceCamera()}
          onToggleVoiceScreenShare={() => void toggleVoiceScreenShare()}
          onLeaveVoice={() => void leaveVoiceChannel("Voice session ended.")}
        />
      }
      chatColumn={
        <ChatColumn
          chatHeader={
            <ChatHeader
              activeChannel={activeChannel()}
              gatewayOnline={gatewayOnline()}
              canShowVoiceHeaderControls={canShowVoiceHeaderControls()}
              isVoiceSessionActive={isVoiceSessionActive()}
              voiceConnectionState={voiceConnectionState()}
              isChannelRailCollapsed={isChannelRailCollapsed()}
              isMemberRailCollapsed={isMemberRailCollapsed()}
              isRefreshingSession={isRefreshingSession()}
              onToggleChannelRail={() => setChannelRailCollapsed((value) => !value)}
              onToggleMemberRail={() => setMemberRailCollapsed((value) => !value)}
              onOpenPanel={openOverlayPanel}
              onRefreshMessages={() => void refreshMessages()}
              onRefreshSession={() => void refreshSession()}
              onLogout={() => void logout()}
            />
          }
          workspaceBootstrapDone={workspaceBootstrapDone()}
          workspaceCount={workspaces().length}
          isLoadingMessages={isLoadingMessages()}
          messageError={messageError()}
          sessionStatus={sessionStatus()}
          sessionError={sessionError()}
          voiceStatus={voiceStatus()}
          voiceError={voiceError()}
          canShowVoiceHeaderControls={canShowVoiceHeaderControls()}
          isVoiceSessionActive={isVoiceSessionActive()}
          activeChannel={activeChannel()}
          canAccessActiveChannel={canAccessActiveChannel()}
          messageList={
            <MessageList
              onListRef={messageListController.onListRef}
              onListScroll={() => {
                messageListController.onMessageListScroll(loadOlderMessages);
              }}
              nextBefore={nextBefore()}
              showLoadOlderButton={showLoadOlderButton()}
              isLoadingOlder={isLoadingOlder()}
              isLoadingMessages={isLoadingMessages()}
              messageError={messageError()}
              messages={messages()}
              onLoadOlderMessages={() => loadOlderMessages()}
              currentUserId={profile()?.userId ?? null}
              canDeleteMessages={canDeleteMessages()}
              displayUserLabel={displayUserLabel}
              resolveAvatarUrl={avatarUrlForUser}
              onOpenAuthorProfile={openUserProfile}
              editingMessageId={editingMessageId()}
              editingDraft={editingDraft()}
              isSavingEdit={isSavingEdit()}
              deletingMessageId={deletingMessageId()}
              openReactionPickerMessageId={openReactionPickerMessageId()}
              reactionState={reactionState()}
              pendingReactionByKey={pendingReactionByKey()}
              messageMediaByAttachmentId={messageMediaByAttachmentId()}
              loadingMediaPreviewIds={loadingMediaPreviewIds()}
              failedMediaPreviewIds={failedMediaPreviewIds()}
              downloadingAttachmentId={downloadingAttachmentId()}
              addReactionIconUrl={ADD_REACTION_ICON_URL}
              editMessageIconUrl={EDIT_MESSAGE_ICON_URL}
              deleteMessageIconUrl={DELETE_MESSAGE_ICON_URL}
              onEditingDraftInput={setEditingDraft}
              onSaveEditMessage={(messageId) => saveEditMessage(messageId)}
              onCancelEditMessage={cancelEditMessage}
              onDownloadAttachment={(record) => downloadAttachment(record)}
              onRetryMediaPreview={retryMediaPreview}
              onToggleMessageReaction={(messageId, emoji) =>
                toggleMessageReaction(messageId, emoji)}
              onToggleReactionPicker={toggleReactionPicker}
              onBeginEditMessage={beginEditMessage}
              onRemoveMessage={(messageId) => removeMessage(messageId)}
            />
          }
          messageComposer={
            <MessageComposer
              attachmentInputRef={(value) => {
                composerAttachmentInputRef = value;
              }}
              activeChannel={activeChannel()}
              canAccessActiveChannel={canAccessActiveChannel()}
              isSendingMessage={isSendingMessage()}
              composerValue={composer()}
              composerAttachments={composerAttachments()}
              onSubmit={sendMessage}
              onComposerInput={setComposer}
              onOpenAttachmentPicker={openComposerAttachmentPicker}
              onAttachmentInput={(event) =>
                onComposerAttachmentInput(
                  event as InputEvent & { currentTarget: HTMLInputElement },
                )
              }
              onRemoveAttachment={removeComposerAttachment}
            />
          }
          reactionPicker={
            <ReactionPickerPortal
              openMessageId={openReactionPickerMessageId()}
              position={reactionPickerOverlayPosition()}
              options={OPENMOJI_REACTION_OPTIONS}
              onClose={() => setOpenReactionPickerMessageId(null)}
              onAddReaction={(messageId, emoji) =>
                addReactionFromPicker(messageId, emoji)}
            />
          }
          messageStatus={messageStatus()}
        />
      }
      memberRail={
        <MemberRail
          profileLoading={profile.loading}
          profileErrorText={profile.error ? profileErrorMessage(profile.error) : ""}
          profile={profile() ?? null}
          showUnauthorizedWorkspaceNote={Boolean(
            activeWorkspace() && activeChannel() && !canAccessActiveChannel(),
          )}
          canAccessActiveChannel={canAccessActiveChannel()}
          onlineMembers={onlineMembers()}
          hasModerationAccess={hasModerationAccess()}
          displayUserLabel={displayUserLabel}
          onOpenPanel={openOverlayPanel}
        />
      }
    >
      <PanelHost
        panel={activeOverlayPanel()}
        canCloseActivePanel={canCloseActivePanel()}
        canManageWorkspaceChannels={canManageWorkspaceChannels()}
        canAccessActiveChannel={canAccessActiveChannel()}
        hasModerationAccess={hasModerationAccess()}
        panelTitle={overlayPanelTitle}
        panelClassName={overlayPanelClassName}
        onClose={closeOverlayPanel}
        {...panelHostPropGroups()}
      />
      <UserProfileOverlay
        selectedProfileUserId={selectedProfileUserId()}
        selectedProfileLoading={selectedProfile.loading}
        selectedProfileError={selectedProfileError()}
        selectedProfile={selectedProfile() ?? null}
        avatarUrlForUser={avatarUrlForUser}
        onClose={() => setSelectedProfileUserId(null)}
      />
    </AppShellLayout>
  );
}
