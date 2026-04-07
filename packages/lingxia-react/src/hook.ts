import * as React from "react";
import type {
  LxChannel,
  LxBridgeError,
  LxStream,
} from "@lingxia/bridge";

type ActionMap = Record<string, (...args: unknown[]) => unknown>;
type Snapshot = Record<string, unknown>;
type Listener = () => void;
type ParamsSource<T> = T | (() => T);
type MethodParams<TMethod> = TMethod extends () => any
  ? undefined
  : TMethod extends (params: infer P) => any
    ? P
    : never;
type StreamData<TMethod> = TMethod extends (...args: any[]) => LxStream<infer TData, any>
  ? TData
  : never;
type StreamResult<TMethod> = TMethod extends (...args: any[]) => LxStream<any, infer TResult>
  ? TResult
  : never;
type ChannelIn<TMethod> = TMethod extends (...args: any[]) => Promise<LxChannel<infer TIn, any>>
  ? TIn
  : never;
type ChannelOut<TMethod> = TMethod extends (...args: any[]) => Promise<LxChannel<any, infer TOut>>
  ? TOut
  : never;

let snapshot: Snapshot = {};
let subscribed = false;
let subscribeRetryTimer: ReturnType<typeof setTimeout> | null = null;
let initialSnapshotResolved = false;
let snapshotRequestInFlight = false;
const listeners = new Set<Listener>();

function toBridgeError(err: unknown): LxBridgeError {
  return err && typeof err === "object" && "code" in err
    ? (err as LxBridgeError)
    : { code: "BRIDGE_INTERNAL_ERROR", message: err instanceof Error ? err.message : String(err) };
}

function resolveParams<T>(source?: ParamsSource<T>): T | undefined {
  if (typeof source === "function") {
    return (source as () => T)();
  }
  return source;
}

function stableParamKey(value: unknown): string {
  if (value === undefined) return "undefined";
  try {
    return JSON.stringify(value) ?? "undefined";
  } catch {
    return String(value);
  }
}

function invokeMethod<TMethod extends (...args: any[]) => any>(
  method: TMethod,
  params: unknown,
): ReturnType<TMethod> {
  if (params === undefined) {
    return method() as ReturnType<TMethod>;
  }
  return method(params) as ReturnType<TMethod>;
}

function getMethodKey(method: unknown): string | undefined {
  if (typeof method !== "function") return undefined;
  const candidate = (method as { __funcName?: unknown }).__funcName;
  return typeof candidate === "string" && candidate !== "" ? candidate : undefined;
}

function notifyListeners(): void {
  listeners.forEach((listener) => {
    try {
      listener();
    } catch {
      // Ignore listener errors to avoid breaking state fanout.
    }
  });
}

function updateSnapshot(next: unknown): void {
  if (next && typeof next === "object") {
    snapshot = next as Snapshot;
  } else {
    snapshot = {};
  }
  notifyListeners();
}

function scheduleSubscribeRetry(): void {
  if (subscribeRetryTimer !== null || subscribed) return;
  subscribeRetryTimer = setTimeout(() => {
    subscribeRetryTimer = null;
    ensureBridgeSubscription();
  }, 10);
}

function requestInitialSnapshot(bridge: Window["LingXiaBridge"] | undefined): void {
  if (initialSnapshotResolved || snapshotRequestInFlight) return;
  if (!bridge?.call) return;
  snapshotRequestInFlight = true;
  bridge
    .call("state.getSnapshot", { scope: "page" })
    .then(() => {
      initialSnapshotResolved = true;
    })
    .catch(() => {
      scheduleSubscribeRetry();
    })
    .finally(() => {
      snapshotRequestInFlight = false;
    });
}

function ensureBridgeSubscription(): void {
  if (subscribed) return;
  const bridge = window.LingXiaBridge;
  const subscribeState = bridge?.state?.subscribe;
  if (!subscribeState) {
    scheduleSubscribeRetry();
    return;
  }
  subscribeState((next) => {
    updateSnapshot(next);
  });
  subscribed = true;
  requestInitialSnapshot(bridge);
}

function resolveActions<TActions extends ActionMap>(): TActions {
  const actions: ActionMap = {};
  const bridge = window.__pageBridge;
  if (!bridge?.__names) {
    return actions as TActions;
  }

  for (const name of bridge.__names) {
    if (typeof name !== "string") continue;
    const fn = bridge[name];
    if (typeof fn === "function") {
      actions[name] = fn as (...args: unknown[]) => unknown;
    }
  }

  return actions as TActions;
}

