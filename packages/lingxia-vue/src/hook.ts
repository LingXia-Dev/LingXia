import {
  onUnmounted,
  reactive,
  readonly,
  ref,
  unref,
  watch,
  type Ref,
} from "vue";
import type {
  LxChannel,
  LxBridgeError,
  LxStream,
} from "@lingxia/bridge";
import { getHost, subscribeHost, type LxHost } from "@lingxia/bridge";
import {
  getMethodKey,
  invokeMethod,
  resolveParams,
  stableParamKey,
  toBridgeError,
  type ChannelIn,
  type ChannelOut,
  type MethodParams,
  type ParamsSource,
  type StreamData,
  type StreamResult,
} from "@lingxia/bridge/invocation";
import {
  getPageActions,
  getPageSnapshot,
  subscribePageSnapshot,
  type ActionMap,
  type Snapshot,
} from "@lingxia/page-runtime";

type MethodSource<T> = T | Ref<T>;

function resolveMethod<TMethod>(source: MethodSource<TMethod>): TMethod {
  return unref(source) as TMethod;
}

// One reactive copy of the page's data for the document, updated in place so
// a destructured `data` stays live.
const reactiveSnapshot = reactive<Snapshot>({});
let snapshotSubscribed = false;

function syncSnapshot(): void {
  const next = getPageSnapshot<Snapshot>();
  const normalized: Snapshot = next && typeof next === "object" ? next : {};
  for (const key of Object.keys(reactiveSnapshot)) {
    if (!Object.prototype.hasOwnProperty.call(normalized, key)) {
      delete reactiveSnapshot[key];
    }
  }
  Object.assign(reactiveSnapshot, normalized);
}

/**
 * This page's Logic state and actions — `this.data` and the page's methods.
 * The page mounts once its first state has arrived, so `data` is whole from
 * the first render. `data` is deep-reactive and updated in place, so it may be
 * destructured; `actions` is one object for the page. Callable anywhere.
 */
export function useLxPage<
  TData = Snapshot,
  TActions extends ActionMap = ActionMap,
>(): { data: TData; actions: TActions } {
  if (!snapshotSubscribed) {
    snapshotSubscribed = true;
    syncSnapshot();
    subscribePageSnapshot(syncSnapshot);
  }
  return { data: reactiveSnapshot as TData, actions: getPageActions<TActions>() };
}

const hostState = reactive<LxHost>({ ...getHost() });
let hostSubscribed = false;

/**
 * What the host decided for this page: `sizeClass`, `aside`,
 * `displayLanguage`, `formFactor`, `os`, `runner` — a readonly reactive object
 * that changes only when one of them does, never while a window is dragged.
 * Read `host.sizeClass`; destructuring takes a snapshot. Geometry is CSS:
 * `--lx-page-chrome-*` and container queries. Callable anywhere.
 */
export function useLxHost(): Readonly<LxHost> {
  if (!hostSubscribed) {
    hostSubscribed = true;
    Object.assign(hostState, getHost());
    subscribeHost(() => Object.assign(hostState, getHost()));
  }
  return readonly(hostState) as Readonly<LxHost>;
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
