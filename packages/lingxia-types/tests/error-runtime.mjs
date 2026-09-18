import assert from "node:assert/strict";
import {
  hostUpgradeRequired,
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

// The runtime floor rides on `data.bizCode`; `code` alone is the wide
// E_NOT_SUPPORTED bucket.
assert.equal(
  hostUpgradeRequired({ code: "E_NOT_SUPPORTED", data: { bizCode: 6002, detail: "update host" } }),
  true,
);
assert.equal(hostUpgradeRequired({ code: "E_NOT_SUPPORTED", data: { bizCode: 6001 } }), false);
assert.equal(hostUpgradeRequired({ code: "E_NOT_FOUND", data: { bizCode: 1003 } }), false);
assert.equal(hostUpgradeRequired(new Error("x")), false);
