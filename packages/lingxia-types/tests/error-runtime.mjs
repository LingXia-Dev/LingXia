import assert from "node:assert/strict";
import { hostUpgradeRequired, isLxError, LxError, LX_ERROR_CODES, surfaceErrorCode, SURFACE_ERROR_CODES } from "../dist/error.js";

// A host rejection is an Error with a string `code`; the guard reads its shape, not its class.
const rejection = (code, message = "failed", data) => Object.assign(new Error(message), { code, ...(data ? { data } : {}) });
for (const code of LX_ERROR_CODES) assert.equal(isLxError(rejection(code)), true, code);
assert.equal(isLxError(rejection("E_SOMETHING_NEWER")), true, "a code this package predates is still an lx error");
assert.equal(isLxError({ code: "E_NOT_FOUND", message: "plain object from another engine" }), true);
for (const input of [null, undefined, "E_ABORT", {}, new Error("x"), { code: 1000, message: "m" }, { code: "E_ABORT" }, { code: "BRIDGE_NOT_READY", message: "m" }, { code: "e_abort", message: "m" }]) {
  assert.equal(isLxError(input), false, JSON.stringify(input));
}
const frozen = Object.freeze(rejection("E_IO"));
assert.equal(isLxError(frozen), true, "the guard does not modify the caught value");

for (const code of SURFACE_ERROR_CODES) {
  assert.equal(surfaceErrorCode({ code: "E_INTERNAL", data: { code } }), code);
  assert.equal(surfaceErrorCode({ code }), null, "surface codes belong in data.code");
}
for (const code of ["unknown", "", 1000, null]) assert.equal(surfaceErrorCode({ data: { code } }), null);

// The runtime floor is a `reason`; `code` alone is the wide E_NOT_SUPPORTED bucket.
assert.equal(hostUpgradeRequired({ code: "E_NOT_SUPPORTED", reason: "hostUpgradeRequired", detail: "update host" }), true);
assert.equal(hostUpgradeRequired({ code: "E_NOT_SUPPORTED", reason: "deviceUnsupported" }), false);
assert.equal(hostUpgradeRequired({ code: "E_NOT_SUPPORTED", data: { bizCode: 6002 } }), false);
assert.equal(hostUpgradeRequired(new Error("x")), false);
// Built by a mock: the fields are set, and every instance passes the guard.
const built = new LxError("no photos", { code: "E_PERMISSION_DENIED", reason: "photoPermissionDenied", detail: "d", data: { n: 1 }, cause: "c" });
assert.deepEqual([built.code, built.reason, built.detail, built.data.n, built.cause, built.message], ["E_PERMISSION_DENIED", "photoPermissionDenied", "d", 1, "c", "no photos"]);
assert.equal(isLxError(built), true);
assert.equal("reason" in new LxError("x", { code: "E_IO" }), false, "an absent field stays absent");
assert.equal(isLxError(new LxError("as the host builds it")), true, "an instance is an lx error before its fields are set");
console.log("lx error guard passed.");
