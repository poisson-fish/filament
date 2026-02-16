import {
  onCleanup,
} from "solid-js";
import { useAuth } from "../../../lib/auth-context";
import { createAttachmentController } from "../controllers/attachment-controller";
import {
  createFriendshipController,
} from "../controllers/friendship-controller";
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
} from "../controllers/voice-controller";
import {
  createChannelPermissionsController,
  createWorkspaceBootstrapController,
  createWorkspaceSelectionController,
} from "../controllers/workspace-controller";
import {
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
import { createOverlayPanelActions } from "./overlay-panel-actions";
import { createAppShellRuntimeLabels } from "./runtime-labels";
import { createWorkspacePermissionActions } from "./workspace-permission-actions";
import { createWorkspaceSettingsActions } from "./workspace-settings-actions";
import { createVoiceDeviceActions } from "./voice-device-actions";
import { createWorkspaceSelectionActions } from "./workspace-selection-actions";
import { createRuntimeEffects } from "./runtime-effects";
import { createSessionDiagnosticsActions } from "./session-diagnostics-actions";
import { registerGatewayController } from "./gateway-controller-registration";
import { createPanelHostPropGroupsFactory } from "./panel-host-prop-groups-factory";
import { createWorkspaceChannelOperationsActions } from "./workspace-channel-operations-actions";

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
    voiceParticipantsByChannel: voiceState.voiceParticipantsByChannel,
    voiceSessionCapabilities: voiceState.voiceSessionCapabilities,
    voiceSessionStartedAtUnixMs: voiceState.voiceSessionStartedAtUnixMs,
    voiceDurationClockUnixMs: voiceState.voiceDurationClockUnixMs,
    activeOverlayPanel: overlayState.activeOverlayPanel,
  });

  const workspaceSettingsActions = createWorkspaceSettingsActions({
    session: auth.session,
    activeGuildId: workspaceChannelState.activeGuildId,
    canManageRoles: selectors.canManageRoles,
    workspaceSettingsName: workspaceChannelState.workspaceSettingsName,
    workspaceSettingsVisibility: workspaceChannelState.workspaceSettingsVisibility,
    setSavingWorkspaceSettings: workspaceChannelState.setSavingWorkspaceSettings,
    setWorkspaceSettingsStatus: workspaceChannelState.setWorkspaceSettingsStatus,
    setWorkspaceSettingsError: workspaceChannelState.setWorkspaceSettingsError,
    setWorkspaces: workspaceChannelState.setWorkspaces,
    setWorkspaceSettingsName: workspaceChannelState.setWorkspaceSettingsName,
    setWorkspaceSettingsVisibility: workspaceChannelState.setWorkspaceSettingsVisibility,
  });

  const { saveWorkspaceSettings } = workspaceSettingsActions;

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
    setVoiceJoinState: voiceState.setVoiceJoinState,
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

  const { refreshAudioDeviceInventory, setVoiceDevicePreference } =
    createVoiceDeviceActions({
      voiceDevicePreferences: voiceState.voiceDevicePreferences,
      setVoiceDevicePreferences: voiceState.setVoiceDevicePreferences,
      audioInputDevices: voiceState.audioInputDevices,
      audioOutputDevices: voiceState.audioOutputDevices,
      isRefreshingAudioDevices: voiceState.isRefreshingAudioDevices,
      setRefreshingAudioDevices: voiceState.setRefreshingAudioDevices,
      setAudioInputDevices: voiceState.setAudioInputDevices,
      setAudioOutputDevices: voiceState.setAudioOutputDevices,
      setAudioDevicesStatus: voiceState.setAudioDevicesStatus,
      setAudioDevicesError: voiceState.setAudioDevicesError,
      isVoiceSessionActive: selectors.isVoiceSessionActive,
      peekRtcClient: voiceOperationsController.peekRtcClient,
    });

  const {
    openOverlayPanel,
    closeOverlayPanel,
    openSettingsCategory,
    openClientSettingsPanel,
    openWorkspaceSettingsPanel,
  } = createOverlayPanelActions({
    activeWorkspace: selectors.activeWorkspace,
    canCloseActivePanel: selectors.canCloseActivePanel,
    setWorkspaceSettingsName: workspaceChannelState.setWorkspaceSettingsName,
    setWorkspaceSettingsVisibility: workspaceChannelState.setWorkspaceSettingsVisibility,
    setWorkspaceSettingsStatus: workspaceChannelState.setWorkspaceSettingsStatus,
    setWorkspaceSettingsError: workspaceChannelState.setWorkspaceSettingsError,
    setActiveOverlayPanel: overlayState.setActiveOverlayPanel,
    setWorkspaceError: workspaceChannelState.setWorkspaceError,
    setChannelCreateError: workspaceChannelState.setChannelCreateError,
    setActiveSettingsCategory: overlayState.setActiveSettingsCategory,
    setActiveVoiceSettingsSubmenu: overlayState.setActiveVoiceSettingsSubmenu,
  });

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
    setSendMessageState: messageState.setSendMessageState,
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

  const { refreshWorkspacePermissionStateFromGateway } =
    createWorkspacePermissionActions({
      session: auth.session,
      activeGuildId: workspaceChannelState.activeGuildId,
      activeChannelId: workspaceChannelState.activeChannelId,
      setChannelPermissions: workspaceChannelState.setChannelPermissions,
      setWorkspaces: workspaceChannelState.setWorkspaces,
      refreshRoles: roleManagementActions.refreshRoles,
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

  createRuntimeEffects({
    workspaceBootstrapDone: workspaceChannelState.workspaceBootstrapDone,
    workspaces: workspaceChannelState.workspaces,
    setActiveOverlayPanel: overlayState.setActiveOverlayPanel,
    activeOverlayPanel: overlayState.activeOverlayPanel,
    activeSettingsCategory: overlayState.activeSettingsCategory,
    activeVoiceSettingsSubmenu: overlayState.activeVoiceSettingsSubmenu,
    refreshAudioDeviceInventory,
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
    setRefreshMessagesState: messageState.setRefreshMessagesState,
    setMessageHistoryLoadTarget: messageState.setMessageHistoryLoadTarget,
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

  registerGatewayController({
    session: auth.session,
    activeGuildId: workspaceChannelState.activeGuildId,
    activeChannelId: workspaceChannelState.activeChannelId,
    canAccessActiveChannel: selectors.canAccessActiveChannel,
    setGatewayOnline: profileState.setGatewayOnline,
    setOnlineMembers: profileState.setOnlineMembers,
    setWorkspaces: workspaceChannelState.setWorkspaces,
    setMessages: messageState.setMessages,
    setReactionState: messageState.setReactionState,
    setResolvedUsernames: profileState.setResolvedUsernames,
    setAvatarVersionByUserId: profileState.setAvatarVersionByUserId,
    setProfileDraftUsername: profileState.setProfileDraftUsername,
    setProfileDraftAbout: profileState.setProfileDraftAbout,
    setFriends: friendshipsState.setFriends,
    setFriendRequests: friendshipsState.setFriendRequests,
    setVoiceParticipantsByChannel: voiceState.setVoiceParticipantsByChannel,
    isMessageListNearBottom: messageListController.isMessageListNearBottom,
    scrollMessageListToBottom: messageListController.scrollMessageListToBottom,
    refreshWorkspacePermissionStateFromGateway,
  });

  const workspaceChannelOperations = createWorkspaceChannelOperationsActions({
    session: auth.session,
    workspaceChannelState,
    messageState,
    overlayState,
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

  const sessionDiagnostics = createSessionDiagnosticsActions({
    session: auth.session,
    setAuthenticatedSession: auth.setAuthenticatedSession,
    clearAuthenticatedSession: auth.clearAuthenticatedSession,
    leaveVoiceChannel,
    releaseRtcClient,
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

  const {
    openTextChannelCreatePanel,
    openVoiceChannelCreatePanel,
    onSelectWorkspace,
  } = createWorkspaceSelectionActions({
    setNewChannelKind: workspaceChannelState.setNewChannelKind,
    openOverlayPanel,
    setActiveGuildId: workspaceChannelState.setActiveGuildId,
    setActiveChannelId: workspaceChannelState.setActiveChannelId,
  });

  const panelHostPropGroups = createPanelHostPropGroupsFactory({
    workspaceChannelCreate: {
      workspaceChannelState,
      selectors,
      workspaceChannelOperations,
      closeOverlayPanel,
    },
    support: {
      discoveryState,
      overlayState,
      voiceState,
      profileState,
      workspaceChannelState,
      diagnosticsState,
      selectors,
      publicDirectoryActions,
      profileController,
      roleManagementActions,
      sessionDiagnostics,
      openSettingsCategory,
      setVoiceDevicePreference,
      refreshAudioDeviceInventory,
      saveWorkspaceSettings,
      openOverlayPanel,
    },
    collaboration: {
      friendshipsState,
      discoveryState,
      messageState,
      diagnosticsState,
      selectors,
      friendshipActions,
      searchActions,
      attachmentActions,
      moderationActions,
      labels,
      openOverlayPanel,
    },
  });

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
    openClientSettingsPanel,
    openWorkspaceSettingsPanel,
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
