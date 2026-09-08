import assert from "node:assert/strict";
import { normalizeVideoEventDetail } from "../dist/video-events.js";

assert.deepEqual(normalizeVideoEventDetail("fullscreenchange", { fullScreen: true, direction: "landscape" }),
  { fullScreen: true, fullscreen: true, direction: "landscape" });
assert.equal(normalizeVideoEventDetail("fullscreenchange", { fullscreen: false, fullScreen: true }).fullscreen, false);
assert.deepEqual(normalizeVideoEventDetail("error", { errMsg: "Decoder failed" }),
  { errMsg: "Decoder failed", code: "NATIVE_COMPONENT_COMMAND_FAILED", message: "Decoder failed" });
assert.deepEqual(normalizeVideoEventDetail("error", { code: "DECODE_FAILED", message: "Bad frame", recoverable: true }),
  { code: "DECODE_FAILED", message: "Bad frame", recoverable: true });
assert.deepEqual(normalizeVideoEventDetail("playing", { currentTimeMs: 100 }), {});
assert.deepEqual(normalizeVideoEventDetail("timeupdate", { currentTime: 1.5, duration: 10 }), { currentTime: 1.5, duration: 10 });
