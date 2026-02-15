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
    expect(isVoiceGatewayEventType("voice_participant_sync")).toBe(true);
    expect(isVoiceGatewayEventType("message_create")).toBe(false);
  });

  it("delegates participant payload decoding through the aggregate decoder", () => {
    const result = decodeVoiceGatewayEvent("voice_participant_update", {
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      user_id: DEFAULT_USER_ID,
      identity: "user.identity",
      updated_fields: {
        is_muted: true,
      },
      updated_at_unix: 5,
    });

    expect(result).toEqual({
      type: "voice_participant_update",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        channelId: DEFAULT_CHANNEL_ID,
        userId: DEFAULT_USER_ID,
        identity: "user.identity",
        updatedFields: {
          isMuted: true,
          isDeafened: undefined,
          isSpeaking: undefined,
          isVideoEnabled: undefined,
          isScreenShareEnabled: undefined,
        },
        updatedAtUnix: 5,
      },
    });
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
