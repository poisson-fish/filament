import type { RtcSnapshot } from "../../../lib/rtc";

export const ADD_REACTION_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Edit/Add_Plus_Circle.svg",
  import.meta.url,
).href;
export const EDIT_MESSAGE_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Edit/Edit_Pencil_Line_01.svg",
  import.meta.url,
).href;
export const DELETE_MESSAGE_ICON_URL = new URL(
  "../../../../resource/coolicons.v4.1/cooliocns SVG/Interface/Trash_Full.svg",
  import.meta.url,
).href;

export const MESSAGE_AUTOLOAD_TOP_THRESHOLD_PX = 120;
export const MESSAGE_LOAD_OLDER_BUTTON_TOP_THRESHOLD_PX = 340;
export const MESSAGE_STICKY_BOTTOM_THRESHOLD_PX = 140;

export const REACTION_PICKER_OVERLAY_GAP_PX = 8;
export const REACTION_PICKER_OVERLAY_MARGIN_PX = 8;
export const REACTION_PICKER_OVERLAY_MAX_WIDTH_PX = 368;
export const REACTION_PICKER_OVERLAY_ESTIMATED_HEIGHT_PX = 252;

export const RTC_DISCONNECTED_SNAPSHOT: RtcSnapshot = {
  connectionStatus: "disconnected",
  localParticipantIdentity: null,
  isMicrophoneEnabled: false,
  isCameraEnabled: false,
  isScreenShareEnabled: false,
  participants: [],
  videoTracks: [],
  activeSpeakerIdentities: [],
  lastErrorCode: null,
  lastErrorMessage: null,
};
