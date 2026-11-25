import type { LxBridgeError, LxChannel, LxStream } from "./types";
import { BRIDGE_ERROR } from "./types";

export type ParamsSource<T> = T | (() => T);

/** The one payload a method takes: `undefined` for none, `P | undefined` when optional. */
export type MethodParams<TMethod> = TMethod extends (...args: infer A) => any
  ? A extends []
    ? undefined
    : A extends [infer P]
      ? P
      : A extends [(infer P)?]
        ? P | undefined
        : never
  : never;

/** Whether calling `TMethod` needs its payload. */
export type RequiresParams<TMethod> = TMethod extends (...args: infer A) => any
  ? [] extends A
    ? false
    : true
  : never;

/** `params`, required exactly when the method requires its payload. */
export type ParamsOption<TMethod, TSource> = RequiresParams<TMethod> extends true
  ? { params: TSource }
  : { params?: TSource };

/** A hook's options argument: required when `params` is. */
export type OptionsArg<TMethod, TOptions> = RequiresParams<TMethod> extends true
  ? [options: TOptions]
  : [options?: TOptions];

export type StreamData<TMethod> =
  TMethod extends (...args: any[]) => LxStream<infer TData, any> ? TData : never;

export type StreamResult<TMethod> =
  TMethod extends (...args: any[]) => LxStream<any, infer TResult>
    ? TResult
    : never;

export type ChannelIn<TMethod> =
  TMethod extends (...args: any[]) => Promise<LxChannel<infer TIn, any>>
    ? TIn
    : never;

export type ChannelOut<TMethod> =
  TMethod extends (...args: any[]) => Promise<LxChannel<any, infer TOut>>
    ? TOut
    : never;

class BridgeInvocationError extends Error implements LxBridgeError {
  code: string;
  reason?: string;
  data?: unknown;

  constructor(code: string, message: string, reason?: string, data?: unknown) {
    super(message);
    this.name = "BridgeInvocationError";
    this.code = code;
    if (reason !== undefined) this.reason = reason;
    this.data = data;
  }
}

export function toBridgeError(err: unknown): LxBridgeError {
  if (err instanceof BridgeInvocationError) return err;
  if (err && typeof err === "object") {
    const source = err as { code?: unknown; message?: unknown; reason?: unknown; data?: unknown };
    const code =
      typeof source.code === "string" && source.code.trim() !== "" ? source.code : "E_INTERNAL";
    const message =
      typeof source.message === "string" && source.message.trim() !== ""
        ? source.message
        : "Unknown error";
    return new BridgeInvocationError(
      code,
      message,
      typeof source.reason === "string" ? source.reason : undefined,
      "data" in source ? source.data : undefined,
    );
  }
  const message =
    err instanceof Error ? err.message : typeof err === "string" ? err : "Unknown error";
  return new BridgeInvocationError("E_INTERNAL", message);
}

export function toNativeError(err: unknown): import("./types").NativeError {
  const bridgeError = toBridgeError(err);
  return {
    code: bridgeError.code,
    message:
      typeof bridgeError.message === "string" && bridgeError.message.trim() !== ""
        ? bridgeError.message
        : "Unknown error",
    ...(bridgeError.reason === undefined ? {} : { reason: bridgeError.reason }),
    ...(bridgeError.data === undefined ? {} : { data: bridgeError.data }),
  };
}

export function isNativeError(
  error: unknown,
  code?: string,
): error is import("./types").NativeError {
  if (!error || typeof error !== "object") return false;
  const candidate = error as { code?: unknown; message?: unknown };
  if (typeof candidate.code !== "string" || candidate.code.trim() === "") return false;
  if (typeof candidate.message !== "string") return false;
  return code === undefined || candidate.code === code;
}

export function resolveParams<T>(source?: ParamsSource<T>): T | undefined {
  if (typeof source === "function") {
    return (source as () => T)();
  }
  return source;
}

export function stableParamKey(value: unknown): string {
  if (value === undefined) return "undefined";
  try {
    return JSON.stringify(value) ?? "undefined";
  } catch {
    return String(value);
  }
}

export function invokeMethod<TMethod extends (...args: any[]) => any>(
  method: TMethod,
  params: unknown,
): ReturnType<TMethod> {
  if (params === undefined) {
    return method() as ReturnType<TMethod>;
  }
  return method(params) as ReturnType<TMethod>;
}

export function getMethodKey(method: unknown): string | undefined {
  if (typeof method !== "function") return undefined;
  const candidate = (method as { __funcName?: unknown }).__funcName;
  return typeof candidate === "string" && candidate !== "" ? candidate : undefined;
}
