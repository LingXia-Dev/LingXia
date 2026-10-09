import assert from "node:assert/strict";
import {
  hostUpgradeRequired,
  isLxApiError,
  parseLxApiError,
  extractLxErrorCode,
  infoForLxErrorCode,
  isKnownLxErrorCode,
  requireLxApiError,
  surfaceErrorCode,
  SURFACE_ERROR_CODES,
} from "../dist/error.js";
import { ERR_CODE_INFO_BY_CODE } from '../dist/generated/error.js';

// Own normalization for every published business code, not the native paths
// that produce them (those require separate API/device contracts).
for (const { code, key } of Object.values(ERR_CODE_INFO_BY_CODE)) {
  assert.equal(isKnownLxErrorCode(code), true);
  assert.deepEqual(infoForLxErrorCode(code), { code, key });
  for (const envelope of [
    { code }, { code: String(code) },
    { code: 'E_INTERNAL', data: { bizCode: code } },
    { code: 'E_INTERNAL', data: { bizCode: String(code) } },
    { code: 'E_INTERNAL', data: { code } },
  ]) {
    const input = Object.freeze({ ...envelope, message: 'business failure' });
    const result = requireLxApiError(input);
    assert.equal(extractLxErrorCode(input), code);
    assert.equal(result.code, code);
    assert.equal(result.key, key);
    assert.equal(result.message, 'business failure');
    assert.equal(result.raw, input);
    assert.equal(isLxApiError(result), true);
    assert.equal(isLxApiError(input), false);
  }
}
assert.equal(extractLxErrorCode({ code: 1000, data: { bizCode: 1001 } }), 1000);
assert.equal(extractLxErrorCode({ code: 'E_INTERNAL', data: { bizCode: 1001, code: 1002 } }), 1001);
for (const code of [0, -1, 999999, 1000.5, NaN, Infinity]) {
  assert.equal(isKnownLxErrorCode(code), false);
  assert.equal(infoForLxErrorCode(code), null);
  assert.equal(parseLxApiError({ code }), null);
  assert.throws(() => requireLxApiError({ code }), /Unknown LingXia API error/);
}
for (const input of [null, undefined, '1000', {}, { code: '' }, { code: 'BRIDGE_NOT_READY' }]) {
  assert.equal(parseLxApiError(input), null);
  assert.equal(isLxApiError(input), false);
}
for (const code of SURFACE_ERROR_CODES) {
  assert.equal(surfaceErrorCode({ code: 'E_INTERNAL', data: { code } }), code);
  assert.equal(surfaceErrorCode({ code }), null, 'surface codes belong in data.code');
  assert.equal(parseLxApiError({ code: 'E_INTERNAL', data: { code } }), null);
}
for (const code of ['unknown', '', 1000, null]) {
  assert.equal(surfaceErrorCode({ data: { code } }), null);
}

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
