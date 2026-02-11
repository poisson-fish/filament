import { describe, expect, it } from "vitest";
import {
  resolveVoiceConnectionTransition,
  resolveVoiceDevicePreferenceStatus,
  unavailableVoiceDeviceError,
} from "../src/features/app-shell/controllers/voice-controller";

describe("app shell voice controller", () => {
  it("emits reconnecting and reconnected transitions", () => {
    expect(
      resolveVoiceConnectionTransition({
        previousStatus: "connected",
        currentStatus: "reconnecting",
        hasConnectedChannel: true,
        isJoining: false,
        isLeaving: false,
      }),
    ).toEqual({
      shouldClearSession: false,
      statusMessage: "Voice reconnecting. Media may recover automatically.",
      errorMessage: "",
    });

    expect(
      resolveVoiceConnectionTransition({
        previousStatus: "reconnecting",
        currentStatus: "connected",
        hasConnectedChannel: true,
        isJoining: false,
        isLeaving: false,
      }),
    ).toEqual({
      shouldClearSession: false,
      statusMessage: "Voice reconnected.",
      errorMessage: "",
    });
  });

  it("forces voice session clear on unexpected disconnect", () => {
    expect(
      resolveVoiceConnectionTransition({
        previousStatus: "connected",
        currentStatus: "disconnected",
        hasConnectedChannel: true,
        isJoining: false,
        isLeaving: false,
      }),
    ).toEqual({
      shouldClearSession: true,
      statusMessage: "",
      errorMessage: "Voice connection dropped. Select Join Voice to reconnect.",
    });
  });

  it("maps voice device status strings for active and inactive sessions", () => {
    expect(resolveVoiceDevicePreferenceStatus("audioinput", false, "mic-1")).toBe(
      "Microphone preference saved for the next voice join.",
    );
    expect(resolveVoiceDevicePreferenceStatus("audiooutput", true, "spk-1")).toBe(
      "Speaker updated for the active voice session.",
    );
    expect(resolveVoiceDevicePreferenceStatus("audioinput", true, null)).toBe(
      "Microphone preference cleared. Current session keeps its current device.",
    );
  });

  it("returns unavailable-device errors by device kind", () => {
    expect(unavailableVoiceDeviceError("audioinput")).toBe(
      "Selected microphone is not available.",
    );
    expect(unavailableVoiceDeviceError("audiooutput")).toBe(
      "Selected speaker is not available.",
    );
  });
});
