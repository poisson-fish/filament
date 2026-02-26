import {
  decodeProfileGatewayEvent,
} from "../src/lib/gateway-profile-events";

const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";

describe("decodeProfileGatewayEvent", () => {
  it("decodes valid profile_update payload", () => {
    const result = decodeProfileGatewayEvent("profile_update", {
      user_id: DEFAULT_USER_ID,
      updated_fields: {
        username: "updated-user",
        about_markdown: "About me",
        about_markdown_tokens: [
          {
            type: "text",
            text: "About me",
          },
        ],
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toEqual({
      type: "profile_update",
      payload: {
        userId: DEFAULT_USER_ID,
        updatedFields: {
          username: "updated-user",
          aboutMarkdown: "About me",
          aboutMarkdownTokens: [
            {
              type: "text",
              text: "About me",
            },
          ],
        },
        updatedAtUnix: 1710000001,
      },
    });
  });

  it("fails closed when about tokens are provided without about markdown", () => {
    const result = decodeProfileGatewayEvent("profile_update", {
      user_id: DEFAULT_USER_ID,
      updated_fields: {
        about_markdown_tokens: [
          {
            type: "text",
            text: "About me",
          },
        ],
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("fails closed for invalid profile_avatar_update payload", () => {
    const result = decodeProfileGatewayEvent("profile_avatar_update", {
      user_id: "bad-user-id",
      avatar_version: 2,
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("decodes valid profile_banner_update payload", () => {
    const result = decodeProfileGatewayEvent("profile_banner_update", {
      user_id: DEFAULT_USER_ID,
      banner_version: 7,
      updated_at_unix: 1710000002,
    });

    expect(result).toEqual({
      type: "profile_banner_update",
      payload: {
        userId: DEFAULT_USER_ID,
        bannerVersion: 7,
        updatedAtUnix: 1710000002,
      },
    });
  });

  it("returns null for unknown event type", () => {
    const result = decodeProfileGatewayEvent("profile_unknown", {
      user_id: DEFAULT_USER_ID,
      updated_fields: {
        username: "updated-user",
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });

  it("fails closed for hostile prototype event type", () => {
    const result = decodeProfileGatewayEvent("__proto__", {
      user_id: DEFAULT_USER_ID,
      updated_fields: {
        username: "updated-user",
      },
      updated_at_unix: 1710000001,
    });

    expect(result).toBeNull();
  });
});
