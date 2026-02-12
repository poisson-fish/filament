import { vi } from "vitest";
import {
  VOICE_DEVICE_SETTINGS_STORAGE_KEY,
  defaultVoiceDevicePreferences,
  enumerateAudioDevices,
  loadVoiceDevicePreferences,
  type MediaDeviceId,
  reconcileVoiceDevicePreferences,
  saveVoiceDevicePreferences,
} from "../src/lib/voice-device-settings";

function createStorageMock(): Storage {
  const store = new Map<string, string>();
  return {
    get length() {
      return store.size;
    },
    clear() {
      store.clear();
    },
    getItem(key: string) {
      return store.has(key) ? store.get(key)! : null;
    },
    key(index: number) {
      return [...store.keys()][index] ?? null;
    },
    removeItem(key: string) {
      store.delete(key);
    },
    setItem(key: string, value: string) {
      store.set(key, value);
    },
  };
}

describe("voice device settings", () => {
  beforeEach(() => {
    const storage = createStorageMock();
    vi.stubGlobal("localStorage", storage);
    Object.defineProperty(window, "localStorage", {
      value: storage,
      configurable: true,
    });
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it("loads safe defaults for invalid storage payloads", () => {
    window.localStorage.setItem(VOICE_DEVICE_SETTINGS_STORAGE_KEY, "{\"bad\":true}");
    expect(loadVoiceDevicePreferences()).toEqual(defaultVoiceDevicePreferences());

    window.localStorage.setItem(
      VOICE_DEVICE_SETTINGS_STORAGE_KEY,
      JSON.stringify({
        audioInputDeviceId: "\n",
        audioOutputDeviceId: "speaker",
      }),
    );
    expect(loadVoiceDevicePreferences()).toEqual(defaultVoiceDevicePreferences());
  });

  it("saves and reloads selected device IDs", () => {
    saveVoiceDevicePreferences({
      audioInputDeviceId: "mic-01" as MediaDeviceId,
      audioOutputDeviceId: "spk-07" as MediaDeviceId,
    });
    expect(loadVoiceDevicePreferences()).toEqual({
      audioInputDeviceId: "mic-01",
      audioOutputDeviceId: "spk-07",
    });
  });

  it("enumerates and reconciles device inventory", async () => {
    Object.defineProperty(navigator, "mediaDevices", {
      configurable: true,
      value: {
        enumerateDevices: vi.fn(async () => [
          { kind: "audioinput", deviceId: "mic-a", label: "Desk Mic" },
          { kind: "audiooutput", deviceId: "spk-a", label: "Desk Speaker" },
          { kind: "videoinput", deviceId: "cam-a", label: "Camera" },
        ]),
      },
    });

    const inventory = await enumerateAudioDevices();
    expect(inventory.audioInputs).toEqual([
      {
        kind: "audioinput",
        deviceId: "mic-a",
        label: "Desk Mic",
      },
    ]);
    expect(inventory.audioOutputs).toEqual([
      {
        kind: "audiooutput",
        deviceId: "spk-a",
        label: "Desk Speaker",
      },
    ]);

    expect(
      reconcileVoiceDevicePreferences(
        {
          audioInputDeviceId: "mic-a" as MediaDeviceId,
          audioOutputDeviceId: "missing-spk" as MediaDeviceId,
        },
        inventory,
      ),
    ).toEqual({
      audioInputDeviceId: "mic-a",
      audioOutputDeviceId: null,
    });
  });
});
