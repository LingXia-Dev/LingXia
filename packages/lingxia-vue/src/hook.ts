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
import {
  getHost,
  setLeaveGuard,
  subscribeBackRequest,
  subscribeHost,
  type LxHost,
} from "@lingxia/bridge";
import {
  getMethodKey,
  invokeMethod,
  resolveParams,
  stableParamKey,
  toBridgeError,
  type ChannelIn,
  type ChannelOut,
  type MethodParams,
  type OptionsArg,
  type ParamsOption,
  type ParamsSource,
  type StreamData,
  type StreamResult,
} from "@lingxia/bridge/invocation";
import {
  getPageActions,
  getPageSnapshot,
  subscribePageSnapshot,
  type ActionMap,
  type PageActions,
  type DeepReadonly,
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

// What pages get: Logic owns the data, so a View write is refused (Vue warns
// in development) while the sync above keeps updating the same object.
const readonlySnapshot = readonly(reactiveSnapshot);

/**
 * This page's Logic state and actions — `this.data` and the page's methods.
 * The page mounts once its first state has arrived, so `data` is whole from
 * the first render. `data` is deep-reactive, readonly and updated in place, so
 * it may be destructured; keep editable state in a `ref` and send it with an
 * action. Call during component setup; pass actions into shared helpers
 * instead of reading at module load.
 */
export function useLxPage<
  TData = Snapshot,
  TActions extends ActionMap = ActionMap,
>(): { data: DeepReadonly<TData>; actions: PageActions<TActions> } {
  if (!snapshotSubscribed) {
    snapshotSubscribed = true;
    syncSnapshot();
    subscribePageSnapshot(syncSnapshot);
  }
  return {
    data: readonlySnapshot as unknown as DeepReadonly<TData>,
    actions: getPageActions<TActions>(),
  };
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

/**
 * Hold the page against user back while `dirty` — unsaved changes. A back
 * from the navigation bar, the system, or an edge swipe then leaves the page
 * where it is and calls `onRequest`; confirm there, and leave with
 * `leavePage()` (which does not ask again). Released when `dirty` turns false
 * or the component unmounts. Call from `setup`.
 */
export function useLxLeaveGuard(
  dirty: Ref<boolean> | (() => boolean),
  onRequest: () => void,
): void {
  // Only a change reaches the host: a clean form mounts without a round trip.
  let held = false;
  const stop = watch(
    () => (typeof dirty === "function" ? dirty() : unref(dirty)),
    (value) => {
      if (value === held) return;
      held = value;
      void setLeaveGuard(value).catch(warnLeaveGuard);
    },
    { immediate: true },
  );
  const unsubscribe = subscribeBackRequest(onRequest);
  onUnmounted(() => {
    stop();
    unsubscribe();
    if (held) {
      held = false;
      void setLeaveGuard(false).catch(warnLeaveGuard);
    }
  });
}

// A refused guard (bridge not ready, a runtime without the route) leaves the
// page unguarded; say so rather than fail silently.
function warnLeaveGuard(error: unknown): void {
  console.warn("[lingxia] setLeaveGuard failed; the page is not guarded", error);
}

type StreamMethod = (...args: any[]) => LxStream<any, any>;
type ChannelMethod = (...args: any[]) => Promise<LxChannel<any, any>>;

export type LxStreamOptions<TMethod> = ParamsOption<TMethod, ParamsSource<MethodParams<TMethod>>> & {
  manual?: boolean;
};

/** A reducer folds chunks into `data`, starting from `initial`. */
export type LxReducedStreamOptions<TMethod, TReduced> = LxStreamOptions<TMethod> & {
  reduce: (accumulated: TReduced, chunk: StreamData<TMethod>) => TReduced;
  initial: TReduced;
};

export interface LxStreamState<TData, TResult = unknown> {
  data: Ref<TData | undefined>;
  result: Ref<TResult | undefined>;
  error: Ref<LxBridgeError | undefined>;
  streaming: Ref<boolean>;
  cancel: () => void;
  start: () => void;
}

export function useLxStream<TMethod extends StreamMethod, TReduced>(
  method: MethodSource<TMethod>,
  options: LxReducedStreamOptions<TMethod, TReduced>,
): LxStreamState<TReduced, StreamResult<TMethod>>;
export function useLxStream<TMethod extends StreamMethod>(
  method: MethodSource<TMethod>,
  ...options: OptionsArg<TMethod, LxStreamOptions<TMethod>>
): LxStreamState<StreamData<TMethod>, StreamResult<TMethod>>;
export function useLxStream(
  method: MethodSource<StreamMethod>,
  options?: {
    params?: ParamsSource<unknown>;
    manual?: boolean;
    reduce?: (accumulated: unknown, chunk: unknown) => unknown;
    initial?: unknown;
  },
): LxStreamState<unknown, unknown> {
  type TData = unknown;
  type TResult = unknown;
  type TReduced = unknown;
  type TOut = unknown;

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

export type LxChannelOptions<TMethod> = ParamsOption<TMethod, ParamsSource<MethodParams<TMethod>>> & {
  manual?: boolean;
};

export interface LxChannelState<TData, TOut = TData> {
  last: Ref<TData | undefined>;
  error: Ref<LxBridgeError | undefined>;
  connecting: Ref<boolean>;
  connected: Ref<boolean>;
  /** Sends when connected; `false` when there is no open channel to send on. */
  send: (payload: TOut) => boolean;
  close: (code?: string, reason?: string) => void;
  reopen: () => void;
}

export function useLxChannel<TMethod extends ChannelMethod>(
  method: MethodSource<TMethod>,
  ...args: OptionsArg<TMethod, LxChannelOptions<TMethod>>
): LxChannelState<ChannelIn<TMethod>, ChannelOut<TMethod>> {
  type TIn = ChannelIn<TMethod>;
  type TOut = ChannelOut<TMethod>;
  const options = args[0] as { params?: ParamsSource<unknown>; manual?: boolean } | undefined;

  const last = ref<TIn | undefined>(undefined) as Ref<TIn | undefined>;
  const error = ref<LxBridgeError | undefined>(undefined);
  const connecting = ref(false);
  const connected = ref(false);

  let ch: LxChannel<TIn, TOut> | null = null;
  let runId = 0;

  function send(payload: TOut): boolean {
    if (!ch) return false;
    ch.send(payload);
    return true;
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

    // In the executor: a method or params getter that throws synchronously
    // lands in `.catch`.
    new Promise<LxChannel<TIn, TOut>>((resolve) => {
      resolve(
        invokeMethod(resolveMethod(method), resolveParams(options?.params)) as Promise<LxChannel<TIn, TOut>>,
      );
    })
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
