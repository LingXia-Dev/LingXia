import type { LxBridgeError, LxChannel, LxStream } from "./types";
import { BRIDGE_ERROR } from "./types";

export type ParamsSource<T> = T | (() => T);

export type MethodParams<TMethod> = TMethod extends () => any
  ? undefined
  : TMethod extends (params: infer P) => any
    ? P
    : never;

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
  code: string | number;
  data?: unknown;

  constructor(code: string | number, message: string, data?: unknown) {
    super(message);
    this.name = "BridgeInvocationError";
    this.code = code;
    this.data = data;
  }
}

export function toBridgeError(err: unknown): LxBridgeError {
  if (err instanceof BridgeInvocationError) return err;
  if (err && typeof err === "object") {
    const source = err as { code?: unknown; message?: unknown; data?: unknown };
    let code: string | number = BRIDGE_ERROR.INTERNAL_ERROR;
    if (typeof source.code === "string" && source.code.trim() !== "") {
      code = source.code;
    } else if (typeof source.code === "number" && Number.isFinite(source.code)) {
      code = source.code;
    }
    const message =
      typeof source.message === "string" && source.message.trim() !== ""
        ? source.message
        : "Unknown error";
    return new BridgeInvocationError(
      code,
      message,
      "data" in source ? source.data : undefined,
    );
  }
  const message =
    err instanceof Error ? err.message : typeof err === "string" ? err : "Unknown error";
  return new BridgeInvocationError(BRIDGE_ERROR.INTERNAL_ERROR, message);
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
