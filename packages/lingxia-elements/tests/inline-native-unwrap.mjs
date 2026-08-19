import assert from "node:assert/strict";
import { unwrapNativeEventPayload } from "../dist/inline-native/unwrap.js";

const press = unwrapNativeEventPayload({
  type: "press",
  detail: { source: "pointer" },
});
assert.deepEqual(press, { source: "pointer" });

const value = unwrapNativeEventPayload({
  type: "valuecommit",
  detail: { value: 12.5 },
});
assert.deepEqual(value, { value: 12.5 });

const empty = unwrapNativeEventPayload({ type: "play", detail: undefined });
assert.deepEqual(empty, {});

const alreadyPayload = unwrapNativeEventPayload({ value: 3 });
assert.deepEqual(alreadyPayload, { value: 3 });

const htmlStillHasDetail = { type: "press", detail: { source: "keyboard" } };
assert.equal(htmlStillHasDetail.detail.source, "keyboard");
assert.deepEqual(unwrapNativeEventPayload(htmlStillHasDetail), { source: "keyboard" });
