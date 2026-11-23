// Node (unit tests): `lx` is a live binding, so `setLx` from `@lingxia/logic/testing` reaches
// modules that imported it before the test ran.
export let lx = globalThis.lx;
export function setLx(value) {
  lx = value;
  // A persisted store reaches lx.storage through the global, as in the Logic runtime.
  globalThis.lx = value;
}
export * from "@lingxia/types/page";
export * from "@lingxia/types/error";
