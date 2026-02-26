import {
  markdownTokensFromResponse,
  profileAboutFromInput,
  userIdFromInput,
} from "../domain/chat";
import type {
  ProfileAvatarUpdatePayload,
  ProfileBannerUpdatePayload,
  ProfileUpdatePayload,
} from "./gateway-contracts";

type ProfileGatewayEvent =
  | {
      type: "profile_update";
      payload: ProfileUpdatePayload;
    }
  | {
      type: "profile_avatar_update";
      payload: ProfileAvatarUpdatePayload;
    }
  | {
      type: "profile_banner_update";
      payload: ProfileBannerUpdatePayload;
    };

type ProfileGatewayEventType = ProfileGatewayEvent["type"];
type ProfileEventDecoder<TPayload> = (payload: unknown) => TPayload | null;

function parseProfileUpdatePayload(payload: unknown): ProfileUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.user_id !== "string" ||
    !value.updated_fields ||
    typeof value.updated_fields !== "object" ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let userId: string;
  try {
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }

  const updatedFieldsDto = value.updated_fields as Record<string, unknown>;
  let username: string | undefined;
  let aboutMarkdown: string | undefined;
  let aboutMarkdownTokens: ProfileUpdatePayload["updatedFields"]["aboutMarkdownTokens"];
  if (typeof updatedFieldsDto.username !== "undefined") {
    if (
      typeof updatedFieldsDto.username !== "string" ||
      updatedFieldsDto.username.length === 0 ||
      updatedFieldsDto.username.length > 64
    ) {
      return null;
    }
    username = updatedFieldsDto.username;
  }
  if (typeof updatedFieldsDto.about_markdown !== "undefined") {
    if (typeof updatedFieldsDto.about_markdown !== "string") {
      return null;
    }
    try {
      aboutMarkdown = profileAboutFromInput(updatedFieldsDto.about_markdown);
    } catch {
      return null;
    }
  }
  if (typeof updatedFieldsDto.about_markdown_tokens !== "undefined") {
    try {
      aboutMarkdownTokens = markdownTokensFromResponse(
        updatedFieldsDto.about_markdown_tokens,
      );
    } catch {
      return null;
    }
  }
  if (
    typeof username === "undefined" &&
    typeof aboutMarkdown === "undefined" &&
    typeof aboutMarkdownTokens === "undefined"
  ) {
    return null;
  }
  if (
    typeof aboutMarkdownTokens !== "undefined" &&
    typeof aboutMarkdown === "undefined"
  ) {
    return null;
  }

  return {
    userId,
    updatedFields: {
      username,
      aboutMarkdown,
      aboutMarkdownTokens,
    },
    updatedAtUnix: value.updated_at_unix,
  };
}

function parseProfileAvatarUpdatePayload(
  payload: unknown,
): ProfileAvatarUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.user_id !== "string" ||
    typeof value.avatar_version !== "number" ||
    !Number.isSafeInteger(value.avatar_version) ||
    value.avatar_version < 0 ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let userId: string;
  try {
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }

  return {
    userId,
    avatarVersion: value.avatar_version,
    updatedAtUnix: value.updated_at_unix,
  };
}

function parseProfileBannerUpdatePayload(
  payload: unknown,
): ProfileBannerUpdatePayload | null {
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const value = payload as Record<string, unknown>;
  if (
    typeof value.user_id !== "string" ||
    typeof value.banner_version !== "number" ||
    !Number.isSafeInteger(value.banner_version) ||
    value.banner_version < 0 ||
    typeof value.updated_at_unix !== "number" ||
    !Number.isSafeInteger(value.updated_at_unix) ||
    value.updated_at_unix < 1
  ) {
    return null;
  }

  let userId: string;
  try {
    userId = userIdFromInput(value.user_id);
  } catch {
    return null;
  }

  return {
    userId,
    bannerVersion: value.banner_version,
    updatedAtUnix: value.updated_at_unix,
  };
}

const PROFILE_EVENT_DECODERS: {
  [K in ProfileGatewayEventType]: ProfileEventDecoder<Extract<ProfileGatewayEvent, { type: K }>["payload"]>;
} = {
  profile_update: parseProfileUpdatePayload,
  profile_avatar_update: parseProfileAvatarUpdatePayload,
  profile_banner_update: parseProfileBannerUpdatePayload,
};

function isProfileGatewayEventType(value: string): value is ProfileGatewayEventType {
  return Object.prototype.hasOwnProperty.call(PROFILE_EVENT_DECODERS, value);
}

export function decodeProfileGatewayEvent(
  type: string,
  payload: unknown,
): ProfileGatewayEvent | null {
  if (!isProfileGatewayEventType(type)) {
    return null;
  }

  if (type === "profile_update") {
    const parsedPayload = PROFILE_EVENT_DECODERS.profile_update(payload);
    if (!parsedPayload) {
      return null;
    }
    return {
      type,
      payload: parsedPayload,
    };
  }

  if (type === "profile_avatar_update") {
    const parsedPayload = PROFILE_EVENT_DECODERS.profile_avatar_update(payload);
    if (!parsedPayload) {
      return null;
    }
    return {
      type,
      payload: parsedPayload,
    };
  }

  const parsedPayload = PROFILE_EVENT_DECODERS.profile_banner_update(payload);
  if (!parsedPayload) {
    return null;
  }
  return {
    type,
    payload: parsedPayload,
  };
}
