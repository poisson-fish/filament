import {
  dispatchProfileGatewayEvent,
} from "../src/lib/gateway-profile-dispatch";

const DEFAULT_USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";

describe("dispatchProfileGatewayEvent", () => {
  it("dispatches decoded profile events to matching handlers", () => {
    const onProfileAvatarUpdate = vi.fn();
    const onProfileBannerUpdate = vi.fn();

    const handled = dispatchProfileGatewayEvent(
      "profile_avatar_update",
      {
        user_id: DEFAULT_USER_ID,
        avatar_version: 4,
        updated_at_unix: 1710000010,
      },
      { onProfileAvatarUpdate, onProfileBannerUpdate },
    );

    expect(handled).toBe(true);
    expect(onProfileAvatarUpdate).toHaveBeenCalledTimes(1);
    expect(onProfileBannerUpdate).not.toHaveBeenCalled();
    expect(onProfileAvatarUpdate).toHaveBeenCalledWith({
      userId: DEFAULT_USER_ID,
      avatarVersion: 4,
      updatedAtUnix: 1710000010,
    });
  });

  it("dispatches banner update events", () => {
    const onProfileBannerUpdate = vi.fn();

    const handled = dispatchProfileGatewayEvent(
      "profile_banner_update",
      {
        user_id: DEFAULT_USER_ID,
        banner_version: 6,
        updated_at_unix: 1710000012,
      },
      { onProfileBannerUpdate },
    );

    expect(handled).toBe(true);
    expect(onProfileBannerUpdate).toHaveBeenCalledTimes(1);
    expect(onProfileBannerUpdate).toHaveBeenCalledWith({
      userId: DEFAULT_USER_ID,
      bannerVersion: 6,
      updatedAtUnix: 1710000012,
    });
  });

  it("fails closed for known profile types with invalid payloads", () => {
    const onProfileUpdate = vi.fn();

    const handled = dispatchProfileGatewayEvent(
      "profile_update",
      {
        user_id: DEFAULT_USER_ID,
        updated_fields: {},
        updated_at_unix: 1710000011,
      },
      { onProfileUpdate },
    );

    expect(handled).toBe(true);
    expect(onProfileUpdate).not.toHaveBeenCalled();
  });

  it("returns false for non-profile event types", () => {
    const onProfileAvatarUpdate = vi.fn();

    const handled = dispatchProfileGatewayEvent(
      "message_create",
      {},
      { onProfileAvatarUpdate },
    );

    expect(handled).toBe(false);
    expect(onProfileAvatarUpdate).not.toHaveBeenCalled();
  });
});
