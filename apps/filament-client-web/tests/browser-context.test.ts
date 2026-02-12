import { describe, expect, it, vi } from "vitest";
import {
  audioDeviceEnumerationUnavailableMessage,
  loopbackAliasRedirectUrl,
  mediaContextHint,
  microphoneToggleErrorMessage,
  microphoneToggleUnavailableMessage,
  redirectLoopbackAliasInDev,
} from "../src/lib/browser-context";

describe("browser context helpers", () => {
  it("returns media hints only for insecure contexts", () => {
    expect(mediaContextHint({ isSecureContext: true, hostname: "localhost" })).toBeNull();
    expect(mediaContextHint({ isSecureContext: false, hostname: "0.0.0.0" })).toBe(
      "Use http://localhost instead of http://0.0.0.0, or use HTTPS.",
    );
    expect(mediaContextHint({ isSecureContext: false, hostname: "192.168.1.10" })).toBe(
      "Use HTTPS or http://localhost to enable microphone access.",
    );
  });

  it("builds capability messages with context-aware guidance", () => {
    expect(
      audioDeviceEnumerationUnavailableMessage({
        isSecureContext: false,
        hostname: "0.0.0.0",
      }),
    ).toBe(
      "Audio device enumeration is unavailable in this browser context. Use http://localhost instead of http://0.0.0.0, or use HTTPS.",
    );
    expect(
      microphoneToggleUnavailableMessage({
        isSecureContext: false,
        hostname: "192.168.1.10",
      }),
    ).toBe(
      "Microphone access is unavailable in this browser context. Use HTTPS or http://localhost to enable microphone access.",
    );
    expect(
      microphoneToggleUnavailableMessage({
        isSecureContext: true,
        hostname: "localhost",
      }),
    ).toBe("Unable to change microphone state.");
  });

  it("maps microphone toggle permission denials to actionable guidance", () => {
    expect(
      microphoneToggleErrorMessage("Unable to update microphone state: NotAllowedError", {
        isSecureContext: true,
        hostname: "localhost",
      }),
    ).toBe("Microphone permission is blocked in browser site settings. Allow access and retry.");
  });

  it("rewrites 0.0.0.0 URLs to localhost", () => {
    const rewritten = loopbackAliasRedirectUrl(
      new URL("http://0.0.0.0:4173/app?tab=voice#panel"),
    );
    expect(rewritten?.toString()).toBe("http://localhost:4173/app?tab=voice#panel");
    expect(loopbackAliasRedirectUrl(new URL("http://localhost:4173/app"))).toBeNull();
  });

  it("redirects loopback alias host in development only", () => {
    const replace = vi.fn();
    const browserWindow = {
      location: {
        href: "http://0.0.0.0:4173/app?tab=voice#panel",
        replace,
      },
    };

    expect(redirectLoopbackAliasInDev(browserWindow, true)).toBe(true);
    expect(replace).toHaveBeenCalledWith("http://localhost:4173/app?tab=voice#panel");
    expect(redirectLoopbackAliasInDev(browserWindow, false)).toBe(false);
  });
});
