import {
  reactive,
  ref,
  onUnmounted,
  unref,
  watch,
  type Ref,
} from "vue";
import type {
  LxChannel,
  LxBridgeError,
  LxStream,
} from "@lingxia/bridge";

type ActionMap = Record<string, (...args: unknown[]) => unknown>;
type Snapshot = Record<string, unknown>;
type MethodSource<T> = T | Ref<T>;
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

function resolveMethod<TMethod>(source: MethodSource<TMethod>): TMethod {
  return unref(source) as TMethod;
}

function getMethodKey(method: unknown): string | undefined {
  if (typeof method !== "function") return undefined;
  const candidate = (method as { __funcName?: unknown }).__funcName;
  return typeof candidate === "string" && candidate !== "" ? candidate : undefined;
}

const reactiveSnapshot = reactive<Snapshot>({});
let subscribed = false;
let subscribeRetryTimer: ReturnType<typeof setTimeout> | null = null;
let initialSnapshotResolved = false;
let snapshotRequestInFlight = false;

function replaceReactiveSnapshot(next: unknown): void {
  const normalized: Snapshot =
    next && typeof next === "object" ? (next as Snapshot) : {};

  for (const key of Object.keys(reactiveSnapshot)) {
    if (!Object.prototype.hasOwnProperty.call(normalized, key)) {
      delete reactiveSnapshot[key];
    }
  }
  Object.assign(reactiveSnapshot, normalized);
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
    replaceReactiveSnapshot(next);
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
  return { data: reactiveSnapshot as TData, actions: resolveActions<TActions>() };
}

export interface LxStreamOptions<TData, TReduced> {
  params?: unknown | (() => unknown);
  manual?: boolean;
  reduce?: (accumulated: TReduced, chunk: TData) => TReduced;
  initial?: TReduced;
}

export interface LxStreamState<TData, TResult = unknown> {
  data: Ref<TData | undefined>;
  result: Ref<TResult | undefined>;
  error: Ref<LxBridgeError | undefined>;
  streaming: Ref<boolean>;
  cancel: () => void;
  start: () => void;
}

export function useLxStream<
  TMethod extends (...args: any[]) => LxStream<any, any>,
  TReduced = StreamData<TMethod>,
>(
  method: MethodSource<TMethod>,
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

  const data = ref<TOut | undefined>(
    (options?.reduce ? options.initial : undefined) as TOut | undefined,
  ) as Ref<TOut | undefined>;
  const result = ref<TResult | undefined>(undefined) as Ref<TResult | undefined>;
  const error = ref<LxBridgeError | undefined>(undefined);
  const streaming = ref(false);

  let handle: LxStream<TData, TResult> | null = null;
  let acc: TReduced | undefined = options?.initial;
  let runId = 0;

  function cancel(): void {
    runId += 1;
    handle?.cancel();
    handle = null;
    streaming.value = false;
  }

  function start(): void {
    handle?.cancel();
    const currentRunId = runId + 1;
    runId = currentRunId;

    acc = options?.initial;
    data.value = (options?.reduce ? options.initial : undefined) as TOut | undefined;
    result.value = undefined;
    error.value = undefined;
    streaming.value = true;

    let nextHandle: LxStream<TData, TResult>;
    try {
      const params = resolveParams(options?.params);
      nextHandle = invokeMethod(resolveMethod(method), params) as LxStream<TData, TResult>;
    } catch (err: unknown) {
      if (runId !== currentRunId) return;
      handle = null;
      error.value = toBridgeError(err);
      streaming.value = false;
      return;
    }

    handle = nextHandle;

    nextHandle.on("data", (chunk: TData) => {
      if (runId !== currentRunId) return;
      if (options?.reduce) {
        acc = options.reduce(acc as TReduced, chunk);
        data.value = acc as TOut;
      } else {
        data.value = chunk as unknown as TOut;
      }
    });

    nextHandle.on("end", (res: TResult) => {
      if (runId !== currentRunId) return;
      handle = null;
      result.value = res;
      streaming.value = false;
    });

    nextHandle.on("error", (err: LxBridgeError) => {
      if (runId !== currentRunId) return;
      handle = null;
      error.value = err;
      streaming.value = false;
    });
  }

  watch(
    () => {
      if (options?.manual) return null;
      const resolvedMethod = resolveMethod(method);
      return [
        getMethodKey(resolvedMethod) ?? resolvedMethod,
        stableParamKey(resolveParams(options?.params)),
      ];
    },
    () => {
      if (!options?.manual) {
        start();
      }
    },
    { immediate: !options?.manual },
  );

  onUnmounted(() => {
    runId += 1;
    handle?.cancel();
    handle = null;
  });

  return { data, result, error, streaming, cancel, start };
}