export function useLxPage<
  TData = Snapshot,
  TActions extends ActionMap = ActionMap,
>(): { data: TData; actions: TActions } {
  ensureBridgeSubscription();
  const [, setVersion] = React.useState(0);

  React.useEffect(() => {
    ensureBridgeSubscription();
    const listener: Listener = () => setVersion((v) => v + 1);
    listeners.add(listener);
    setVersion((v) => v + 1);
    return () => {
      listeners.delete(listener);
    };
  }, []);

  const actions = React.useMemo(() => resolveActions<TActions>(), []);
  return { data: snapshot as TData, actions };
}

export interface LxStreamOptions<TData, TReduced> {
  params?: unknown | (() => unknown);
  manual?: boolean;
  reduce?: (accumulated: TReduced, chunk: TData) => TReduced;
  initial?: TReduced;
}

export interface LxStreamState<TData, TResult = unknown> {
  data: TData | undefined;
  result: TResult | undefined;
  error: LxBridgeError | undefined;
  streaming: boolean;
  cancel: () => void;
  start: () => void;
}

export function useLxStream<
  TMethod extends (...args: any[]) => LxStream<any, any>,
  TReduced = StreamData<TMethod>,
>(
  method: TMethod,
  options?: LxStreamOptions<StreamData<TMethod>, TReduced> & {
    params?: ParamsSource<MethodParams<TMethod>>;
  },
): LxStreamState<
  TReduced extends StreamData<TMethod> ? StreamData<TMethod> : TReduced,
  StreamResult<TMethod>
> {
  type TData = StreamData<TMethod>;
  type TResult = StreamResult<TMethod>;
  type TOut = TReduced extends TData ? TData : TReduced;

  const [state, setState] = React.useState<{
    data: TOut | undefined;
    result: TResult | undefined;
    error: LxBridgeError | undefined;
    streaming: boolean;
  }>({
    data: (options?.reduce ? options.initial : undefined) as TOut | undefined,
    result: undefined,
    error: undefined,
    streaming: false,
  });

  const handleRef = React.useRef<LxStream<TData, TResult> | null>(null);
  const accRef = React.useRef<TReduced | undefined>(options?.initial);
  const optionsRef = React.useRef(options);
  const methodRef = React.useRef(method);
  const paramsRef = React.useRef(resolveParams(options?.params));
  const runIdRef = React.useRef(0);
  const resolvedParams = resolveParams(options?.params);
  const paramsKey = options?.manual ? "" : stableParamKey(resolvedParams);
  const methodDep = getMethodKey(method) ?? method;

  optionsRef.current = options;
  methodRef.current = method;
  paramsRef.current = resolvedParams;

  const cancel = React.useCallback(() => {
    runIdRef.current += 1;
    handleRef.current?.cancel();
    handleRef.current = null;
    setState((prev) => ({ ...prev, streaming: false }));
  }, []);

  const start = React.useCallback(() => {
    handleRef.current?.cancel();
    const runId = runIdRef.current + 1;
    runIdRef.current = runId;

    const opts = optionsRef.current;
    accRef.current = opts?.initial;
    setState({
      data: (opts?.reduce ? opts.initial : undefined) as TOut | undefined,
      result: undefined,
      error: undefined,
      streaming: true,
    });

    let handle: LxStream<TData, TResult>;
    try {
      handle = invokeMethod(methodRef.current, paramsRef.current) as LxStream<TData, TResult>;
    } catch (err: unknown) {
      if (runIdRef.current !== runId) return;
      handleRef.current = null;
      setState((prev) => ({
        ...prev,
        error: toBridgeError(err),
        streaming: false,
      }));
      return;
    }
    handleRef.current = handle;

    handle.on("data", (chunk: TData) => {
      if (runIdRef.current !== runId) return;
      const currentOpts = optionsRef.current;
      if (currentOpts?.reduce) {
        accRef.current = currentOpts.reduce(
          accRef.current as TReduced,
          chunk,
        );
        setState((prev) => ({
          ...prev,
          data: accRef.current as TOut,
        }));
      } else {
        setState((prev) => ({
          ...prev,
          data: chunk as unknown as TOut,
        }));
      }
    });

    handle.on("end", (result: TResult) => {
      if (runIdRef.current !== runId) return;
      handleRef.current = null;
      setState((prev) => ({
        ...prev,
        result,
        streaming: false,
      }));
    });

    handle.on("error", (err: LxBridgeError) => {
      if (runIdRef.current !== runId) return;
      handleRef.current = null;
      setState((prev) => ({
        ...prev,
        error: err,
        streaming: false,
      }));
    });
  }, []);

  React.useEffect(() => {
    if (!options?.manual) {
      start();
    }
    return () => {
      runIdRef.current += 1;
      handleRef.current?.cancel();
      handleRef.current = null;
    };
  }, [methodDep, options?.manual, paramsKey, start]);

  return { ...state, cancel, start };
}

