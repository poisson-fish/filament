import { invoke, isTauri } from "@tauri-apps/api/core";
import type { AuthSession } from "../domain/auth";

const MAX_NATIVE_DEVICES = 100;
const MAX_POLICY_RECONCILIATIONS = 1_024;
const MAX_UNIX_TIMESTAMP = 253_402_300_799;
const CANONICAL_ULID = /^[0-9A-HJKMNP-TV-Z]{26}$/;
const SAFETY_NUMBER = /^[0-9a-f]{32}$/;

export const NATIVE_ROTATE_IDENTITY_CONFIRMATION = "ROTATE MY IDENTITY";

export type NativeClientErrorCode =
  | "invalid_request"
  | "unavailable"
  | "rejected"
  | "invalid_response";

export class NativeClientError extends Error {
  readonly code: NativeClientErrorCode;

  constructor(code: NativeClientErrorCode) {
    super("Native client operation failed.");
    this.name = "NativeClientError";
    this.code = code;
  }
}

export interface NativeEncryptionSettingsDevice {
  deviceId: string;
  addedAtUnix: number;
  isCurrentDevice: boolean;
  verification: "verified" | "unverified";
}

export interface NativePolicyReconciliation {
  groupId: string;
  deadlineUnix: number;
  state: "pending" | "overdue";
}

export interface NativeEncryptionSettings {
  ready: true;
  safetyNumber: string;
  rotationSequence: number;
  devices: NativeEncryptionSettingsDevice[];
  backupEnrolled: boolean;
  policyReconciliations: NativePolicyReconciliation[];
}

type InvokeCommand = (
  command: string,
  args?: Record<string, unknown>,
) => Promise<unknown>;

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  return actual.length === expected.length
    && actual.every((key, index) => key === expected[index]);
}

function isSafeUnixTimestamp(value: unknown, allowZero = true): value is number {
  return Number.isSafeInteger(value)
    && typeof value === "number"
    && value >= (allowZero ? 0 : 1)
    && value <= MAX_UNIX_TIMESTAMP;
}

function isCanonicalUlid(value: unknown): value is string {
  return typeof value === "string" && CANONICAL_ULID.test(value);
}

function isByteArray(value: unknown, length: number): boolean {
  return Array.isArray(value)
    && value.length === length
    && value.every((byte) => Number.isInteger(byte) && byte >= 0 && byte <= 255);
}

function parseSessionMetadata(value: unknown, expectedExpiry: number): void {
  if (
    !isRecord(value)
    || !hasExactKeys(value, ["stored", "expires_at_unix"])
    || value.stored !== true
    || value.expires_at_unix !== expectedExpiry
  ) {
    throw new NativeClientError("invalid_response");
  }
}

function parseStoreStatus(value: unknown): void {
  if (
    !isRecord(value)
    || !hasExactKeys(value, ["ready", "backend", "key_custody"])
    || value.ready !== true
    || value.backend !== "sqlcipher"
    || value.key_custody !== "platform_keystore"
  ) {
    throw new NativeClientError("invalid_response");
  }
}

function parseEncryptionDevice(value: unknown): NativeEncryptionSettingsDevice {
  if (
    !isRecord(value)
    || !hasExactKeys(value, [
      "device_id",
      "added_at_unix",
      "is_current_device",
      "verification",
    ])
    || !isCanonicalUlid(value.device_id)
    || !isSafeUnixTimestamp(value.added_at_unix)
    || typeof value.is_current_device !== "boolean"
    || (value.verification !== "verified" && value.verification !== "unverified")
  ) {
    throw new NativeClientError("invalid_response");
  }
  return {
    deviceId: value.device_id,
    addedAtUnix: value.added_at_unix,
    isCurrentDevice: value.is_current_device,
    verification: value.verification,
  };
}

function parsePolicyReconciliation(value: unknown): NativePolicyReconciliation {
  if (
    !isRecord(value)
    || !hasExactKeys(value, ["group_id", "deadline_unix", "state"])
    || !isCanonicalUlid(value.group_id)
    || !isSafeUnixTimestamp(value.deadline_unix)
    || (value.state !== "pending" && value.state !== "overdue")
  ) {
    throw new NativeClientError("invalid_response");
  }
  return {
    groupId: value.group_id,
    deadlineUnix: value.deadline_unix,
    state: value.state,
  };
}

