# Spike: Insertable-Streams / RTCRtpScriptTransform WebView Verification

**Phase:** 0 (Engineering Spike)
**Status:** Executable probe ready — target-runtime results still require
Windows, macOS, and Linux packaged-app runs
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

## Verification Matrix

The upstream engines implement or are actively testing the standardized API,
but that is not evidence that a particular packaged webview exposes a working
sender-and-receiver frame path. The checked-in probe is the release evidence.

| Target | Runtime | Packaged runtime result | Shipping E2EE media path |
|---|---|---|---|
| Windows | WebView2 | Pending target run | Native LiveKit GCM in Rust host |
| macOS | WKWebView | Pending target run | Native LiveKit GCM in Rust host |
| Linux | WebKitGTK | Pending target run | Native LiveKit GCM in Rust host |

Filament does not select a webview media path on any result. The probe is a
compatibility matrix required by the design, while the security policy chooses
the native path on every desktop target. If that backend is not ready, calls
remain disabled.

## Verification Procedure (Phase 5)

Actual verification must be performed in the packaged application on each
target platform. Bundle `probe.html`, `probe.css`, `probe.js`, and
`rtp-transform-worker.js` as local assets, navigate the test webview to
`probe.html`, select **Run probe**, and preserve its JSON output with the test
record. Do not host the probe remotely or weaken the application CSP.

A passing record must have:

- `outcome: "supported"`;
- every `features` value set to `true`;
- `observed_directions` exactly equal to `["receiver", "sender"]`;
- a user-agent/runtime version captured in the bounded `user_agent` field.

Merely observing `RTCRtpScriptTransform` on the global object is not a pass.
The probe creates a local synthetic audio track and two peer connections, then
requires encoded frames to traverse worker transforms in both directions. It
uses no network servers, credentials, MLS keys, microphone, or camera.

Record results for:

1. **WebView2 (Windows):**
   - Record Windows, WebView2 runtime, and packaged-app versions.
   - Run against the minimum supported runtime and current Evergreen Stable.

2. **WKWebView (macOS):**
   - Record macOS, WebKit/Safari, and packaged-app versions.
   - Run on the oldest and newest supported macOS versions.

3. **WebKitGTK (Linux):**
   - Record distribution, WebKitGTK, GStreamer, and packaged-app versions.
   - Run on every supported distribution baseline because port build flags and
     multimedia packages can differ.

For `unsupported` or `failed`, preserve the exact result and confirm that the
native backend remains selected. A failure must never make a plaintext or
webview-key path available.

## Fallback: Native WebRTC Media Path

When a webview lacks insertable streams support, the required fallback is a **native WebRTC media path in the host layer**:

- The Tauri Rust host process handles:
  - Media capture (microphone, camera, screen share) via native APIs.
  - SFrame encryption of RTP frames using the MLS exporter secret.
  - LiveKit connection via the Rust LiveKit client SDK.
- The webview's WebRTC stack is not used for E2EE calls.
- The webview handles UI only (call controls, participant display, encryption indicators).
- The webview receives only bounded, public call-control state from the future
  audited adapter. Key material and decoded media do not cross IPC.

This fallback is architecturally consistent with the packaged-client design: crypto operations (including SFrame media encryption) run in the Rust host process, and the webview handles UI only. Key material never enters the JS heap regardless of which media path is used.

**Key constraint:** Shipping unencrypted media is never an acceptable fallback. If neither insertable streams nor a native WebRTC media path can be made to work on a platform, E2EE calls are not supported on that platform — not even as a degraded mode.

## Recommendations

1. **Phase 5 gate:** Preserve a passing or failing probe record for every
   supported runtime baseline before calls ship on that platform.
2. **Native-only security path:** Keep the Rust LiveKit GCM backend selected on
   all targets regardless of the diagnostic webview result.
3. **Runtime tracking:** Re-run the probe when the minimum WebView2, macOS, or
   WebKitGTK runtime changes.
4. **Fail closed:** If the native path cannot build, connect, attach cryptors,
   or verify the current MLS epoch, keep calls disabled.

## References

- [W3C: Insertable Media Using Streams](https://w3c.github.io/webrtc-insertable-media/)
- [W3C: WebRTC Encoded Transform](https://w3c.github.io/webrtc-encoded-transform/)
- [Microsoft: Introduction to WebView2](https://learn.microsoft.com/en-us/microsoft-edge/webview2/)
- [WebKit: Interop 2025 encoded-transform focus](https://webkit.org/blog/16458/announcing-interop-2025/)
- [RFC 9420: MLS (for exporter secret derivation)](https://www.rfc-editor.org/rfc/rfc9420.html)
- [SFrame: Secure Frame (draft)](https://datatracker.ietf.org/doc/draft-ietf-sframe-enc/)
- [Discord DAVE Protocol](https://discord.com/blog/dave-protocol-e2ee-voice-video)
- [PLAN_E2EE.md §"Voice/Video E2EE Direction"](../../plans/PLAN_E2EE.md)
- [ADR 0001: E2EE Protocol Stack](../../docs/adr/0001-e2ee-mls-openmls.md)