export interface LxChannelOptions {
  params?: unknown | (() => unknown);
  manual?: boolean;
}

export interface LxChannelState<TData, TOut = TData> {
  last: Ref<TData | undefined>;
  error: Ref<LxBridgeError | undefined>;
  connecting: Ref<boolean>;
  connected: Ref<boolean>;
  send: (payload: TOut) => void;
  close: (code?: string, reason?: string) => void;
  reopen: () => void;
}

export function useLxChannel<
  TMethod extends (...args: any[]) => Promise<LxChannel<any, any>>,
>(
  method: MethodSource<TMethod>,
  options?: LxChannelOptions & {
    params?: ParamsSource<MethodParams<TMethod>>;
  },
): LxChannelState<ChannelIn<TMethod>, ChannelOut<TMethod>> {
  type TIn = ChannelIn<TMethod>;
  type TOut = ChannelOut<TMethod>;

  const last = ref<TIn | undefined>(undefined) as Ref<TIn | undefined>;
  const error = ref<LxBridgeError | undefined>(undefined);
  const connecting = ref(false);
  const connected = ref(false);

  let ch: LxChannel<TIn, TOut> | null = null;
  let runId = 0;

  function send(payload: TOut): void {
    ch?.send(payload);
  }

  function close(code?: string, reason?: string): void {
    runId += 1;
    ch?.close(code, reason);
    ch = null;
    connecting.value = false;
    connected.value = false;
  }

  function reopen(): void {
    ch?.close();
    ch = null;

    const thisRunId = ++runId;
    last.value = undefined;
    error.value = undefined;
    connecting.value = true;
    connected.value = false;

    Promise.resolve(
      invokeMethod(resolveMethod(method), resolveParams(options?.params)) as Promise<LxChannel<TIn, TOut>>,
    )
      .then((nextChannel) => {
        if (runId !== thisRunId) {
          nextChannel.close();
          return;
        }
        ch = nextChannel;
        connecting.value = false;
        connected.value = true;

        nextChannel.on("data", (payload) => {
          if (runId !== thisRunId) return;
          last.value = payload as TIn;
        });

        nextChannel.on("close", () => {
          if (runId !== thisRunId) return;
          ch = null;
          connecting.value = false;
          connected.value = false;
        });

        nextChannel.on("error", (err: LxBridgeError) => {
          if (runId !== thisRunId) return;
          ch = null;
          error.value = err;
          connecting.value = false;
          connected.value = false;
        });
      })
      .catch((err: unknown) => {
        if (runId !== thisRunId) return;
        error.value = toBridgeError(err);
        connecting.value = false;
        connected.value = false;
      });
  }

  watch(
    () => {
      if (options?.manual) return null;
      const resolvedMethod = resolveMethod(method);
      return [
        getMethodKey(resolvedMethod) ?? resolvedMethod,
        stableParamKey(resolveParams(options?.params)),
      ];
    },
    () => {
      if (!options?.manual) {
        reopen();
      }
    },
    { immediate: !options?.manual },
  );

  onUnmounted(() => {
    runId += 1;
    ch?.close();
    ch = null;
  });

  return { last, error, connecting, connected, send, close, reopen };
}
