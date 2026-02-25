import { useAuth } from "../lib/auth-context";
import { profileErrorMessage } from "../features/app-shell/helpers";
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
import { StreamColumn } from "../features/app-shell/components/stream/StreamColumn";
import {
  ADD_REACTION_ICON_URL,
  DELETE_MESSAGE_ICON_URL,
  EDIT_MESSAGE_ICON_URL,
} from "../features/app-shell/config/ui-constants";
import {
  overlayPanelClassName,
  overlayPanelTitle,
} from "../features/app-shell/controllers/overlay-controller";
import { createAppShellRuntime } from "../features/app-shell/runtime/create-app-shell-runtime";

export function AppShellPage() {
  const auth = useAuth();
  const runtime = createAppShellRuntime(auth);

  const { workspaceState, messageState, profileState, voiceState, diagnosticsState, overlayState } =
    runtime;
  const {
    activeWorkspace,
    activeChannel,
    activeTextChannels,
    activeVoiceChannels,
    canAccessActiveChannel,
    canManageWorkspaceChannels,
    hasRoleManagementAccess,
    hasModerationAccess,
    canDeleteMessages,
    activeVoiceSessionLabel,
    voiceConnectionState,
    isVoiceSessionActive,
    isVoiceSessionForChannel,
    voiceRosterEntriesForChannel,
    canToggleVoiceCamera,
    canToggleVoiceScreenShare,
    canShowVoiceHeaderControls,
    voiceStreamPermissionHints,
    voiceSessionDurationLabel,
    canCloseActivePanel,
  } = runtime.selectors;
  const { profile, selectedProfile, avatarUrlForUser, openUserProfile } =
    runtime.profileController;

  const {
    messageMediaByAttachmentId,
    loadingMediaPreviewIds,
    failedMediaPreviewIds,
    retryMediaPreview,
  } = runtime.messageMediaPreviewController;

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
  } = runtime.messageActions;

  const { downloadAttachment } = runtime.attachmentActions;

  const { refreshMessages, loadOlderMessages } = runtime.messageHistoryActions;
  const { refreshSession, logout } = runtime.sessionDiagnostics;

  const {
    messageListController,
    openOverlayPanel,
    closeOverlayPanel,
    openClientSettingsPanel,
    openWorkspaceSettingsPanel,
    panelHostPropGroups,
    actorLabel,
    actorColor,
    displayUserLabel,
    displayUserColor,
    voiceParticipantLabel,
    userIdFromVoiceIdentity,
    joinVoiceChannel,
    leaveVoiceChannel,
    toggleVoiceMicrophone,
    toggleVoiceDeafen,
    toggleVoiceCamera,
    toggleVoiceScreenShare,
    setComposerAttachmentInputRef,
    setComposerInputRef,
    openTextChannelCreatePanel,
    openVoiceChannelCreatePanel,
    onSelectWorkspace,
  } = runtime;

  return (
    <AppShellLayout
      isChannelRailCollapsed={overlayState.isChannelRailCollapsed()}
      isMemberRailCollapsed={overlayState.isMemberRailCollapsed()}
      serverRail={
        <ServerRail
          workspaces={workspaceState.workspaceChannel.workspaces()}
          activeGuildId={workspaceState.workspaceChannel.activeGuildId()}
          isCreatingWorkspace={workspaceState.workspaceChannel.isCreatingWorkspace()}
          onSelectWorkspace={onSelectWorkspace}
          onOpenPanel={openOverlayPanel}
        />
      }
      channelRail={
        <ChannelRail
          activeWorkspace={activeWorkspace()}
          activeChannel={activeChannel()}
          activeChannelId={workspaceState.workspaceChannel.activeChannelId()}
          activeTextChannels={activeTextChannels()}
          activeVoiceChannels={activeVoiceChannels()}
          canManageWorkspaceChannels={canManageWorkspaceChannels()}
          canShowVoiceHeaderControls={canShowVoiceHeaderControls()}
          isVoiceSessionActive={isVoiceSessionActive()}
          isVoiceSessionForChannel={isVoiceSessionForChannel}
          voiceSessionDurationLabel={voiceSessionDurationLabel()}
          voiceRosterEntriesForChannel={voiceRosterEntriesForChannel}
          voiceStreamPermissionHints={voiceStreamPermissionHints()}
          activeVoiceSessionLabel={activeVoiceSessionLabel()}
          rtcSnapshot={voiceState.rtcSnapshot()}
          canToggleVoiceCamera={canToggleVoiceCamera()}
          canToggleVoiceScreenShare={canToggleVoiceScreenShare()}
          isJoiningVoice={voiceState.isJoiningVoice()}
          isLeavingVoice={voiceState.isLeavingVoice()}
          isTogglingVoiceMic={voiceState.isTogglingVoiceMic()}
          isTogglingVoiceDeaf={voiceState.isTogglingVoiceDeaf()}
          isTogglingVoiceCamera={voiceState.isTogglingVoiceCamera()}
          isTogglingVoiceScreenShare={voiceState.isTogglingVoiceScreenShare()}
          currentUserId={profile()?.userId ?? null}
          currentUserLabel={profile()?.username}
          currentUserStatusLabel={profileState.gatewayOnline() ? "Online" : "Offline"}
          resolveAvatarUrl={avatarUrlForUser}
          userIdFromVoiceIdentity={userIdFromVoiceIdentity}
          actorLabel={actorLabel}
          resolveUserNameColor={displayUserColor}
          voiceParticipantLabel={voiceParticipantLabel}
          onOpenUserProfile={openUserProfile}
          onOpenClientSettings={openClientSettingsPanel}
          onOpenWorkspaceSettings={openWorkspaceSettingsPanel}
          onCreateTextChannel={openTextChannelCreatePanel}
          onCreateVoiceChannel={openVoiceChannelCreatePanel}
          onSelectChannel={(channelId) => workspaceState.workspaceChannel.setActiveChannelId(channelId)}
          onJoinVoice={() => void joinVoiceChannel()}
          onToggleVoiceMicrophone={() => void toggleVoiceMicrophone()}
          onToggleVoiceDeafen={() => void toggleVoiceDeafen()}
          onToggleVoiceCamera={() => void toggleVoiceCamera()}
          onToggleVoiceScreenShare={() => void toggleVoiceScreenShare()}
          onLeaveVoice={() => void leaveVoiceChannel("Voice session ended.")}
        />
      }
      streamColumn={
        isVoiceSessionActive() ? (
          <StreamColumn
            rtcSnapshot={voiceState.rtcSnapshot()}
            userIdFromVoiceIdentity={userIdFromVoiceIdentity}
            actorLabel={actorLabel}
            resolveActorNameColor={actorColor}
            resolveAvatarUrl={avatarUrlForUser}
            attachVideoTrack={runtime.attachVideoTrack}
            detachVideoTrack={runtime.detachVideoTrack}
            canToggleVoiceCamera={canToggleVoiceCamera()}
            canToggleVoiceScreenShare={canToggleVoiceScreenShare()}
            isJoiningVoice={voiceState.isJoiningVoice()}
            isLeavingVoice={voiceState.isLeavingVoice()}
            isTogglingVoiceMic={voiceState.isTogglingVoiceMic()}
            isTogglingVoiceDeaf={voiceState.isTogglingVoiceDeaf()}
            isTogglingVoiceCamera={voiceState.isTogglingVoiceCamera()}
            isTogglingVoiceScreenShare={voiceState.isTogglingVoiceScreenShare()}
            onToggleVoiceMicrophone={() => void toggleVoiceMicrophone()}
            onToggleVoiceDeafen={() => void toggleVoiceDeafen()}
            onToggleVoiceCamera={() => void toggleVoiceCamera()}
            onToggleVoiceScreenShare={() => void toggleVoiceScreenShare()}
            onLeaveVoice={() => void leaveVoiceChannel("Voice session ended.")}
          />
        ) : undefined
      }
      chatColumn={
        <ChatColumn
          chatHeader={
            <ChatHeader
              activeChannel={activeChannel()}
              gatewayOnline={profileState.gatewayOnline()}
              canShowVoiceHeaderControls={canShowVoiceHeaderControls()}
              isVoiceSessionActive={isVoiceSessionActive()}
              voiceConnectionState={voiceConnectionState()}
              isChannelRailCollapsed={overlayState.isChannelRailCollapsed()}
              isMemberRailCollapsed={overlayState.isMemberRailCollapsed()}
              isRefreshingSession={diagnosticsState.isRefreshingSession()}
              onToggleChannelRail={() =>
                overlayState.setChannelRailCollapsed((value) => !value)}
              onToggleMemberRail={() =>
                overlayState.setMemberRailCollapsed((value) => !value)}
              onOpenPanel={openOverlayPanel}
              onRefreshMessages={() => void refreshMessages()}
              onRefreshSession={() => void refreshSession()}
              onLogout={() => void logout()}
            />
          }
          workspaceBootstrapDone={workspaceState.workspaceChannel.workspaceBootstrapDone()}
          workspaceCount={workspaceState.workspaceChannel.workspaces().length}
          isLoadingMessages={messageState.isLoadingMessages()}
          messageError={messageState.messageError()}
          sessionStatus={diagnosticsState.sessionStatus()}
          sessionError={diagnosticsState.sessionError()}
          voiceStatus={voiceState.voiceStatus()}
          voiceError={voiceState.voiceError()}
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
              nextBefore={messageState.nextBefore()}
              showLoadOlderButton={messageState.showLoadOlderButton()}
              isLoadingOlder={messageState.isLoadingOlder()}
              isLoadingMessages={messageState.isLoadingMessages()}
              messageError={messageState.messageError()}
              messages={messageState.messages()}
              onLoadOlderMessages={() => loadOlderMessages()}
              currentUserId={profile()?.userId ?? null}
              canDeleteMessages={canDeleteMessages()}
              displayUserLabel={displayUserLabel}
              resolveUserNameColor={displayUserColor}
              resolveAvatarUrl={avatarUrlForUser}
              onOpenAuthorProfile={openUserProfile}
              editingMessageId={messageState.editingMessageId()}
              editingDraft={messageState.editingDraft()}
              isSavingEdit={messageState.isSavingEdit()}
              deletingMessageId={messageState.deletingMessageId()}
              openReactionPickerMessageId={messageState.openReactionPickerMessageId()}
              reactionState={messageState.reactionState()}
              pendingReactionByKey={messageState.pendingReactionByKey()}
              messageMediaByAttachmentId={messageMediaByAttachmentId()}
              loadingMediaPreviewIds={loadingMediaPreviewIds()}
              failedMediaPreviewIds={failedMediaPreviewIds()}
              downloadingAttachmentId={messageState.downloadingAttachmentId()}
              addReactionIconUrl={ADD_REACTION_ICON_URL}
              editMessageIconUrl={EDIT_MESSAGE_ICON_URL}
              deleteMessageIconUrl={DELETE_MESSAGE_ICON_URL}
              onEditingDraftInput={runtime.messageState.setEditingDraft}
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
                setComposerAttachmentInputRef(value);
              }}
              composerInputRef={(value) => {
                setComposerInputRef(value);
              }}
              activeChannel={activeChannel()}
              canAccessActiveChannel={canAccessActiveChannel()}
              isSendingMessage={messageState.isSendingMessage()}
              composerValue={messageState.composer()}
              composerAttachments={messageState.composerAttachments()}
              onSubmit={sendMessage}
              onComposerInput={runtime.messageState.setComposer}
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
              openMessageId={messageState.openReactionPickerMessageId()}
              onClose={() => messageState.setOpenReactionPickerMessageId(null)}
              onAddReaction={(messageId, emoji) =>
                addReactionFromPicker(messageId, emoji)}
            />
          }
          messageStatus={messageState.messageStatus()}
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
          onlineMembers={profileState.onlineMembers()}
          hasRoleManagementAccess={hasRoleManagementAccess()}
          hasModerationAccess={hasModerationAccess()}
          displayUserLabel={displayUserLabel}
          resolveUserNameColor={displayUserColor}
          onOpenPanel={openOverlayPanel}
          onOpenWorkspaceRoleSettings={() => openWorkspaceSettingsPanel("roles")}
        />
      }
    >
      <PanelHost
        panel={overlayState.activeOverlayPanel()}
        canCloseActivePanel={canCloseActivePanel()}
        canManageWorkspaceChannels={canManageWorkspaceChannels()}
        canAccessActiveChannel={canAccessActiveChannel()}
        hasRoleManagementAccess={hasRoleManagementAccess()}
        hasModerationAccess={hasModerationAccess()}
        panelTitle={overlayPanelTitle}
        panelClassName={overlayPanelClassName}
        onClose={closeOverlayPanel}
        {...panelHostPropGroups()}
      />
      <UserProfileOverlay
        selectedProfileUserId={profileState.selectedProfileUserId()}
        selectedProfileLoading={selectedProfile.loading}
        selectedProfileError={profileState.selectedProfileError()}
        selectedProfile={selectedProfile() ?? null}
        avatarUrlForUser={avatarUrlForUser}
        onClose={() => profileState.setSelectedProfileUserId(null)}
      />
    </AppShellLayout>
  );
}
