"use strict";

const MAX_REPORTED_FRAMES = 2;

self.onrtctransform = (event) => {
  const transformer = event.transformer;
  const direction = transformer.options?.direction;
  if (direction !== "sender" && direction !== "receiver") {
    self.postMessage({ type: "error", reason: "invalid_direction" });
    return;
  }

  let frames = 0;
  const observer = new TransformStream({
    transform(frame, controller) {
      frames += 1;
      if (frames <= MAX_REPORTED_FRAMES) {
        self.postMessage({ type: "frame", direction, frames });
      }
      controller.enqueue(frame);
    },
  });

  transformer.readable
    .pipeThrough(observer)
    .pipeTo(transformer.writable)
    .catch(() => self.postMessage({ type: "error", reason: "pipeline_failed" }));
};
