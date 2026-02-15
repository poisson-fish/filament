import {
  decodeVoiceStreamGatewayEvent,
  isVoiceStreamGatewayEventType,
} from "../src/lib/gateway-voice-stream-events";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEFAULT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";

describe("decodeVoiceStreamGatewayEvent", () => {
  it("exposes strict voice stream event type guard from decoder registry", () => {
    expect(isVoiceStreamGatewayEventType("voice_stream_publish")).toBe(true);
    expect(isVoiceStreamGatewayEventType("voice_participant_sync")).toBe(false);
  });

  it("decodes valid voice_stream_publish payload", () => {
    const result = decodeVoiceStreamGatewayEvent("voice_stream_publish", {
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      user_id: DEFAULT_USER_ID,
      identity: "user.identity",
      stream: "microphone",
      published_at_unix: 6,
    });

    expect(result).toEqual({
      type: "voice_stream_publish",
      payload: {
        guildId: DEFAULT_GUILD_ID,
        channelId: DEFAULT_CHANNEL_ID,
        userId: DEFAULT_USER_ID,
        identity: "user.identity",
        stream: "microphone",
        publishedAtUnix: 6,
      },
    });
  });

  it("fails closed for invalid voice_stream_unpublish payload", () => {
    const result = decodeVoiceStreamGatewayEvent("voice_stream_unpublish", {
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      user_id: DEFAULT_USER_ID,
      identity: "user.identity",
      stream: "invalid",
      unpublished_at_unix: 6,
    });

    expect(result).toBeNull();
  });

  it("returns null for unknown event type", () => {
    const result = decodeVoiceStreamGatewayEvent("voice_unknown", {
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      user_id: DEFAULT_USER_ID,
      identity: "user.identity",
      stream: "microphone",
      unpublished_at_unix: 6,
    });

    expect(result).toBeNull();
  });

  it("fails closed for prototype-key event type", () => {
    const result = decodeVoiceStreamGatewayEvent("__proto__", {
      guild_id: DEFAULT_GUILD_ID,
      channel_id: DEFAULT_CHANNEL_ID,
      user_id: DEFAULT_USER_ID,
      identity: "user.identity",
      stream: "microphone",
      unpublished_at_unix: 6,
    });

    expect(result).toBeNull();
  });
});
