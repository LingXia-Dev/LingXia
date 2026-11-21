// The host's own `lx`, not a copy or a wrapper: it is installed before any Logic module runs.
export const lx = globalThis.lx;
export * from "@lingxia/types/page";
export * from "@lingxia/types/error";
