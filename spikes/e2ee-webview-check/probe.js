"use strict";

const PROBE_VERSION = 1;
const PROBE_TIMEOUT_MS = 10_000;

async function withTimeout(operation, reason) {
  let timeout;
  try {
    return await Promise.race([
      operation,
      new Promise((_, reject) => {
        timeout = globalThis.setTimeout(
          () => reject(new Error(reason)),
          PROBE_TIMEOUT_MS,
        );
      }),
    ]);
  } finally {
    globalThis.clearTimeout(timeout);
  }
}

function featureSnapshot() {
  return {
    secure_context: globalThis.isSecureContext === true,
    worker: typeof globalThis.Worker === "function",
    peer_connection: typeof globalThis.RTCPeerConnection === "function",
    script_transform: typeof globalThis.RTCRtpScriptTransform === "function",
    sender_transform:
      typeof globalThis.RTCRtpSender === "function" &&
      "transform" in globalThis.RTCRtpSender.prototype,
    receiver_transform:
      typeof globalThis.RTCRtpReceiver === "function" &&
      "transform" in globalThis.RTCRtpReceiver.prototype,
  };
}

function waitForIceGathering(peer) {
  if (peer.iceGatheringState === "complete") {
    return Promise.resolve();
  }
  return new Promise((resolve) => {
    const changed = () => {
      if (peer.iceGatheringState === "complete") {
        peer.removeEventListener("icegatheringstatechange", changed);
        resolve();
      }
    };
    peer.addEventListener("icegatheringstatechange", changed);
  });
}

function waitForDirections(worker) {
  return new Promise((resolve, reject) => {
    const observed = new Set();
    const timeout = globalThis.setTimeout(() => {
      reject(new Error("frame_timeout"));
    }, PROBE_TIMEOUT_MS);

    worker.onmessage = ({ data }) => {
      if (data?.type === "error") {
        globalThis.clearTimeout(timeout);
        reject(new Error(data.reason));
        return;
      }
      if (data?.type !== "frame") {
        return;
      }
      if (data.direction === "sender" || data.direction === "receiver") {
        observed.add(data.direction);
      }
      if (observed.size === 2) {
        globalThis.clearTimeout(timeout);
        resolve([...observed].sort());
      }
    };
  });
}

async function runFrameProbe() {
  const features = featureSnapshot();
  if (!Object.values(features).every(Boolean)) {
    return { outcome: "unsupported", features, observed_directions: [] };
  }

  const worker = new Worker("rtp-transform-worker.js");
  const senderPeer = new RTCPeerConnection({ iceServers: [] });
  const receiverPeer = new RTCPeerConnection({ iceServers: [] });
  const AudioContextConstructor = globalThis.AudioContext || globalThis.webkitAudioContext;
  let audioContext;
  let oscillator;

  try {
    if (typeof AudioContextConstructor !== "function") {
      throw new Error("audio_context_unavailable");
    }
    audioContext = new AudioContextConstructor();
    await withTimeout(audioContext.resume(), "audio_context_timeout");
    oscillator = audioContext.createOscillator();
    const destination = audioContext.createMediaStreamDestination();
    oscillator.connect(destination);
    oscillator.start();

    const [track] = destination.stream.getAudioTracks();
    if (!track) {
      throw new Error("audio_track_unavailable");
    }
    const sender = senderPeer.addTrack(track, destination.stream);
    sender.transform = new RTCRtpScriptTransform(worker, { direction: "sender" });

    receiverPeer.ontrack = (event) => {
      event.receiver.transform = new RTCRtpScriptTransform(worker, {
        direction: "receiver",
      });
    };

    const directions = waitForDirections(worker);
    const offer = await senderPeer.createOffer();
    await senderPeer.setLocalDescription(offer);
    await withTimeout(waitForIceGathering(senderPeer), "sender_ice_timeout");
    await receiverPeer.setRemoteDescription(senderPeer.localDescription);
    const answer = await receiverPeer.createAnswer();
    await receiverPeer.setLocalDescription(answer);
    await withTimeout(waitForIceGathering(receiverPeer), "receiver_ice_timeout");
    await senderPeer.setRemoteDescription(receiverPeer.localDescription);

    return {
      outcome: "supported",
      features,
      observed_directions: await directions,
    };
  } finally {
    oscillator?.stop();
    await audioContext?.close();
    senderPeer.close();
    receiverPeer.close();
    worker.terminate();
  }
}

async function runProbe() {
  const startedAt = new Date().toISOString();
  try {
    const result = await runFrameProbe();
    return {
      schema_version: PROBE_VERSION,
      started_at: startedAt,
      user_agent: navigator.userAgent.slice(0, 1024),
      ...result,
    };
  } catch (error) {
    return {
      schema_version: PROBE_VERSION,
      started_at: startedAt,
      user_agent: navigator.userAgent.slice(0, 1024),
      outcome: "failed",
      features: featureSnapshot(),
      observed_directions: [],
      reason: error instanceof Error ? error.message.slice(0, 128) : "unknown_error",
    };
  }
}

const runButton = document.querySelector("#run");
const resultElement = document.querySelector("#result");

runButton.addEventListener("click", async () => {
  runButton.disabled = true;
  resultElement.textContent = "Running…";
  const result = await runProbe();
  resultElement.textContent = JSON.stringify(result, null, 2);
  runButton.disabled = false;
});

globalThis.runFilamentEncodedTransformProbe = runProbe;
