import { createRoot, createSignal } from "solid-js";
import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import { createNativeEncryptionController } from "../src/features/app-shell/controllers/native-encryption-controller";
import {
  NATIVE_ROTATE_IDENTITY_CONFIRMATION,
  NativeClientError,
  type NativeEncryptionSettings,
} from "../src/lib/native-client";

const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3_600,
});
const SETTINGS: NativeEncryptionSettings = {
  ready: true,
  safetyNumber: "ab".repeat(16),
  rotationSequence: 2,
  devices: [
    {
      deviceId: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
      addedAtUnix: 1_700_000_000,
      isCurrentDevice: true,
      verification: "verified",
    },
  ],
  backupEnrolled: false,
};

async function settle(): Promise<void> {
  await Promise.resolve();
  await Promise.resolve();
  await Promise.resolve();
}

describe("native encryption controller", () => {
  it("initializes custody and exposes only the validated settings snapshot", async () => {
    const storeSession = vi.fn(async () => undefined);
    const initializeE2eeStore = vi.fn(async () => undefined);
    const readEncryptionSettings = vi.fn(async () => SETTINGS);

    await new Promise<void>((resolve) => {
      createRoot((dispose) => {
        const [session] = createSignal(SESSION);
        const controller = createNativeEncryptionController(
          { session },
          {
            bridge: {
              isAvailable: () => true,
              storeSession,
              initializeE2eeStore,
              readEncryptionSettings,
              rotateRootIdentity: vi.fn(async () => undefined),
            },
          },
        );
        void settle().then(() => {
          expect(storeSession).toHaveBeenCalledWith(SESSION);
          expect(initializeE2eeStore).toHaveBeenCalledOnce();
          expect(controller.settings()).toEqual(SETTINGS);
          expect(controller.error()).toBe("");
          dispose();
          resolve();
        });
      });
    });
  });

  it("gates rotation on exact confirmation and refreshes public state", async () => {
    const rotated = { ...SETTINGS, rotationSequence: 3 };
    const readEncryptionSettings = vi
      .fn<() => Promise<NativeEncryptionSettings>>()
      .mockResolvedValueOnce(SETTINGS)
      .mockResolvedValueOnce(rotated);
    const rotateRootIdentity = vi.fn(async () => undefined);

    await new Promise<void>((resolve) => {
      createRoot((dispose) => {
        const [session] = createSignal(SESSION);
        const controller = createNativeEncryptionController(
          { session },
          {
            bridge: {
              isAvailable: () => true,
              storeSession: vi.fn(async () => undefined),
              initializeE2eeStore: vi.fn(async () => undefined),
              readEncryptionSettings,
              rotateRootIdentity,
            },
          },
        );
        void settle().then(async () => {
          controller.setRotationConfirmation("ROTATE");
          await controller.rotateIdentity();
          expect(rotateRootIdentity).not.toHaveBeenCalled();

          controller.setRotationConfirmation(NATIVE_ROTATE_IDENTITY_CONFIRMATION);
          await controller.rotateIdentity();
          expect(rotateRootIdentity).toHaveBeenCalledWith(
            NATIVE_ROTATE_IDENTITY_CONFIRMATION,
          );
          expect(controller.settings()?.rotationSequence).toBe(3);
          expect(controller.rotationConfirmation()).toBe("");
          expect(controller.status()).toContain("Other devices were revoked");
          dispose();
          resolve();
        });
      });
    });
  });

  it("maps native rejection to a fixed pairing state", async () => {
    await new Promise<void>((resolve) => {
      createRoot((dispose) => {
        const [session] = createSignal(SESSION);
        const controller = createNativeEncryptionController(
          { session },
          {
            bridge: {
              isAvailable: () => true,
              storeSession: vi.fn(async () => undefined),
              initializeE2eeStore: vi.fn(async () => {
                throw new NativeClientError("rejected");
              }),
              readEncryptionSettings: vi.fn(async () => SETTINGS),
              rotateRootIdentity: vi.fn(async () => undefined),
            },
          },
        );
        void settle().then(() => {
          expect(controller.settings()).toBeNull();
          expect(controller.error()).toContain("Pair or repair this client");
          dispose();
          resolve();
        });
      });
    });
  });
});