export interface LxChannelOptions {
  params?: unknown | (() => unknown);
  manual?: boolean;
}

export interface LxChannelState<TData, TOut = TData> {
  last: TData | undefined;
  error: LxBridgeError | undefined;
  connecting: boolean;
  connected: boolean;
  send: (payload: TOut) => void;
  close: (code?: string, reason?: string) => void;
  reopen: () => void;
}

export function useLxChannel<
  TMethod extends (...args: any[]) => Promise<LxChannel<any, any>>,
>(
  method: TMethod,
  options?: LxChannelOptions & {
    params?: ParamsSource<MethodParams<TMethod>>;
  },
): LxChannelState<ChannelIn<TMethod>, ChannelOut<TMethod>> {
  type TIn = ChannelIn<TMethod>;
  type TOut = ChannelOut<TMethod>;

  const [state, setState] = React.useState<{
    last: TIn | undefined;
    error: LxBridgeError | undefined;
    connecting: boolean;
    connected: boolean;
  }>({
    last: undefined,
    error: undefined,
    connecting: false,
    connected: false,
  });

  const chRef = React.useRef<LxChannel<TIn, TOut> | null>(null);
  const methodRef = React.useRef(method);
  const paramsRef = React.useRef(resolveParams(options?.params));
  const runIdRef = React.useRef(0);
  const resolvedParams = resolveParams(options?.params);
  const paramsKey = options?.manual ? "" : stableParamKey(resolvedParams);
  const methodDep = getMethodKey(method) ?? method;

  methodRef.current = method;
  paramsRef.current = resolvedParams;

  const send = React.useCallback((payload: TOut) => {
    chRef.current?.send(payload);
  }, []);

  const close = React.useCallback((code?: string, reason?: string) => {
    runIdRef.current += 1;
    chRef.current?.close(code, reason);
    chRef.current = null;
    setState((prev) => ({ ...prev, connecting: false, connected: false }));
  }, []);

  const reopen = React.useCallback(() => {
    chRef.current?.close();
    chRef.current = null;

    const runId = ++runIdRef.current;
    setState({
      last: undefined,
      error: undefined,
      connecting: true,
      connected: false,
    });

    Promise.resolve(
      invokeMethod(methodRef.current, paramsRef.current) as Promise<LxChannel<TIn, TOut>>,
    )
      .then((ch) => {
        if (runIdRef.current !== runId) {
          ch.close();
          return;
        }
        chRef.current = ch;
        setState((prev) => ({ ...prev, connecting: false, connected: true }));

        ch.on("data", (payload) => {
          if (runIdRef.current !== runId) return;
          setState((prev) => ({ ...prev, last: payload as TIn }));
        });
        ch.on("close", () => {
          if (runIdRef.current !== runId) return;
          chRef.current = null;
          setState((prev) => ({ ...prev, connecting: false, connected: false }));
        });
        ch.on("error", (err: LxBridgeError) => {
          if (runIdRef.current !== runId) return;
          chRef.current = null;
          setState((prev) => ({ ...prev, error: err, connecting: false, connected: false }));
        });
      })
      .catch((err: unknown) => {
        if (runIdRef.current !== runId) return;
        setState({
          last: undefined,
          error: toBridgeError(err),
          connecting: false,
          connected: false,
        });
      });
  }, []);

  React.useEffect(() => {
    if (!options?.manual) {
      reopen();
    }
    return () => {
      runIdRef.current += 1;
      chRef.current?.close();
      chRef.current = null;
    };
  }, [methodDep, options?.manual, paramsKey, reopen]);

  return { ...state, send, close, reopen };
}
