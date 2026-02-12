import {
  Show,
  createEffect,
  createResource,
  onCleanup,
  untrack,
} from "solid-js";
import {
  channelKindFromInput,
  channelNameFromInput,
  guildVisibilityFromInput,
  guildNameFromInput,
  profileAboutFromInput,
  roleFromInput,
  userIdFromInput,
  type AttachmentId,
  type AttachmentRecord,
  type ChannelKindName,
  type FriendRecord,
  type FriendRequestList,
  type GuildVisibility,
  type GuildId,
  type MessageRecord,
  type GuildRecord,
  type MediaPublishSource,
  type RoleName,
  type SearchResults,
  type UserId,
  type WorkspaceRecord,
} from "../domain/chat";
import {
  acceptFriendRequest,
  createChannel,
  createFriendRequest,
  createGuild,
  deleteFriendRequest,
  echoMessage,
  fetchChannelMessages,
  fetchFriendRequests,
  fetchFriends,
  fetchHealth,
  fetchMe,
  fetchPublicGuildDirectory,
  fetchUserProfile,
  issueVoiceToken,
  logoutAuthSession,
  profileAvatarUrl,
  refreshAuthSession,
  removeFriend,
  updateMyProfile,
  uploadMyProfileAvatar,
} from "../lib/api";
import { useAuth } from "../lib/auth-context";
import { usernameFromInput } from "../domain/auth";
import {
  channelHeaderLabel,
  channelKey,
  mapError,
  mapRtcError,
  mapVoiceJoinError,
  mergeMessage,
  mergeMessageHistory,
  normalizeMessageOrder,
  profileErrorMessage,
  shortActor,
  upsertWorkspace,
  userIdFromVoiceIdentity,
  type ReactionView,
} from "../features/app-shell/helpers";
import { ChannelRail } from "../features/app-shell/components/ChannelRail";
import { ChatHeader } from "../features/app-shell/components/ChatHeader";
import { MemberRail } from "../features/app-shell/components/MemberRail";
import { MessageComposer } from "../features/app-shell/components/messages/MessageComposer";
import { MessageList } from "../features/app-shell/components/messages/MessageList";
import { ReactionPickerPortal } from "../features/app-shell/components/messages/ReactionPickerPortal";
import { PanelHost } from "../features/app-shell/components/panels/PanelHost";
import { ServerRail } from "../features/app-shell/components/ServerRail";
import { SafeMarkdown } from "../features/app-shell/components/SafeMarkdown";
import { OPENMOJI_REACTION_OPTIONS } from "../features/app-shell/config/reaction-options";
import {
  SETTINGS_CATEGORIES,
  VOICE_SETTINGS_SUBMENU,
} from "../features/app-shell/config/settings-menu";
import {
  ADD_REACTION_ICON_URL,
  DELETE_MESSAGE_ICON_URL,
  EDIT_MESSAGE_ICON_URL,
  RTC_DISCONNECTED_SNAPSHOT,
} from "../features/app-shell/config/ui-constants";
import { createAttachmentController } from "../features/app-shell/controllers/attachment-controller";
import { createMessageListController } from "../features/app-shell/controllers/message-list-controller";
import { createModerationController } from "../features/app-shell/controllers/moderation-controller";
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
import { createProfileOverlayController } from "../features/app-shell/controllers/profile-overlay-controller";
import { createReactionPickerController } from "../features/app-shell/controllers/reaction-picker-controller";
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
import { connectGateway } from "../lib/gateway";
import { createRtcClient, type RtcClient } from "../lib/rtc";
import {
  enumerateAudioDevices,
  reconcileVoiceDevicePreferences,
  saveVoiceDevicePreferences,
  type MediaDeviceId,
  type VoiceDevicePreferences,
} from "../lib/voice-device-settings";
import { clearWorkspaceCache, saveWorkspaceCache } from "../lib/workspace-cache";
import {
  clearUsernameLookupCache,
  primeUsernameCache,
  resolveUsernames,
} from "../lib/username-cache";

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
  let rtcClient: RtcClient | null = null;
  let stopRtcSubscription: (() => void) | null = null;

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

    if (!rtcClient || !isVoiceSessionActive()) {
      setAudioDevicesStatus(
        resolveVoiceDevicePreferenceStatus(kind, false, nextDeviceId),
      );
      return;
    }

    try {
      if (kind === "audioinput") {
        await rtcClient.setAudioInputDevice(next.audioInputDeviceId);
      } else {
        await rtcClient.setAudioOutputDevice(next.audioOutputDeviceId);
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

  const ensureRtcClient = (): RtcClient => {
    if (rtcClient) {
      return rtcClient;
    }
    rtcClient = createRtcClient();
    stopRtcSubscription = rtcClient.subscribe((snapshot) => {
      setRtcSnapshot(snapshot);
    });
    return rtcClient;
  };

  const releaseRtcClient = async (): Promise<void> => {
    if (stopRtcSubscription) {
      stopRtcSubscription();
      stopRtcSubscription = null;
    }
    if (rtcClient) {
      try {
        await rtcClient.destroy();
      } catch {
        // Deterministic local teardown even if remote transport cleanup fails.
      } finally {
        rtcClient = null;
      }
    }
    setRtcSnapshot(RTC_DISCONNECTED_SNAPSHOT);
    setVoiceSessionChannelKey(null);
    setVoiceSessionStartedAtUnixMs(null);
    setVoiceSessionCapabilities(DEFAULT_VOICE_SESSION_CAPABILITIES);
  };

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

  const loadPublicGuildDirectory = async (query?: string) => {
    const session = auth.session();
    if (!session) {
      setPublicGuildDirectory([]);
      return;
    }
    if (isSearchingPublicGuilds()) {
      return;
    }
    setSearchingPublicGuilds(true);
    setPublicGuildSearchError("");
    try {
      const directory = await fetchPublicGuildDirectory(session, {
        query,
        limit: 20,
      });
      setPublicGuildDirectory(directory.guilds);
    } catch (error) {
      setPublicGuildSearchError(mapError(error, "Unable to load public workspace directory."));
      setPublicGuildDirectory([]);
    } finally {
      setSearchingPublicGuilds(false);
    }
  };

  const refreshFriendDirectory = async () => {
    const session = auth.session();
    if (!session) {
      setFriends([]);
      setFriendRequests({ incoming: [], outgoing: [] });
      return;
    }
    setFriendError("");
    try {
      const [friendList, requestList] = await Promise.all([
        fetchFriends(session),
        fetchFriendRequests(session),
      ]);
      setFriends(friendList);
      setFriendRequests(requestList);
    } catch (error) {
      setFriendError(mapError(error, "Unable to load friendship state."));
    }
  };

  const avatarUrlForUser = (userId: string): string | null => {
    try {
      const parsedUserId = userIdFromInput(userId);
      const avatarVersion = avatarVersionByUserId()[userId] ?? 0;
      return profileAvatarUrl(parsedUserId, avatarVersion);
    } catch {
      return null;
    }
  };

  const openUserProfile = (rawUserId: string) => {
    try {
      const userId = userIdFromInput(rawUserId);
      setSelectedProfileError("");
      setSelectedProfileUserId(userId);
    } catch {
      setSelectedProfileError("User profile is unavailable.");
    }
  };

  const saveProfileSettings = async () => {
    const session = auth.session();
    const currentProfile = profile();
    if (!session || !currentProfile || isSavingProfile()) {
      return;
    }

    setSavingProfile(true);
    setProfileSettingsStatus("");
    setProfileSettingsError("");
    try {
      const nextUsername = usernameFromInput(profileDraftUsername().trim());
      const nextAbout = profileAboutFromInput(profileDraftAbout());
      const updated = await updateMyProfile(session, {
        username: nextUsername,
        aboutMarkdown: nextAbout,
      });
      mutateProfile(updated);
      setProfileSettingsStatus("Profile updated.");
    } catch (error) {
      setProfileSettingsError(mapError(error, "Unable to save profile settings."));
    } finally {
      setSavingProfile(false);
    }
  };

  const uploadProfileAvatar = async () => {
    const session = auth.session();
    const selectedFile = selectedProfileAvatarFile();
    if (!session || !selectedFile || isUploadingProfileAvatar()) {
      return;
    }

    setUploadingProfileAvatar(true);
    setProfileSettingsStatus("");
    setProfileSettingsError("");
    try {
      const updated = await uploadMyProfileAvatar(session, selectedFile);
      mutateProfile(updated);
      setSelectedProfileAvatarFile(null);
      setProfileSettingsStatus("Profile avatar updated.");
    } catch (error) {
      setProfileSettingsError(mapError(error, "Unable to upload profile avatar."));
    } finally {
      setUploadingProfileAvatar(false);
    }
  };

  const [profile, { mutate: mutateProfile }] = createResource(async () => {
    const session = auth.session();
    if (!session) {
      throw new Error("missing_session");
    }
    return fetchMe(session);
  });
  const [selectedProfile] = createResource(
    () => selectedProfileUserId() ?? undefined,
    async (userId) => {
      const session = auth.session();
      if (!session) {
        return null;
      }
      try {
        return await fetchUserProfile(session, userId);
      } catch (error) {
        setSelectedProfileError(mapError(error, "Profile unavailable."));
        return null;
      }
    },
  );

  createEffect(() => {
    const session = auth.session();
    if (!session) {
      clearUsernameLookupCache();
      setResolvedUsernames({});
      setAvatarVersionByUserId({});
      setPublicGuildDirectory([]);
      setPublicGuildSearchError("");
      setProfileDraftUsername("");
      setProfileDraftAbout("");
      setSelectedProfileAvatarFile(null);
      setProfileSettingsStatus("");
      setProfileSettingsError("");
      setSelectedProfileUserId(null);
      setSelectedProfileError("");
      return;
    }
    void untrack(() => loadPublicGuildDirectory());
  });

  createEffect(() => {
    const session = auth.session();
    if (!session) {
      clearUsernameLookupCache();
      setResolvedUsernames({});
      setAvatarVersionByUserId({});
      setFriends([]);
      setFriendRequests({ incoming: [], outgoing: [] });
      setFriendStatus("");
      setFriendError("");
      return;
    }
    void untrack(() => refreshFriendDirectory());
  });

  createEffect(() => {
    const value = profile();
    if (!value) {
      return;
    }
    setProfileDraftUsername(value.username);
    setProfileDraftAbout(value.aboutMarkdown);
    primeUsernameCache([{ userId: value.userId, username: value.username }]);
    setResolvedUsernames((existing) => ({
      ...existing,
      [value.userId]: value.username,
    }));
    setAvatarVersionByUserId((existing) => ({
      ...existing,
      [value.userId]: value.avatarVersion,
    }));
  });

  createEffect(() => {
    const value = selectedProfile();
    if (!value) {
      return;
    }
    setSelectedProfileError("");
    setAvatarVersionByUserId((existing) => ({
      ...existing,
      [value.userId]: value.avatarVersion,
    }));
  });

  createEffect(() => {
    const known = [
      ...friends().map((friend) => ({
        userId: friend.userId,
        username: friend.username,
      })),
      ...friendRequests().incoming.map((request) => ({
        userId: request.senderUserId,
        username: request.senderUsername,
      })),
      ...friendRequests().outgoing.map((request) => ({
        userId: request.recipientUserId,
        username: request.recipientUsername,
      })),
    ];
    if (known.length === 0) {
      return;
    }
    primeUsernameCache(known);
    setResolvedUsernames((existing) => ({
      ...existing,
      ...Object.fromEntries(known.map((entry) => [entry.userId, entry.username])),
    }));
  });

  createEffect(() => {
    const session = auth.session();
    if (!session) {
      return;
    }

    const lookupIds = new Set<UserId>();
    for (const message of messages()) {
      lookupIds.add(message.authorId);
    }
    for (const memberId of onlineMembers()) {
      try {
        lookupIds.add(userIdFromInput(memberId));
      } catch {
        continue;
      }
    }
    for (const participant of voiceRosterEntries()) {
      const participantUserId = userIdFromVoiceIdentity(participant.identity);
      if (participantUserId) {
        lookupIds.add(participantUserId);
      }
    }
    const result = searchResults();
    if (result) {
      for (const message of result.messages) {
        lookupIds.add(message.authorId);
      }
    }
    if (lookupIds.size === 0) {
      return;
    }

    let cancelled = false;
    const resolveVisibleUsernames = async () => {
      try {
        const resolved = await resolveUsernames(session, [...lookupIds]);
        if (cancelled || Object.keys(resolved).length === 0) {
          return;
        }
        setResolvedUsernames((existing) => ({
          ...existing,
          ...resolved,
        }));
      } catch {
        // Keep user-id fallback rendering if lookup fails.
      }
    };
    void resolveVisibleUsernames();

    onCleanup(() => {
      cancelled = true;
    });
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

  const refreshMessages = async () => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId) {
      setMessages([]);
      setNextBefore(null);
      setShowLoadOlderButton(false);
      return;
    }

    setMessageError("");
    setLoadingMessages(true);
    try {
      const history = await fetchChannelMessages(session, guildId, channelId, { limit: 50 });
      setMessages(normalizeMessageOrder(history.messages));
      setNextBefore(history.nextBefore);
      setEditingMessageId(null);
      setEditingDraft("");
      messageListController.scrollMessageListToBottom();
    } catch (error) {
      setMessageError(mapError(error, "Unable to load messages."));
      setMessages([]);
      setNextBefore(null);
      setShowLoadOlderButton(false);
    } finally {
      setLoadingMessages(false);
    }
  };

  const loadOlderMessages = async () => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    const before = nextBefore();
    if (!session || !guildId || !channelId || !before || isLoadingOlder()) {
      return;
    }

    const previousScrollMetrics = messageListController.captureScrollMetrics();
    setLoadingOlder(true);
    setMessageError("");
    try {
      const history = await fetchChannelMessages(session, guildId, channelId, {
        limit: 50,
        before,
      });
      setMessages((existing) => mergeMessageHistory(existing, history.messages));
      setNextBefore(history.nextBefore);
      messageListController.restoreScrollAfterPrepend(previousScrollMetrics);
    } catch (error) {
      setMessageError(mapError(error, "Unable to load older messages."));
    } finally {
      setLoadingOlder(false);
    }
  };

  createEffect(() => {
    void activeGuildId();
    void activeChannelId();
    const canRead = canAccessActiveChannel();
    setReactionState({});
    setPendingReactionByKey({});
    setOpenReactionPickerMessageId(null);
    setSearchResults(null);
    setSearchError("");
    setSearchOpsStatus("");
    setAttachmentStatus("");
    setAttachmentError("");
    setVoiceStatus("");
    setVoiceError("");
    if (canRead) {
      void refreshMessages();
    } else {
      setMessages([]);
      setNextBefore(null);
      setShowLoadOlderButton(false);
    }
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

  createEffect(() => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channelId = activeChannelId();
    if (!session || !guildId || !channelId || !canAccessActiveChannel()) {
      setGatewayOnline(false);
      setOnlineMembers([]);
      return;
    }

    const gateway = connectGateway(session.accessToken, guildId, channelId, {
      onOpenStateChange: (isOpen) => setGatewayOnline(isOpen),
      onMessageCreate: (message) => {
        if (message.guildId !== guildId || message.channelId !== channelId) {
          return;
        }
        const shouldStickToBottom = messageListController.isMessageListNearBottom();
        setMessages((existing) => mergeMessage(existing, message));
        if (shouldStickToBottom) {
          messageListController.scrollMessageListToBottom();
        }
      },
      onPresenceSync: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        setOnlineMembers(payload.userIds);
      },
      onPresenceUpdate: (payload) => {
        if (payload.guildId !== guildId) {
          return;
        }
        setOnlineMembers((existing) => {
          if (payload.status === "online") {
            return existing.includes(payload.userId) ? existing : [...existing, payload.userId];
          }
          return existing.filter((entry) => entry !== payload.userId);
        });
      },
    });

    onCleanup(() => gateway.close());
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

  const runPublicGuildSearch = async (event: SubmitEvent) => {
    event.preventDefault();
    await loadPublicGuildDirectory(publicGuildSearchQuery());
  };

  const submitFriendRequest = async (event: SubmitEvent) => {
    event.preventDefault();
    const session = auth.session();
    if (!session || isRunningFriendAction()) {
      return;
    }
    setRunningFriendAction(true);
    setFriendError("");
    setFriendStatus("");
    try {
      const recipientUserId = userIdFromInput(friendRecipientUserIdInput().trim());
      await createFriendRequest(session, recipientUserId);
      setFriendRecipientUserIdInput("");
      await refreshFriendDirectory();
      setFriendStatus("Friend request sent.");
    } catch (error) {
      setFriendError(mapError(error, "Unable to create friend request."));
    } finally {
      setRunningFriendAction(false);
    }
  };

  const acceptIncomingFriendRequest = async (requestId: string) => {
    const session = auth.session();
    if (!session || isRunningFriendAction()) {
      return;
    }
    setRunningFriendAction(true);
    setFriendError("");
    setFriendStatus("");
    try {
      await acceptFriendRequest(session, requestId);
      await refreshFriendDirectory();
      setFriendStatus("Friend request accepted.");
    } catch (error) {
      setFriendError(mapError(error, "Unable to accept friend request."));
    } finally {
      setRunningFriendAction(false);
    }
  };

  const dismissFriendRequest = async (requestId: string) => {
    const session = auth.session();
    if (!session || isRunningFriendAction()) {
      return;
    }
    setRunningFriendAction(true);
    setFriendError("");
    setFriendStatus("");
    try {
      await deleteFriendRequest(session, requestId);
      await refreshFriendDirectory();
      setFriendStatus("Friend request removed.");
    } catch (error) {
      setFriendError(mapError(error, "Unable to remove friend request."));
    } finally {
      setRunningFriendAction(false);
    }
  };

  const removeFriendship = async (friendUserId: UserId) => {
    const session = auth.session();
    if (!session || isRunningFriendAction()) {
      return;
    }
    setRunningFriendAction(true);
    setFriendError("");
    setFriendStatus("");
    try {
      await removeFriend(session, friendUserId);
      await refreshFriendDirectory();
      setFriendStatus("Friend removed.");
    } catch (error) {
      setFriendError(mapError(error, "Unable to remove friend."));
    } finally {
      setRunningFriendAction(false);
    }
  };

  const leaveVoiceChannel = async (statusMessage?: string) => {
    if (isLeavingVoice()) {
      return;
    }
    setLeavingVoice(true);
    try {
      if (rtcClient) {
        await rtcClient.leave();
      }
    } catch {
      // Leave should still clear local voice state even if transport teardown fails.
    } finally {
      setVoiceSessionChannelKey(null);
      setVoiceSessionStartedAtUnixMs(null);
      setRtcSnapshot(RTC_DISCONNECTED_SNAPSHOT);
      setVoiceSessionCapabilities(DEFAULT_VOICE_SESSION_CAPABILITIES);
      if (statusMessage) {
        setVoiceStatus(statusMessage);
      }
      setLeavingVoice(false);
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

  const joinVoiceChannel = async () => {
    const session = auth.session();
    const guildId = activeGuildId();
    const channel = activeChannel();
    if (
      !session ||
      !guildId ||
      !channel ||
      channel.kind !== "voice" ||
      isJoiningVoice() ||
      isLeavingVoice()
    ) {
      return;
    }

    setJoiningVoice(true);
    setVoiceError("");
    setVoiceStatus("");
    setVoiceSessionCapabilities(DEFAULT_VOICE_SESSION_CAPABILITIES);
    try {
      const requestedPublishSources: MediaPublishSource[] = ["microphone"];
      if (canPublishVoiceCamera()) {
        requestedPublishSources.push("camera");
      }
      if (canPublishVoiceScreenShare()) {
        requestedPublishSources.push("screen_share");
      }
      const token = await issueVoiceToken(session, guildId, channel.channelId, {
        canSubscribe: canSubscribeVoiceStreams(),
        publishSources: requestedPublishSources,
      });
      const client = ensureRtcClient();
      const preferences = voiceDevicePreferences();
      await client.setAudioInputDevice(preferences.audioInputDeviceId);
      await client.setAudioOutputDevice(preferences.audioOutputDeviceId);
      await client.join({
        livekitUrl: token.livekitUrl,
        token: token.token,
      });
      setVoiceSessionChannelKey(channelKey(guildId, channel.channelId));
      setVoiceSessionStartedAtUnixMs(Date.now());
      setVoiceDurationClockUnixMs(Date.now());
      setVoiceSessionCapabilities({
        canSubscribe: token.canSubscribe,
        publishSources: [...token.publishSources],
      });
      const joinSnapshot = client.snapshot();
      if (joinSnapshot.lastErrorCode === "audio_device_switch_failed" && joinSnapshot.lastErrorMessage) {
        setAudioDevicesError(joinSnapshot.lastErrorMessage);
      }

      if (token.canPublish && token.publishSources.includes("microphone")) {
        try {
          await client.setMicrophoneEnabled(true);
          setVoiceStatus("Voice connected. Microphone enabled.");
        } catch (error) {
          setVoiceStatus("Voice connected.");
          setVoiceError(mapRtcError(error, "Connected, but microphone activation failed."));
        }
        return;
      }

      setVoiceStatus("Voice connected in listen-only mode.");
    } catch (error) {
      setVoiceError(mapVoiceJoinError(error));
    } finally {
      setJoiningVoice(false);
    }
  };

  const toggleVoiceMicrophone = async () => {
    if (!rtcClient || isTogglingVoiceMic()) {
      return;
    }
    setTogglingVoiceMic(true);
    setVoiceError("");
    try {
      const enabled = await rtcClient.toggleMicrophone();
      setVoiceStatus(enabled ? "Microphone unmuted." : "Microphone muted.");
    } catch (error) {
      setVoiceError(mapRtcError(error, "Unable to update microphone."));
    } finally {
      setTogglingVoiceMic(false);
    }
  };

  const toggleVoiceCamera = async () => {
    if (!rtcClient || isTogglingVoiceCamera()) {
      return;
    }
    if (!canToggleVoiceCamera()) {
      setVoiceError("Camera publish is not allowed for this call.");
      return;
    }
    setTogglingVoiceCamera(true);
    setVoiceError("");
    try {
      const enabled = await rtcClient.toggleCamera();
      setVoiceStatus(enabled ? "Camera enabled." : "Camera disabled.");
    } catch (error) {
      setVoiceError(mapRtcError(error, "Unable to update camera."));
    } finally {
      setTogglingVoiceCamera(false);
    }
  };

  const toggleVoiceScreenShare = async () => {
    if (!rtcClient || isTogglingVoiceScreenShare()) {
      return;
    }
    if (!canToggleVoiceScreenShare()) {
      setVoiceError("Screen share publish is not allowed for this call.");
      return;
    }
    setTogglingVoiceScreenShare(true);
    setVoiceError("");
    try {
      const enabled = await rtcClient.toggleScreenShare();
      setVoiceStatus(enabled ? "Screen share enabled." : "Screen share stopped.");
    } catch (error) {
      setVoiceError(mapRtcError(error, "Unable to update screen share."));
    } finally {
      setTogglingVoiceScreenShare(false);
    }
  };

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

  return (
    <div class="app-shell-scaffold">
      <div
        classList={{
          "app-shell": true,
          "channel-rail-collapsed": isChannelRailCollapsed(),
          "member-rail-collapsed": isMemberRailCollapsed(),
        }}
      >
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

        <Show when={!isChannelRailCollapsed()}>
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
        </Show>

        <main class="chat-panel">
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

        <Show
          when={workspaceBootstrapDone() && workspaces().length === 0}
          fallback={
            <>
              <Show when={!workspaceBootstrapDone()}>
                <p class="panel-note">Validating workspace access...</p>
              </Show>
              <Show when={workspaceBootstrapDone()}>
                <Show when={isLoadingMessages()}>
                  <p class="panel-note">Loading messages...</p>
                </Show>
                <Show when={messageError()}>
                  <p class="status error panel-note">{messageError()}</p>
                </Show>
                <Show when={sessionStatus()}>
                  <p class="status ok panel-note">{sessionStatus()}</p>
                </Show>
                <Show when={sessionError()}>
                  <p class="status error panel-note">{sessionError()}</p>
                </Show>
                <Show when={voiceStatus() && (canShowVoiceHeaderControls() || isVoiceSessionActive())}>
                  <p class="status ok panel-note">{voiceStatus()}</p>
                </Show>
                <Show when={voiceError() && (canShowVoiceHeaderControls() || isVoiceSessionActive())}>
                  <p class="status error panel-note">{voiceError()}</p>
                </Show>
                <Show when={activeChannel() && !canAccessActiveChannel()}>
                  <p class="status error panel-note">
                    Channel is not visible with your current default permissions.
                  </p>
                </Show>

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

                <ReactionPickerPortal
                  openMessageId={openReactionPickerMessageId()}
                  position={reactionPickerOverlayPosition()}
                  options={OPENMOJI_REACTION_OPTIONS}
                  onClose={() => setOpenReactionPickerMessageId(null)}
                  onAddReaction={(messageId, emoji) =>
                    addReactionFromPicker(messageId, emoji)}
                />
              </Show>
            </>
          }
        >
          <section class="empty-workspace">
            <h3>Create your first workspace</h3>
            <p class="muted">Use the + button in the workspace rail to create your first guild and channel.</p>
          </section>
        </Show>

        <Show when={messageStatus()}>
          <p class="status ok panel-note">{messageStatus()}</p>
        </Show>
      </main>

        <Show when={!isMemberRailCollapsed()}>
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
        </Show>
      </div>

      <PanelHost
        panel={activeOverlayPanel()}
        canCloseActivePanel={canCloseActivePanel()}
        canManageWorkspaceChannels={canManageWorkspaceChannels()}
        canAccessActiveChannel={canAccessActiveChannel()}
        hasModerationAccess={hasModerationAccess()}
        panelTitle={overlayPanelTitle}
        panelClassName={overlayPanelClassName}
        onClose={closeOverlayPanel}
        workspaceCreatePanelProps={{
          createGuildName: createGuildName(),
          createGuildVisibility: createGuildVisibility(),
          createChannelName: createChannelName(),
          createChannelKind: createChannelKind(),
          isCreatingWorkspace: isCreatingWorkspace(),
          canDismissWorkspaceCreateForm: canDismissWorkspaceCreateForm(),
          workspaceError: workspaceError(),
          onSubmit: createWorkspace,
          onCreateGuildNameInput: setCreateGuildName,
          onCreateGuildVisibilityChange: (value) =>
            setCreateGuildVisibility(guildVisibilityFromInput(value)),
          onCreateChannelNameInput: setCreateChannelName,
          onCreateChannelKindChange: (value) =>
            setCreateChannelKind(channelKindFromInput(value)),
          onCancel: closeOverlayPanel,
        }}
        channelCreatePanelProps={{
          newChannelName: newChannelName(),
          newChannelKind: newChannelKind(),
          isCreatingChannel: isCreatingChannel(),
          channelCreateError: channelCreateError(),
          onSubmit: createNewChannel,
          onNewChannelNameInput: setNewChannelName,
          onNewChannelKindChange: (value) =>
            setNewChannelKind(channelKindFromInput(value)),
          onCancel: closeOverlayPanel,
        }}
        publicDirectoryPanelProps={{
          searchQuery: publicGuildSearchQuery(),
          isSearching: isSearchingPublicGuilds(),
          searchError: publicGuildSearchError(),
          guilds: publicGuildDirectory(),
          onSubmitSearch: runPublicGuildSearch,
          onSearchInput: setPublicGuildSearchQuery,
        }}
        settingsPanelProps={{
          settingsCategories: SETTINGS_CATEGORIES,
          voiceSettingsSubmenu: VOICE_SETTINGS_SUBMENU,
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
          profileStatus: profileSettingsStatus(),
          profileError: profileSettingsError(),
          onOpenSettingsCategory: openSettingsCategory,
          onOpenVoiceSettingsSubmenu: setActiveVoiceSettingsSubmenu,
          onSetVoiceDevicePreference: (kind, value) =>
            setVoiceDevicePreference(kind, value),
          onRefreshAudioDeviceInventory: refreshAudioDeviceInventory,
          onProfileUsernameInput: setProfileDraftUsername,
          onProfileAboutInput: setProfileDraftAbout,
          onSelectProfileAvatarFile: setSelectedProfileAvatarFile,
          onSaveProfile: saveProfileSettings,
          onUploadProfileAvatar: uploadProfileAvatar,
        }}
        friendshipsPanelProps={{
          friendRecipientUserIdInput: friendRecipientUserIdInput(),
          friendRequests: friendRequests(),
          friends: friends(),
          isRunningFriendAction: isRunningFriendAction(),
          friendStatus: friendStatus(),
          friendError: friendError(),
          onSubmitFriendRequest: submitFriendRequest,
          onFriendRecipientInput: setFriendRecipientUserIdInput,
          onAcceptIncomingFriendRequest: (requestId) =>
            acceptIncomingFriendRequest(requestId),
          onDismissFriendRequest: (requestId) => dismissFriendRequest(requestId),
          onRemoveFriendship: (friendUserId) => removeFriendship(friendUserId),
        }}
        searchPanelProps={{
          searchQuery: searchQuery(),
          isSearching: isSearching(),
          hasActiveWorkspace: Boolean(activeWorkspace()),
          canManageSearchMaintenance: canManageSearchMaintenance(),
          isRunningSearchOps: isRunningSearchOps(),
          searchOpsStatus: searchOpsStatus(),
          searchError: searchError(),
          searchResults: searchResults(),
          onSubmitSearch: runSearch,
          onSearchQueryInput: setSearchQuery,
          onRebuildSearch: rebuildSearch,
          onReconcileSearch: reconcileSearch,
          displayUserLabel,
        }}
        attachmentsPanelProps={{
          attachmentFilename: attachmentFilename(),
          activeAttachments: activeAttachments(),
          isUploadingAttachment: isUploadingAttachment(),
          hasActiveChannel: Boolean(activeChannel()),
          attachmentStatus: attachmentStatus(),
          attachmentError: attachmentError(),
          downloadingAttachmentId: downloadingAttachmentId(),
          deletingAttachmentId: deletingAttachmentId(),
          onSubmitUpload: uploadAttachment,
          onAttachmentFileInput: (file) => {
            setSelectedAttachment(file);
            setAttachmentFilename(file?.name ?? "");
          },
          onAttachmentFilenameInput: setAttachmentFilename,
          onDownloadAttachment: (record) => downloadAttachment(record),
          onRemoveAttachment: (record) => removeAttachment(record),
        }}
        moderationPanelProps={{
          moderationUserIdInput: moderationUserIdInput(),
          moderationRoleInput: moderationRoleInput(),
          overrideRoleInput: overrideRoleInput(),
          overrideAllowCsv: overrideAllowCsv(),
          overrideDenyCsv: overrideDenyCsv(),
          isModerating: isModerating(),
          hasActiveWorkspace: Boolean(activeWorkspace()),
          hasActiveChannel: Boolean(activeChannel()),
          canManageRoles: canManageRoles(),
          canBanMembers: canBanMembers(),
          canManageChannelOverrides: canManageChannelOverrides(),
          moderationStatus: moderationStatus(),
          moderationError: moderationError(),
          onModerationUserIdInput: setModerationUserIdInput,
          onModerationRoleChange: (value) =>
            setModerationRoleInput(roleFromInput(value)),
          onRunMemberAction: (action) => runMemberAction(action),
          onOverrideRoleChange: (value) =>
            setOverrideRoleInput(roleFromInput(value)),
          onOverrideAllowInput: setOverrideAllowCsv,
          onOverrideDenyInput: setOverrideDenyCsv,
          onApplyOverride: applyOverride,
        }}
        utilityPanelProps={{
          echoInput: echoInput(),
          healthStatus: healthStatus(),
          diagError: diagError(),
          isCheckingHealth: isCheckingHealth(),
          isEchoing: isEchoing(),
          onEchoInput: setEchoInput,
          onRunHealthCheck: runHealthCheck,
          onRunEcho: runEcho,
        }}
      />

      <Show when={selectedProfileUserId()}>
        <div
          class="panel-backdrop"
          role="presentation"
          onClick={(event) => {
            if (event.target === event.currentTarget) {
              setSelectedProfileUserId(null);
            }
          }}
        >
          <section
            class="panel-window panel-window-compact profile-view-panel"
            role="dialog"
            aria-modal="true"
            aria-label="User profile panel"
          >
            <header class="panel-window-header">
              <h4>User profile</h4>
              <button type="button" onClick={() => setSelectedProfileUserId(null)}>
                Close
              </button>
            </header>
            <div class="panel-window-body">
              <Show when={selectedProfile.loading}>
                <p class="panel-note">Loading profile...</p>
              </Show>
              <Show when={selectedProfileError()}>
                <p class="status error">{selectedProfileError()}</p>
              </Show>
              <Show when={selectedProfile()}>
                {(value) => (
                  <section class="profile-view-body">
                    <div class="profile-view-header">
                      <span class="profile-view-avatar" aria-hidden="true">
                        <span class="profile-view-avatar-fallback">
                          {value().username.slice(0, 1).toUpperCase()}
                        </span>
                        <Show when={avatarUrlForUser(value().userId)}>
                          <img
                            class="profile-view-avatar-image"
                            src={avatarUrlForUser(value().userId)!}
                            alt={`${value().username} avatar`}
                            loading="lazy"
                            decoding="async"
                            referrerPolicy="no-referrer"
                            onError={(event) => {
                              event.currentTarget.style.display = "none";
                            }}
                          />
                        </Show>
                      </span>
                      <div>
                        <p class="profile-view-name">{value().username}</p>
                        <p class="mono">{value().userId}</p>
                      </div>
                    </div>
                    <SafeMarkdown class="profile-view-markdown" tokens={value().aboutMarkdownTokens} />
                  </section>
                )}
              </Show>
            </div>
          </section>
        </div>
      </Show>
    </div>
  );
}
