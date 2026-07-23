import { type Accessor, createEffect, createSignal, on } from "solid-js";
import type { AuthSession } from "../../../domain/auth";
import {
  NATIVE_ROTATE_IDENTITY_CONFIRMATION,
  NativeClientError,
  type NativeClientBridge,
  type NativeEncryptionSettings,
  nativeClientBridge,
} from "../../../lib/native-client";

export interface NativeEncryptionControllerOptions {
  session: Accessor<AuthSession | null>;
}

export interface NativeEncryptionControllerDependencies {
  bridge: Pick<
    NativeClientBridge,
    | "isAvailable"
    | "storeSession"
    | "initializeE2eeStore"
    | "readEncryptionSettings"
    | "rotateRootIdentity"
  >;
}

function mapNativeEncryptionError(error: unknown): string {
  if (!(error instanceof NativeClientError)) {
    return "Native encryption is unavailable.";
  }
  if (error.code === "rejected") {
    return "Native encryption state was rejected. Pair or repair this client before using E2EE.";
  }
  if (error.code === "invalid_response") {
    return "Native encryption returned invalid public metadata.";
  }
  if (error.code === "invalid_request") {
    return "Native encryption rejected the request.";
  }
  return "Platform key custody or encrypted storage is unavailable.";
}

export function createNativeEncryptionController(
  options: NativeEncryptionControllerOptions,
  dependencies: Partial<NativeEncryptionControllerDependencies> = {},
) {
  const bridge = dependencies.bridge ?? nativeClientBridge;
  const [settings, setSettings] = createSignal<NativeEncryptionSettings | null>(null);
  const [error, setError] = createSignal("");
  const [status, setStatus] = createSignal("");
  const [rotationConfirmation, setRotationConfirmation] = createSignal("");
  const [isInitializing, setInitializing] = createSignal(false);
  const [isRotatingIdentity, setRotatingIdentity] = createSignal(false);
  let operationGeneration = 0;

  const load = async (session: AuthSession, generation: number): Promise<void> => {
    setInitializing(true);
    setError("");
    setStatus("");
    try {
      await bridge.storeSession(session);
      await bridge.initializeE2eeStore();
      const snapshot = await bridge.readEncryptionSettings();
      if (generation === operationGeneration) {
        setSettings(snapshot);
      }
    } catch (loadError) {
      if (generation === operationGeneration) {
        setSettings(null);
        setError(mapNativeEncryptionError(loadError));
      }
    } finally {
      if (generation === operationGeneration) {
        setInitializing(false);
      }
    }
  };

  const refresh = async (): Promise<void> => {
    const session = options.session();
    if (!session || !bridge.isAvailable()) {
      return;
    }
    operationGeneration += 1;
    await load(session, operationGeneration);
  };

  const rotateIdentity = async (): Promise<void> => {
    const session = options.session();
    if (
      !session
      || !bridge.isAvailable()
      || isRotatingIdentity()
      || rotationConfirmation() !== NATIVE_ROTATE_IDENTITY_CONFIRMATION
    ) {
      return;
    }
    operationGeneration += 1;
    const generation = operationGeneration;
    setRotatingIdentity(true);
    setError("");
    setStatus("");
    try {
      await bridge.storeSession(session);
      await bridge.rotateRootIdentity(rotationConfirmation());
      const snapshot = await bridge.readEncryptionSettings();
      if (generation === operationGeneration) {
        setSettings(snapshot);
        setRotationConfirmation("");
        setStatus("Root identity rotated. Other devices were revoked.");
      }
    } catch (rotationError) {
      if (generation === operationGeneration) {
        setError(mapNativeEncryptionError(rotationError));
      }
    } finally {
      if (generation === operationGeneration) {
        setRotatingIdentity(false);
      }
    }
  };

  createEffect(
    on(options.session, (session) => {
      operationGeneration += 1;
      const generation = operationGeneration;
      setSettings(null);
      setError("");
      setStatus("");
      setRotationConfirmation("");
      if (session && bridge.isAvailable()) {
        void load(session, generation);
      } else {
        setInitializing(false);
      }
    }),
  );

  return {
    isNativeClient: bridge.isAvailable(),
    settings,
    error,
    status,
    rotationConfirmation,
    setRotationConfirmation,
    isInitializing,
    isRotatingIdentity,
    refresh,
    rotateIdentity,
  };
}
