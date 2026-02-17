import {
  type AuthSession,
  type AccessToken,
} from "../domain/auth";
import {
  type ChannelId,
  type GuildId,
  type MediaPublishSource,
  type VoiceTokenRecord,
  voiceTokenFromResponse,
} from "../domain/chat";

interface JsonRequest {
  method: "GET" | "POST" | "PATCH" | "DELETE";
  path: string;
  body?: unknown;
  accessToken?: AccessToken;
}

interface VoiceApiDependencies {
  requestJson: (request: JsonRequest) => Promise<unknown>;
  requestNoContent: (request: JsonRequest) => Promise<void>;
}

export interface VoiceApi {
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
}

export function createVoiceApi(input: VoiceApiDependencies): VoiceApi {
  return {
    async issueVoiceToken(session, guildId, channelId, payload) {
      const dto = await input.requestJson({
        method: "POST",
        path: `/guilds/${guildId}/channels/${channelId}/voice/token`,
        accessToken: session.accessToken,
        body: {
          can_publish: payload.canPublish,
          can_subscribe: payload.canSubscribe,
          publish_sources: payload.publishSources,
        },
      });

      return voiceTokenFromResponse(dto);
    },
    async leaveVoiceChannel(session, guildId, channelId) {
      await input.requestNoContent({
        method: "POST",
        path: `/guilds/${guildId}/channels/${channelId}/voice/leave`,
        accessToken: session.accessToken,
      });
    },
  };
}