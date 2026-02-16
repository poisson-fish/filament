import { accessTokenFromInput, refreshTokenFromInput } from "../src/domain/auth";
import {
  channelIdFromInput,
  guildIdFromInput,
  voiceTokenFromResponse,
} from "../src/domain/chat";
import type { VoiceApi } from "../src/lib/api-voice";
import { createVoiceClient } from "../src/lib/api-voice-client";

describe("api-voice-client", () => {
  function createSession() {
    return {
      accessToken: accessTokenFromInput("A".repeat(64)),
      refreshToken: refreshTokenFromInput("B".repeat(64)),
      expiresAtUnix: 2_000_000_000,
    };
  }

  function createVoiceApiStub(overrides?: Partial<VoiceApi>): VoiceApi {
    const api: VoiceApi = {
      issueVoiceToken: vi.fn(async () =>
        voiceTokenFromResponse({
          token: "tok_valid_123",
          livekit_url: "wss://livekit.example.test",
          room: "guild-room",
          identity: "member-identity",
          can_publish: true,
          can_subscribe: false,
          publish_sources: ["microphone"],
          expires_in_secs: 300,
        }),
      ),
    };

    return {
      ...api,
      ...overrides,
    };
  }

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("delegates issueVoiceToken through voice API", async () => {
    const expected = voiceTokenFromResponse({
      token: "tok_valid_123",
      livekit_url: "wss://livekit.example.test",
      room: "guild-room",
      identity: "member-identity",
      can_publish: true,
      can_subscribe: false,
      publish_sources: ["microphone"],
      expires_in_secs: 300,
    });
    const issueVoiceToken = vi.fn(async () => expected);
    const voiceClient = createVoiceClient({
      voiceApi: createVoiceApiStub({ issueVoiceToken }),
    });
    const session = createSession();
    const guildId = guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV");
    const channelId = channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB0");

    await expect(
      voiceClient.issueVoiceToken(session, guildId, channelId, {
        canPublish: true,
        canSubscribe: false,
        publishSources: ["microphone"],
      }),
    ).resolves.toBe(expected);

    expect(issueVoiceToken).toHaveBeenCalledWith(session, guildId, channelId, {
      canPublish: true,
      canSubscribe: false,
      publishSources: ["microphone"],
    });
  });

  it("returns upstream voice token value unchanged", async () => {
    const expected = voiceTokenFromResponse({
      token: "tok_valid_456",
      livekit_url: "wss://livekit-alt.example.test",
      room: "another-room",
      identity: "another-identity",
      can_publish: false,
      can_subscribe: true,
      publish_sources: [],
      expires_in_secs: 600,
    });
    const voiceClient = createVoiceClient({
      voiceApi: createVoiceApiStub({
        issueVoiceToken: vi.fn(async () => expected),
      }),
    });
    const session = createSession();

    await expect(
      voiceClient.issueVoiceToken(
        session,
        guildIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FAV"),
        channelIdFromInput("01ARZ3NDEKTSV4RRFFQ69G5FB0"),
        {},
      ),
    ).resolves.toBe(expected);
  });
});