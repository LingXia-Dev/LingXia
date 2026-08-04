import assert from "node:assert/strict";
import {
  isLxApiError,
  parseLxApiError,
} from "../dist/error.js";

const raw = Object.freeze({
  code: 1000,
  message: "original message",
  data: Object.freeze({ detail: "actionable detail" }),
});
const before = JSON.stringify(raw);

isLxApiError(raw);
assert.equal(JSON.stringify(raw), before);

const parsed = parseLxApiError(raw);
assert.deepEqual(parsed, {
  code: 1000,
  key: "err_code_1000",
  message: "actionable detail",
  raw,
});
assert.equal(JSON.stringify(raw), before);
assert.doesNotThrow(() => JSON.stringify(parsed));

const normalized = Object.freeze({
  code: 1000,
  key: "err_code_1000",
  message: "already normalized",
  raw,
});
assert.equal(isLxApiError(normalized), true);
assert.equal(JSON.stringify(normalized), JSON.stringify({
  code: 1000,
  key: "err_code_1000",
  message: "already normalized",
  raw,
}));
