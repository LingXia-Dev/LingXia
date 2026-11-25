// The host's own `lx`, not a copy or a wrapper: it is installed before any Logic module runs.
export const lx = globalThis.lx;
export { packageText } from "./package-text.js";
export * from "@lingxia/types/page";
export * from "@lingxia/types/error";
