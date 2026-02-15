import {
  decodeVoiceGatewayEvent,
  isVoiceGatewayEventType,
} from "../src/lib/gateway-voice-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEFAULT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";

describe("decodeVoiceGatewayEvent", () => {
  it("exposes strict voice event type guard from decoder registry", () => {
    expect(isVoiceGatewayEventType("voice_stream_publish")).toBe(true);
    expect(isVoiceGatewayEventType("message_create")).toBe(false);
  });

  it("decodes valid voice_participant_sync payload with deduped identities", () => {
    const result = decodeVoiceGatewayEvent("voice_participant_sync", {
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      participants: [
        {
          user_id: DEFAULT_USER_ID,
          identity: "user.identity",
          joined_at_unix: 1,
          updated_at_unix: 2,
          is_muted: false,
          is_deafened: false,
          is_speaking: true,
          is_video_enabled: false,
          is_screen_share_enabled: false,
        },
        {
          user_id: DEFAULT_USER_ID,
          identity: "user.identity",
          joined_at_unix: 1,
          updated_at_unix: 2,
          is_muted: false,
          is_deafened: false,
          is_speaking: true,
          is_video_enabled: false,
          is_screen_share_enabled: false,
        },
      ],
      synced_at_unix: 3,
    });

    expect(result).toEqual({
      type: "voice_participant_sync",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        channelId: DEFAULT_CHANNEL_ID,
        participants: [
          {
            userId: DEFAULT_USER_ID,
            identity: "user.identity",
            joinedAtUnix: 1,
            updatedAtUnix: 2,
            isMuted: false,
            isDeafened: false,
            isSpeaking: true,
            isVideoEnabled: false,
            isScreenShareEnabled: false,
          },
        ],
        syncedAtUnix: 3,
      },
    });
  });

  it("fails closed for invalid voice_participant_update payload", () => {
    const result = decodeVoiceGatewayEvent("voice_participant_update", {
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      user_id: DEFAULT_USER_ID,
      identity: "user.identity",
      updated_fields: {},
      updated_at_unix: 5,
    });

    expect(result).toBeNull();
  });

  it("fails closed for invalid voice_stream_publish stream", () => {
    const result = decodeVoiceGatewayEvent("voice_stream_publish", {
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      user_id: DEFAULT_USER_ID,
      identity: "user.identity",
      stream: "invalid",
      published_at_unix: 6,
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown event type", () => {
    const result = decodeVoiceGatewayEvent("voice_unknown", {
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      participants: [],
      synced_at_unix: 1,
    });

    expect(result).toBeNull();
  });
});
