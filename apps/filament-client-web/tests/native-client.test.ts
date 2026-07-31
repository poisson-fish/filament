import { describe, expect, it, vi } from "vitest";
import { authSessionFromResponse } from "../src/domain/auth";
import {
  NATIVE_ROTATE_IDENTITY_CONFIRMATION,
  NativeClientBridge,
  NativeClientError,
} from "../src/lib/native-client";

const USER_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const DEVICE_ID = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
const SESSION = authSessionFromResponse({
  access_token: "A".repeat(64),
  refresh_token: "B".repeat(64),
  expires_in_secs: 3_600,
});

function commandFixture(command: string): unknown {
  if (command === "store_session") {
    return { stored: true, expires_at_unix: SESSION.expiresAtUnix };
  }
  if (command === "clear_session") {
    return null;
  }
  if (command === "initialize_e2ee_store") {
    return {
      ready: true,
      backend: "sqlcipher",
      key_custody: "platform_keystore",
    };
  }
  if (command === "read_encryption_settings") {
    return {
      ready: true,
      safety_number: "ab".repeat(16),
      rotation_sequence: 4,
      devices: [
        {
          device_id: DEVICE_ID,
          added_at_unix: 1_700_000_000,
          is_current_device: true,
          verification: "verified",
        },
      ],
      backup_enrolled: false,
      policy_reconciliations: [
        {
          group_id: "01ARZ3NDEKTSV4RRFFQ69G5FAX",
          deadline_unix: 1_700_000_100,
          state: "overdue",
        },
      ],
    };
  }
  if (command === "rotate_root_identity") {
    return {
      protocol_version: 1,
      user_id: USER_ID,
      device_id: DEVICE_ID,
      rotation_sequence: 5,
      previous_root_key_pub: Array(32).fill(17),
      new_root_key_pub: Array(32).fill(23),
      revoked_device_count: 1,
      deleted_keypackage_count: 8,
      rotated_at_unix: 1_700_000_100,
    };
  }
  throw new Error("unexpected command");
}

describe("packaged native client bridge", () => {
  it("uses only the audited commands and strictly maps public settings", async () => {
    const invoke = vi.fn(
      async (command: string, _args?: Record<string, unknown>) =>
        commandFixture(command),
    );
    const bridge = new NativeClientBridge(invoke, () => true);

    await bridge.storeSession(SESSION);
    await bridge.initializeE2eeStore();
    expect(await bridge.readEncryptionSettings()).toEqual({
      ready: true,
      safetyNumber: "ab".repeat(16),
      rotationSequence: 4,
      devices: [
        {
          deviceId: DEVICE_ID,
          addedAtUnix: 1_700_000_000,
          isCurrentDevice: true,
          verification: "verified",
        },
      ],
      backupEnrolled: false,
      policyReconciliations: [
        {
          groupId: "01ARZ3NDEKTSV4RRFFQ69G5FAX",
          deadlineUnix: 1_700_000_100,
          state: "overdue",
        },
      ],
    });
    await bridge.rotateRootIdentity(NATIVE_ROTATE_IDENTITY_CONFIRMATION);
    await bridge.clearSession();

    expect(invoke.mock.calls.map(([command]) => command)).toEqual([
      "store_session",
      "initialize_e2ee_store",
      "read_encryption_settings",
      "rotate_root_identity",
      "clear_session",
    ]);
    expect(invoke.mock.calls[0]?.[1]).toEqual({
      request: {
        access_token: SESSION.accessToken,
        refresh_token: SESSION.refreshToken,
        expires_at_unix: SESSION.expiresAtUnix,
      },
    });
    expect(invoke.mock.calls[3]?.[1]).toEqual({
      request: { confirmation: NATIVE_ROTATE_IDENTITY_CONFIRMATION },
    });
  });

  it("rejects extra fields, duplicate devices, and unexpected current-device counts", async () => {
    const base = commandFixture("read_encryption_settings") as Record<string, unknown>;
    const extraBridge = new NativeClientBridge(
      vi.fn(async () => ({ ...base, root_secret: "hostile" })),
      () => true,
    );
    await expect(extraBridge.readEncryptionSettings()).rejects.toMatchObject({
      code: "invalid_response",
    });

    const device = (base.devices as unknown[])[0] as Record<string, unknown>;
    const duplicateBridge = new NativeClientBridge(
      vi.fn(async () => ({ ...base, devices: [device, { ...device }] })),
      () => true,
    );
    await expect(duplicateBridge.readEncryptionSettings()).rejects.toMatchObject({
      code: "invalid_response",
    });

    const noCurrentBridge = new NativeClientBridge(
      vi.fn(async () => ({
        ...base,
        devices: [{ ...device, is_current_device: false }],
      })),
      () => true,
    );
    await expect(noCurrentBridge.readEncryptionSettings()).rejects.toMatchObject({
      code: "invalid_response",
    });

    const reconciliation = (base.policy_reconciliations as unknown[])[0] as Record<string, unknown>;
    const duplicateReconciliationBridge = new NativeClientBridge(
      vi.fn(async () => ({
        ...base,
        policy_reconciliations: [reconciliation, { ...reconciliation }],
      })),
      () => true,
    );
    await expect(duplicateReconciliationBridge.readEncryptionSettings()).rejects.toMatchObject({
      code: "invalid_response",
    });

    const invalidReconciliationBridge = new NativeClientBridge(
      vi.fn(async () => ({
        ...base,
        policy_reconciliations: [{ ...reconciliation, state: "complete" }],
      })),
      () => true,
    );
    await expect(invalidReconciliationBridge.readEncryptionSettings()).rejects.toMatchObject({
      code: "invalid_response",
    });
  });

  it("fails before IPC for an inexact destructive confirmation", async () => {
    const invoke = vi.fn(async () => commandFixture("rotate_root_identity"));
    const bridge = new NativeClientBridge(invoke, () => true);

    await expect(bridge.rotateRootIdentity("ROTATE")).rejects.toEqual(
      new NativeClientError("invalid_request"),
    );
    expect(invoke).not.toHaveBeenCalled();
  });

  it("is a no-op for session custody in an ordinary browser", async () => {
    const invoke = vi.fn();
    const bridge = new NativeClientBridge(invoke, () => false);

    await bridge.storeSession(SESSION);
    await bridge.clearSession();
    expect(invoke).not.toHaveBeenCalled();
    await expect(bridge.readEncryptionSettings()).rejects.toMatchObject({
      code: "unavailable",
    });
  });

  it("maps native failures to fixed codes without reflecting attacker text", async () => {
    const bridge = new NativeClientBridge(
      vi.fn(async () => {
        throw "credential backend leaked /secret/path";
      }),
      () => true,
    );
    const error = await bridge.readEncryptionSettings().catch((caught) => caught);
    expect(error).toBeInstanceOf(NativeClientError);
    expect(error).toMatchObject({ code: "unavailable" });
    expect(String(error)).not.toContain("/secret/path");
  });
});
