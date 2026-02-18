import { type AuthSession } from "../domain/auth";
import {
  type ChannelId,
  type GuildId,
  type MediaPublishSource,
  type VoiceTokenRecord,
} from "../domain/chat";
import type { VoiceApi } from "./api-voice";

interface VoiceClientDependencies {
  voiceApi: VoiceApi;
}

export interface VoiceClient {
  issueVoiceToken(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
    input: {
      canPublish?: boolean;
      canSubscribe?: boolean;
      publishSources?: MediaPublishSource[];
    },
  ): Promise<VoiceTokenRecord>;
  leaveVoiceChannel(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
  ): Promise<void>;
  updateVoiceParticipantState(
    session: AuthSession,
    guildId: GuildId,
    channelId: ChannelId,
    input: {
      isMuted?: boolean;
      isDeafened?: boolean;
    },
  ): Promise<void>;
}

export function createVoiceClient(input: VoiceClientDependencies): VoiceClient {
  return {
    issueVoiceToken(session, guildId, channelId, payload) {
      return input.voiceApi.issueVoiceToken(session, guildId, channelId, payload);
    },
    leaveVoiceChannel(session, guildId, channelId) {
      return input.voiceApi.leaveVoiceChannel(session, guildId, channelId);
    },
    updateVoiceParticipantState(session, guildId, channelId, payload) {
      return input.voiceApi.updateVoiceParticipantState(session, guildId, channelId, payload);
    },
  };
}
