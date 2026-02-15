import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";
import { channelIdFromInput, guildIdFromInput } from "../src/domain/chat";
import { createVoiceApi } from "../src/lib/api-voice";

describe("api-voice", () => {
  const session = {
    accessToken: accessTokenFromInput("A".repeat(64)),
    refreshToken: refreshTokenFromInput("B".repeat(64)),
    expiresAtUnix: 2_000_000_000,
  };
  const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
  const channelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB0");

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("issueVoiceToken sends scoped request and maps strict DTO", async () => {
    const requestJson = vi.fn(async () => ({
      token: "tok_valid_123",
      livekit_url: "wss://livekit.example.test",
      room: "guild-room",
      identity: "member-identity",
      can_publish: true,
      can_subscribe: false,
      publish_sources: ["microphone"],
      expires_in_secs: 300,
    }));

    const api = createVoiceApi({ requestJson });

    await expect(
      api.issueVoiceToken(session, guildId, channelId, {
        canPublish: true,
        canSubscribe: false,
        publishSources: ["microphone"],
      }),
    ).resolves.toMatchObject({
      room: "guild-room",
      identity: "member-identity",
      canPublish: true,
      canSubscribe: false,
    });

    expect(requestJson).toHaveBeenCalledWith({
      method: "POST",
      path: `/guilds/${guildId}/channels/${channelId}/voice/token`,
      accessToken: session.accessToken,
      body: {
        can_publish: true,
        can_subscribe: false,
        publish_sources: ["microphone"],
      },
    });
  });

  it("issueVoiceToken fails closed on invalid DTO shape", async () => {
    const api = createVoiceApi({
      requestJson: vi.fn(async () => ({
        token: "tok_valid_123",
        livekit_url: "wss://livekit.example.test",
        room: "guild-room",
        identity: "member-identity",
        can_publish: true,
        can_subscribe: false,
        publish_sources: ["microphone"],
        expires_in_secs: 0,
      })),
    });

    await expect(api.issueVoiceToken(session, guildId, channelId, {})).rejects.toThrow(
      "expires_in_secs",
    );
  });
});
