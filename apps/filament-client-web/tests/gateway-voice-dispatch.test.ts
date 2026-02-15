import {
  dispatchVoiceGatewayEvent,
} from "../src/lib/gateway-voice-dispatch";

const DEFAULT_GUILD_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEFAULT_CHANNEL_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAX";

describe("dispatchVoiceGatewayEvent", () => {
  it("dispatches decoded voice events to matching handlers", () => {
    const onVoiceParticipantSync = vi.fn();

    const handled = dispatchVoiceGatewayEvent(
      "voice_participant_sync",
      {
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
        ],
        synced_at_unix: 3,
      },
      { onVoiceParticipantSync },
    );

    expect(handled).toBe(true);
    expect(onVoiceParticipantSync).toHaveBeenCalledTimes(1);
    expect(onVoiceParticipantSync).toHaveBeenCalledWith({
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
    });
  });

  it("fails closed for known voice types with invalid payloads", () => {
    const onVoiceParticipantUpdate = vi.fn();

    const handled = dispatchVoiceGatewayEvent(
      "voice_participant_update",
      {
        guild_id: DEFAULT_GUILD_ID,
        channel_id: DEFAULT_CHANNEL_ID,
        user_id: DEFAULT_USER_ID,
        identity: "user.identity",
        updated_fields: {},
        updated_at_unix: 5,
      },
      { onVoiceParticipantUpdate },
    );

    expect(handled).toBe(true);
    expect(onVoiceParticipantUpdate).not.toHaveBeenCalled();
  });

  it("returns false for non-voice event types", () => {
    const onVoiceStreamPublish = vi.fn();

    const handled = dispatchVoiceGatewayEvent(
      "message_create",
      {},
      { onVoiceStreamPublish },
    );

    expect(handled).toBe(false);
    expect(onVoiceStreamPublish).not.toHaveBeenCalled();
  });
});