function parseEncryptionSettings(value: unknown): NativeEncryptionSettings {
  if (
    !isRecord(value)
    || !hasExactKeys(value, [
      "ready",
      "safety_number",
      "rotation_sequence",
      "devices",
      "backup_enrolled",
      "policy_reconciliations",
    ])
    || value.ready !== true
    || typeof value.safety_number !== "string"
    || !SAFETY_NUMBER.test(value.safety_number)
    || !Number.isSafeInteger(value.rotation_sequence)
    || typeof value.rotation_sequence !== "number"
    || value.rotation_sequence < 0
    || !Array.isArray(value.devices)
    || value.devices.length < 1
    || value.devices.length > MAX_NATIVE_DEVICES
    || typeof value.backup_enrolled !== "boolean"
    || !Array.isArray(value.policy_reconciliations)
    || value.policy_reconciliations.length > MAX_POLICY_RECONCILIATIONS
  ) {
    throw new NativeClientError("invalid_response");
  }
  const devices = value.devices.map(parseEncryptionDevice);
  const policyReconciliations = value.policy_reconciliations.map(parsePolicyReconciliation);
  const deviceIds = new Set(devices.map((device) => device.deviceId));
  const reconciliationGroupIds = new Set(
    policyReconciliations.map((reconciliation) => reconciliation.groupId),
  );
  if (
    deviceIds.size !== devices.length
    || devices.filter((device) => device.isCurrentDevice).length !== 1
    || reconciliationGroupIds.size !== policyReconciliations.length
  ) {
    throw new NativeClientError("invalid_response");
  }
  return {
    ready: true,
    safetyNumber: value.safety_number,
    rotationSequence: value.rotation_sequence,
    devices,
    backupEnrolled: value.backup_enrolled,
    policyReconciliations,
  };
}

function parseRotationResponse(value: unknown): void {
  if (
    !isRecord(value)
    || !hasExactKeys(value, [
      "protocol_version",
      "user_id",
      "device_id",
      "rotation_sequence",
      "previous_root_key_pub",
      "new_root_key_pub",
      "revoked_device_count",
      "deleted_keypackage_count",
      "rotated_at_unix",
    ])
    || value.protocol_version !== 1
    || !isCanonicalUlid(value.user_id)
    || !isCanonicalUlid(value.device_id)
    || !Number.isSafeInteger(value.rotation_sequence)
    || typeof value.rotation_sequence !== "number"
    || value.rotation_sequence < 1
    || !isByteArray(value.previous_root_key_pub, 32)
    || !isByteArray(value.new_root_key_pub, 32)
    || !Number.isSafeInteger(value.revoked_device_count)
    || typeof value.revoked_device_count !== "number"
    || value.revoked_device_count < 0
    || value.revoked_device_count > MAX_NATIVE_DEVICES
    || !Number.isSafeInteger(value.deleted_keypackage_count)
    || typeof value.deleted_keypackage_count !== "number"
    || value.deleted_keypackage_count < 0
    || !isSafeUnixTimestamp(value.rotated_at_unix, false)
  ) {
    throw new NativeClientError("invalid_response");
  }
}

function mapInvokeError(error: unknown): NativeClientError {
  if (error instanceof NativeClientError) {
    return error;
  }
  if (error === "InvalidRequest" || error === "invalid_request") {
    return new NativeClientError("invalid_request");
  }
  if (error === "Rejected" || error === "rejected") {
    return new NativeClientError("rejected");
  }
  return new NativeClientError("unavailable");
}

export class NativeClientBridge {
  readonly #invokeCommand: InvokeCommand;
  readonly #isNative: () => boolean;

  constructor(
    invokeCommand: InvokeCommand = invoke,
    nativeDetector: () => boolean = isTauri,
  ) {
    this.#invokeCommand = invokeCommand;
    this.#isNative = nativeDetector;
  }

  isAvailable(): boolean {
    return this.#isNative();
  }

  async storeSession(session: AuthSession): Promise<void> {
    if (!this.isAvailable()) {
      return;
    }
    try {
      const response = await this.#invokeCommand("store_session", {
        request: {
          access_token: session.accessToken,
          refresh_token: session.refreshToken,
          expires_at_unix: session.expiresAtUnix,
        },
      });
      parseSessionMetadata(response, session.expiresAtUnix);
    } catch (error) {
      throw mapInvokeError(error);
    }
  }

  async clearSession(): Promise<void> {
    if (!this.isAvailable()) {
      return;
    }
    try {
      const response = await this.#invokeCommand("clear_session");
      if (response !== null) {
        throw new NativeClientError("invalid_response");
      }
    } catch (error) {
      throw mapInvokeError(error);
    }
  }

  async initializeE2eeStore(): Promise<void> {
    if (!this.isAvailable()) {
      throw new NativeClientError("unavailable");
    }
    try {
      parseStoreStatus(await this.#invokeCommand("initialize_e2ee_store"));
    } catch (error) {
      throw mapInvokeError(error);
    }
  }

  async readEncryptionSettings(): Promise<NativeEncryptionSettings> {
    if (!this.isAvailable()) {
      throw new NativeClientError("unavailable");
    }
    try {
      return parseEncryptionSettings(
        await this.#invokeCommand("read_encryption_settings"),
      );
    } catch (error) {
      throw mapInvokeError(error);
    }
  }

  async rotateRootIdentity(confirmation: string): Promise<void> {
    if (!this.isAvailable()) {
      throw new NativeClientError("unavailable");
    }
    if (confirmation !== NATIVE_ROTATE_IDENTITY_CONFIRMATION) {
      throw new NativeClientError("invalid_request");
    }
    try {
      parseRotationResponse(
        await this.#invokeCommand("rotate_root_identity", {
          request: { confirmation },
        }),
      );
    } catch (error) {
      throw mapInvokeError(error);
    }
  }
}

export const nativeClientBridge = new NativeClientBridge();
