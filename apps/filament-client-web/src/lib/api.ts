import { createAuthApi } from "./api-auth";
import { createAuthClient } from "./api-auth-client";
import { createFriendsApi } from "./api-friends";
import { createFriendsClient } from "./api-friends-client";
import { createMessagesApi } from "./api-messages";
import { createMessagesClient } from "./api-messages-client";
import { createSystemApi } from "./api-system";
import { createSystemClient } from "./api-system-client";
import { ApiError, createApiTransport } from "./api-transport";
import { createVoiceApi } from "./api-voice";
import { createVoiceClient } from "./api-voice-client";
import { createWorkspaceApi } from "./api-workspace";
import { createWorkspaceClient } from "./api-workspace-client";

const transport = createApiTransport();

const authApi = createAuthApi({
  requestJson: transport.requestJson,
  requestJsonWithBody: transport.requestJsonWithBody,
  requestNoContent: transport.requestNoContent,
  createApiError: transport.createApiError,
  apiBaseUrl: transport.apiBaseUrl,
});

const authClient = createAuthClient({ authApi });

const friendsApi = createFriendsApi({
  requestJson: transport.requestJson,
  requestNoContent: transport.requestNoContent,
  createApiError: transport.createApiError,
});

const friendsClient = createFriendsClient({
  friendsApi,
});

const messagesApi = createMessagesApi({
  requestJson: transport.requestJson,
  requestNoContent: transport.requestNoContent,
  requestJsonWithBody: transport.requestJsonWithBody,
  requestBinary: transport.requestBinary,
  createApiError: transport.createApiError,
  isApiErrorCode: transport.isApiErrorCode,
});

const messagesClient = createMessagesClient({
  messagesApi,
});

const voiceApi = createVoiceApi({
  requestJson: transport.requestJson,
  requestNoContent: transport.requestNoContent,
});

const voiceClient = createVoiceClient({
  voiceApi,
});

const workspaceApi = createWorkspaceApi({
  requestJson: transport.requestJson,
  createApiError: transport.createApiError,
});

const workspaceClient = createWorkspaceClient({
  workspaceApi,
});

const systemApi = createSystemApi({
  requestJson: transport.requestJson,
  createApiError: transport.createApiError,
});

const systemClient = createSystemClient({
  systemApi,
});

export { ApiError };

export const registerWithPassword = authClient.registerWithPassword;
export const loginWithPassword = authClient.loginWithPassword;
export const refreshAuthSession = authClient.refreshAuthSession;
export const logoutAuthSession = authClient.logoutAuthSession;
export const fetchMe = authClient.fetchMe;
export const fetchUserProfile = authClient.fetchUserProfile;
export const updateMyProfile = authClient.updateMyProfile;
export const uploadMyProfileAvatar = authClient.uploadMyProfileAvatar;
export const uploadMyProfileBanner = authClient.uploadMyProfileBanner;
export const profileAvatarUrl = authClient.profileAvatarUrl;
export const profileBannerUrl = authClient.profileBannerUrl;
export const lookupUsersByIds = authClient.lookupUsersByIds;

export const fetchFriends = friendsClient.fetchFriends;
export const fetchFriendRequests = friendsClient.fetchFriendRequests;
export const createFriendRequest = friendsClient.createFriendRequest;
export const acceptFriendRequest = friendsClient.acceptFriendRequest;
export const deleteFriendRequest = friendsClient.deleteFriendRequest;
export const removeFriend = friendsClient.removeFriend;

export const fetchHealth = systemClient.fetchHealth;
export const echoMessage = systemClient.echoMessage;

export const createGuild = workspaceClient.createGuild;
export const fetchGuilds = workspaceClient.fetchGuilds;
export const updateGuild = workspaceClient.updateGuild;
export const fetchPublicGuildDirectory = workspaceClient.fetchPublicGuildDirectory;
export const joinPublicGuild = workspaceClient.joinPublicGuild;
export const fetchGuildChannels = workspaceClient.fetchGuildChannels;
export const createChannel = workspaceClient.createChannel;
export const fetchChannelPermissionSnapshot = workspaceClient.fetchChannelPermissionSnapshot;
export const fetchGuildMembers = workspaceClient.fetchGuildMembers;

export const fetchChannelMessages = messagesClient.fetchChannelMessages;
export const createChannelMessage = messagesClient.createChannelMessage;
export const editChannelMessage = messagesClient.editChannelMessage;
export const deleteChannelMessage = messagesClient.deleteChannelMessage;
export const searchGuildMessages = messagesClient.searchGuildMessages;
export const rebuildGuildSearchIndex = messagesClient.rebuildGuildSearchIndex;
export const reconcileGuildSearchIndex = messagesClient.reconcileGuildSearchIndex;
export const addMessageReaction = messagesClient.addMessageReaction;
export const removeMessageReaction = messagesClient.removeMessageReaction;
export const uploadChannelAttachment = messagesClient.uploadChannelAttachment;
export const downloadChannelAttachment = messagesClient.downloadChannelAttachment;
export const downloadChannelAttachmentPreview = messagesClient.downloadChannelAttachmentPreview;
export const deleteChannelAttachment = messagesClient.deleteChannelAttachment;

export const addGuildMember = workspaceClient.addGuildMember;
export const updateGuildMemberRole = workspaceClient.updateGuildMemberRole;
export const kickGuildMember = workspaceClient.kickGuildMember;
export const banGuildMember = workspaceClient.banGuildMember;
export const setChannelRoleOverride = workspaceClient.setChannelRoleOverride;
export const fetchGuildRoles = workspaceClient.fetchGuildRoles;
export const createGuildRole = workspaceClient.createGuildRole;
export const updateGuildRole = workspaceClient.updateGuildRole;
export const deleteGuildRole = workspaceClient.deleteGuildRole;
export const reorderGuildRoles = workspaceClient.reorderGuildRoles;
export const assignGuildRole = workspaceClient.assignGuildRole;
export const unassignGuildRole = workspaceClient.unassignGuildRole;
export const updateGuildDefaultJoinRole = workspaceClient.updateGuildDefaultJoinRole;

export const issueVoiceToken = voiceClient.issueVoiceToken;
export const leaveVoiceChannel = voiceClient.leaveVoiceChannel;
export const updateVoiceParticipantState = voiceClient.updateVoiceParticipantState;
