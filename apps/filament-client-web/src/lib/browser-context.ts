const LOOPBACK_ALIAS_HOSTNAME = "0.0.0.0";
const LOCALHOST_HOSTNAME = "localhost";

interface BrowserContextSnapshot {
  isSecureContext: boolean | null;
  hostname: string | null;
}

interface BrowserLocationLike {
  href: string;
  replace(url: string): void;
}

interface BrowserWindowLike {
  location: BrowserLocationLike;
}

const GENERIC_MEDIA_CONTEXT_HINT = "Use HTTPS or http://localhost to enable microphone access.";
const LOOPBACK_ALIAS_MEDIA_CONTEXT_HINT =
  "Use http://localhost instead of http://0.0.0.0, or use HTTPS.";

const GENERIC_MICROPHONE_UNAVAILABLE_MESSAGE = "Unable to change microphone state.";
const MICROPHONE_PERMISSION_BLOCKED_MESSAGE =
  "Microphone permission is blocked in browser site settings. Allow access and retry.";

function readBrowserContextSnapshot(): BrowserContextSnapshot {
  if (typeof window === "undefined") {
    return { isSecureContext: null, hostname: null };
  }

  return {
    isSecureContext: typeof window.isSecureContext === "boolean" ? window.isSecureContext : null,
    hostname: typeof window.location?.hostname === "string" ? window.location.hostname : null,
  };
}

export function mediaContextHint(snapshot: BrowserContextSnapshot = readBrowserContextSnapshot()): string | null {
  if (snapshot.isSecureContext !== false) {
    return null;
  }
  if (snapshot.hostname === LOOPBACK_ALIAS_HOSTNAME) {
    return LOOPBACK_ALIAS_MEDIA_CONTEXT_HINT;
  }
  return GENERIC_MEDIA_CONTEXT_HINT;
}

export function audioDeviceEnumerationUnavailableMessage(
  snapshot: BrowserContextSnapshot = readBrowserContextSnapshot(),
): string {
  const hint = mediaContextHint(snapshot);
  if (!hint) {
    return "Audio device enumeration is unavailable in this browser.";
  }
  return `Audio device enumeration is unavailable in this browser context. ${hint}`;
}

export function microphoneToggleUnavailableMessage(
  snapshot: BrowserContextSnapshot = readBrowserContextSnapshot(),
): string {
  const hint = mediaContextHint(snapshot);
  if (!hint) {
    return GENERIC_MICROPHONE_UNAVAILABLE_MESSAGE;
  }
  return `Microphone access is unavailable in this browser context. ${hint}`;
}

export function microphoneToggleErrorMessage(
  rawRtcErrorMessage: string,
  snapshot: BrowserContextSnapshot = readBrowserContextSnapshot(),
): string {
  const normalized = rawRtcErrorMessage.toLowerCase();
  if (
    normalized.includes("notallowed") ||
    normalized.includes("permission denied") ||
    normalized.includes("permission blocked") ||
    normalized.includes("denied permission") ||
    normalized.includes("securityerror")
  ) {
    return MICROPHONE_PERMISSION_BLOCKED_MESSAGE;
  }
  return microphoneToggleUnavailableMessage(snapshot);
}

export function loopbackAliasRedirectUrl(current: URL): URL | null {
  if (current.hostname !== LOOPBACK_ALIAS_HOSTNAME) {
    return null;
  }
  if (current.protocol !== "http:" && current.protocol !== "https:") {
    return null;
  }
  const redirected = new URL(current.toString());
  redirected.hostname = LOCALHOST_HOSTNAME;
  return redirected;
}

export function redirectLoopbackAliasInDev(
  browserWindow: BrowserWindowLike | null | undefined,
  isDev: boolean,
): boolean {
  if (!isDev || !browserWindow) {
    return false;
  }
  if (
    !browserWindow.location ||
    typeof browserWindow.location.href !== "string" ||
    typeof browserWindow.location.replace !== "function"
  ) {
    return false;
  }

  let current: URL;
  try {
    current = new URL(browserWindow.location.href);
  } catch {
    return false;
  }

  const redirected = loopbackAliasRedirectUrl(current);
  if (!redirected) {
    return false;
  }
  browserWindow.location.replace(redirected.toString());
  return true;
}
