import { EMPTY_ROOT_REF, type NativeError, type RootRef } from "./types.js";
import type { NativeErrorCode } from "./schema.js";

export function nativeError(
  code: NativeErrorCode,
  message: string,
  options: {
    scope?: "node" | "root";
    recoverable?: boolean;
    root?: RootRef;
  } = {}
): NativeError {
  return {
    code,
    scope: options.scope ?? "root",
    recoverable: options.recoverable ?? false,
    root: options.root ?? EMPTY_ROOT_REF,
    message,
  };
}

export function isNativeErrorCode(value: unknown): value is NativeErrorCode {
  return (
    value === "NATIVE_ROOT_UNAVAILABLE" ||
    value === "NATIVE_ROOT_INCOMPATIBLE" ||
    value === "NATIVE_ROOT_INVALID_STRUCTURE" ||
    value === "NATIVE_ROOT_FAILED" ||
    value === "NATIVE_ROOT_UNSUPPORTED_LAYOUT" ||
    value === "NATIVE_COMPONENT_INVALID_PROPS" ||
    value === "NATIVE_COMPONENT_MOUNT_FAILED" ||
    value === "NATIVE_COMPONENT_COMMAND_FAILED" ||
    value === "NATIVE_ROOT_UNSUPPORTED_STYLE" ||
    value === "NATIVE_ROOT_DESTROYED"
  );
}
