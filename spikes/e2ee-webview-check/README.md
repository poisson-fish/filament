# Spike: Insertable-Streams / RTCRtpScriptTransform WebView Verification

**Phase:** 0 (Engineering Spike)
**Status:** Documentation-only — actual verification requires per-platform testing
**Relates to:** Phase 5 (Voice/Video E2EE via SFrame)

## Purpose

This spike documents the expected availability of insertable streams / `RTCRtpScriptTransform` across the webview runtimes Filament targets. This directly informs the Phase 5 media E2EE path: SFrame media encryption keyed from MLS exporter secrets requires the ability to intercept and transform RTP frames before they hit the network stack.

## What Are Insertable Streams?

Insertable streams (W3C WebRTC "Insertable Media Using Streams" proposal, now partially folded into WebCodecs and the WebRTC Transceiver API) allow JavaScript to insert custom processing into the WebRTC media pipeline — specifically, to transform RTP frames before encode/after decode. The primary API surface is:

- **`RTCRtpScriptTransform`** — a worker-based transform that processes RTP frames using a `TransformStream`. This is the modern, standardized approach (replacing the older `RTCRtpSender.insertableStream` / `RTCRtpReceiver.insertableStream` API which was deprecated).
- **`RTCRtpTransform`** — the newer unified transform interface being standardized in the W3C WebRTC Working Group.

With insertable streams, the application code can:
1. Intercept outgoing video/audio frames before they are encrypted by the WebRTC DTLS-SRTP layer.
2. Apply SFrame encryption (AEAD over the frame payload).
3. Pass the encrypted frame back to the WebRTC stack for transport.
4. On the receive side, intercept incoming frames, decrypt them, and pass plaintext back to the decoder.

This is the mechanism Discord's DAVE protocol uses for E2EE calls in their Electron-based desktop client (which uses a Chromium WebView2-equivalent engine).

## Why They Matter for SFrame Media E2EE

Filament's voice/video E2EE design (PLAN_E2EE.md §"Voice/Video E2EE Direction") specifies:

- SFrame over insertable streams.
- The SFU (LiveKit) forwards opaque encrypted frames and cannot decrypt media.
- Media keys are derived from the MLS group's `exporter_secret`; media epoch == MLS epoch.
- Rekey on participant join/leave and periodic update commits.

Without insertable streams support, the webview cannot apply SFrame encryption to RTP frames. In that case, the required fallback is a **native WebRTC media path in the host layer** — the Rust/Tauri host process handles media capture, SFrame encryption, and LiveKit connection directly, bypassing the webview's WebRTC stack entirely. **Shipping unencrypted media is never the fallback.**

## Expected Support Matrix

| Platform | WebView Runtime | Insertable Streams Support | Notes |
|---|---|---|---|
| **Windows** | WebView2 (Chromium-based) | **Expected: Supported** | WebView2 ships a recent Chromium engine. Insertable streams / `RTCRtpScriptTransform` have been available in Chromium since ~version 104. Discord's DAVE protocol works in Electron (same Chromium base). **Action:** Verify on target WebView2 runtime version. |
| **macOS** | WKWebView (WebKit) | **Needs verification** | WKWebView uses Apple's WebKit, not Chromium. WebKit's WebRTC implementation has historically lagged behind Chromium on experimental API surface. `RTCRtpScriptTransform` may not be available in all macOS/iOS versions. **Action:** Test on target macOS version with current WKWebView. If unavailable, use native WebRTC media path. |
| **Linux** | WebKitGTK | **Needs verification** | WebKitGTK is the Linux webview engine (used by Tauri on Linux). Its WebRTC support has historically been the weakest of the three. `RTCRtpScriptTransform` may not be available. **Action:** Test on target Linux distributions with current WebKitGTK. If unavailable, use native WebRTC media path. |

## Verification Procedure (Phase 5)

Actual verification must be performed on each target platform:

1. **WebView2 (Windows):**
   - Create a Tauri app on Windows with a WebView2 instance.
   - Attempt to create an `RTCRtpScriptTransform` and attach it to an `RTCRtpSender`.
   - Verify that frames can be intercepted and transformed.
   - Record the WebView2 runtime version and Chromium engine version.

2. **WKWebView (macOS):**
   - Create a Tauri app on macOS with a WKWebView instance.
   - Attempt the same `RTCRtpScriptTransform` creation and frame interception.
   - If `RTCRtpScriptTransform` is unavailable, check for the older `insertableStream` API.
   - If neither is available, document the native WebRTC fallback as required for macOS.

3. **WebKitGTK (Linux):**
   - Create a Tauri app on Linux with a WebKitGTK instance.
   - Attempt the same `RTCRtpScriptTransform` creation and frame interception.
   - If unavailable, document the native WebRTC fallback as required for Linux.

## Fallback: Native WebRTC Media Path

When a webview lacks insertable streams support, the required fallback is a **native WebRTC media path in the host layer**:

- The Tauri Rust host process handles:
  - Media capture (microphone, camera, screen share) via native APIs.
  - SFrame encryption of RTP frames using the MLS exporter secret.
  - LiveKit connection via the Rust LiveKit client SDK.
- The webview's WebRTC stack is not used for E2EE calls.
- The webview handles UI only (call controls, participant display, encryption indicators).
- Communication between the webview and the native media path is over the same typed IPC surface used for all E2EE operations.

This fallback is architecturally consistent with the packaged-client design: crypto operations (including SFrame media encryption) run in the Rust host process, and the webview handles UI only. Key material never enters the JS heap regardless of which media path is used.

**Key constraint:** Shipping unencrypted media is never an acceptable fallback. If neither insertable streams nor a native WebRTC media path can be made to work on a platform, E2EE calls are not supported on that platform — not even as a degraded mode.

## Recommendations

1. **Phase 5 gate:** The insertable-streams verification matrix must be complete before media E2EE ships on any platform. This is an explicit exit criterion for Phase 5.
2. **Prefer native media path on weak platforms:** If WKWebView or WebKitGTK lacks support, implement the native WebRTC media path rather than blocking the platform entirely.
3. **Chromium version tracking:** Track WebView2's Chromium engine version to ensure insertable streams support remains available as Microsoft updates the runtime.
4. **Discord DAVE precedent:** Discord's DAVE protocol proves MLS-keyed SFrame E2EE works in a Chromium-based desktop client. This is strong evidence that the Windows path will work. The macOS/Linux paths are the real unknowns.
5. **Future deprecation risk:** The W3C is standardizing `RTCRtpScriptTransform` but the API surface is still evolving. Pin to specific API versions and track standards changes.

## References

- [W3C: Insertable Media Using Streams](https://w3c.github.io/webrtc-insertable-media/)
- [W3C: WebRTC Encoded Transform](https://w3c.github.io/webrtc-encoded-transform/)
- [RFC 9420: MLS (for exporter secret derivation)](https://www.rfc-editor.org/rfc/rfc9420.html)
- [SFrame: Secure Frame (draft)](https://datatracker.ietf.org/doc/draft-ietf-sframe-enc/)
- [Discord DAVE Protocol](https://discord.com/blog/dave-protocol-e2ee-voice-video)
- [PLAN_E2EE.md §"Voice/Video E2EE Direction"](../../plans/PLAN_E2EE.md)
- [ADR 0001: E2EE Protocol Stack](../../docs/adr/0001-e2ee-mls-openmls.md)
