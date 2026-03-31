import * as React from "react";
import type {
  StreamHandle,
  Subscription as BridgeSubscription,
  Channel as BridgeChannel,
  LxBridgeError,
} from "@lingxia/bridge";

type ActionMap = Record<string, (...args: unknown[]) => unknown>;
type Snapshot = Record<string, unknown>;
type Listener = () => void;

let snapshot: Snapshot = {};
let subscribed = false;
let subscribeRetryTimer: ReturnType<typeof setTimeout> | null = null;
let initialSnapshotResolved = false;
let snapshotRequestInFlight = false;
const listeners = new Set<Listener>();

function getParamsSignature(params: unknown): string {
  try {
    return JSON.stringify(params) ?? "undefined";
  } catch {
    return "__nonserializable__";
  }
}

function toBridgeError(
  err: unknown,
  fallbackCode: string,
  fallbackMessage: string,
): LxBridgeError {
  return err && typeof err === "object" && "code" in err
    ? (err as LxBridgeError)
    : { code: fallbackCode, message: String(err ?? fallbackMessage) };
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
    // Pull latest snapshot that may arrive before this component subscribes.
    setVersion((v) => v + 1);
    return () => {
      listeners.delete(listener);
    };
  }, []);

  const actions = React.useMemo(() => resolveActions<TActions>(), []);
  return { data: snapshot as TData, actions };
}

export interface LxStreamOptions<TData, TReduced> {
  /** Don't auto-call the factory on mount. Use `call()` to start manually. */
  manual?: boolean;
  /** Accumulate chunks into a single value. */
  reduce?: (accumulated: TReduced, chunk: TData) => TReduced;
  /** Initial value for the accumulator. Required when `reduce` is provided. */
  initial?: TReduced;
}

export interface LxStreamState<TData, TResult = unknown> {
  /** Latest chunk (no reduce) or accumulated value (with reduce). */
  data: TData | undefined;
  /** Final result when the stream completes. */
  result: TResult | undefined;
  /** Stream error, if any. */
  error: LxBridgeError | undefined;
  /** Whether the stream is currently active. */
  streaming: boolean;
  /** Cancel the active stream. */
  cancel: () => void;
  /** Start (or restart) the stream. Only useful with `manual: true`. */
  call: () => void;
}

export function useLxStream<TData = unknown, TResult = unknown, TReduced = TData>(
  factory: () => StreamHandle<TData, TResult>,
  options?: LxStreamOptions<TData, TReduced>,
): LxStreamState<TReduced extends TData ? TData : TReduced, TResult> {
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

  const handleRef = React.useRef<StreamHandle<TData, TResult> | null>(null);
  const accRef = React.useRef<TReduced | undefined>(options?.initial);
  const optionsRef = React.useRef(options);
  const factoryRef = React.useRef(factory);
  const runIdRef = React.useRef(0);
  factoryRef.current = factory;
  optionsRef.current = options;

  const cancel = React.useCallback(() => {
    runIdRef.current += 1;
    handleRef.current?.cancel();
    handleRef.current = null;
    setState((prev) => ({ ...prev, streaming: false }));
  }, []);

  const call = React.useCallback(() => {
    // Cancel any previous stream.
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

    let handle: StreamHandle<TData, TResult>;
    try {
      handle = factoryRef.current();
    } catch (err: unknown) {
      if (runIdRef.current !== runId) return;
      handleRef.current = null;
      setState((prev) => ({
        ...prev,
        error: toBridgeError(err, "STREAM_CALL_FAILED", "Failed to start stream"),
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

  // Auto-start on mount when not manual.
  React.useEffect(() => {
    if (!options?.manual) {
      call();
    }
    return () => {
      runIdRef.current += 1;
      handleRef.current?.cancel();
      handleRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  return { ...state, cancel, call };
}

export interface LxSubscriptionOptions {
  params?: Record<string, unknown>;
}

export interface LxSubscriptionState<TData> {
  data: TData | undefined;
  error: LxBridgeError | undefined;
  active: boolean;
  close: () => void;
}

export function useLxSubscription<TData = unknown>(
  topic: string,
  options?: LxSubscriptionOptions,
): LxSubscriptionState<TData> {
  const [state, setState] = React.useState<{
    data: TData | undefined;
    error: LxBridgeError | undefined;
    active: boolean;
  }>({
    data: undefined,
    error: undefined,
    active: false,
  });

  const subRef = React.useRef<BridgeSubscription<TData> | null>(null);
  const paramsSignature = getParamsSignature(options?.params);

  const close = React.useCallback(() => {
    subRef.current?.close();
    subRef.current = null;
    setState((prev) => ({ ...prev, active: false }));
  }, []);

  React.useEffect(() => {
    const bridge = window.LingXiaBridge;
    if (!bridge?.subscribe) return;

    let cancelled = false;
    setState((prev) => ({ ...prev, error: undefined, active: false }));
    bridge.subscribe<TData>(topic, options?.params)
      .then((sub) => {
        if (cancelled) {
          sub.close();
          return;
        }
        subRef.current = sub;
        setState((prev) => ({ ...prev, active: true }));

        sub.on("data", (payload) => {
          setState((prev) => ({ ...prev, data: payload }));
        });
        sub.on("error", (err: LxBridgeError) => {
          subRef.current = null;
          setState((prev) => ({ ...prev, error: err, active: false }));
        });
      })
      .catch((err: unknown) => {
        const error = toBridgeError(
          err,
          "SUBSCRIBE_FAILED",
          "Failed to subscribe",
        );
        setState((prev) => ({ ...prev, error, active: false }));
      });

    return () => {
      cancelled = true;
      subRef.current?.close();
      subRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [topic, paramsSignature]);

  return { ...state, close };
}

export interface LxChannelOptions {
  params?: Record<string, unknown>;
}

export interface LxChannelState<TData> {
  data: TData | undefined;
  error: LxBridgeError | undefined;
  connected: boolean;
  send: (payload: unknown) => void;
  close: (code?: string, reason?: string) => void;
}

export function useLxChannel<TData = unknown>(
  topic: string,
  options?: LxChannelOptions,
): LxChannelState<TData> {
  const [state, setState] = React.useState<{
    data: TData | undefined;
    error: LxBridgeError | undefined;
    connected: boolean;
  }>({
    data: undefined,
    error: undefined,
    connected: false,
  });

  const chRef = React.useRef<BridgeChannel<TData> | null>(null);
  const paramsSignature = getParamsSignature(options?.params);

  const send = React.useCallback((payload: unknown) => {
    chRef.current?.send(payload);
  }, []);

  const close = React.useCallback((code?: string, reason?: string) => {
    chRef.current?.close(code, reason);
    chRef.current = null;
    setState((prev) => ({ ...prev, connected: false }));
  }, []);

  React.useEffect(() => {
    const bridge = window.LingXiaBridge;
    if (!bridge?.channel?.open) return;

    let cancelled = false;
    setState((prev) => ({ ...prev, error: undefined, connected: false }));
    bridge.channel.open<TData>(topic, options?.params)
      .then((ch) => {
        if (cancelled) {
          ch.close();
          return;
        }
        chRef.current = ch;
        setState((prev) => ({ ...prev, connected: true }));

        ch.on("data", (payload) => {
          setState((prev) => ({ ...prev, data: payload }));
        });
        ch.on("close", () => {
          chRef.current = null;
          setState((prev) => ({ ...prev, connected: false }));
        });
        ch.on("error", (err: LxBridgeError) => {
          chRef.current = null;
          setState((prev) => ({ ...prev, error: err, connected: false }));
        });
      })
      .catch((err: unknown) => {
        const error = toBridgeError(
          err,
          "CHANNEL_OPEN_FAILED",
          "Failed to open channel",
        );
        setState((prev) => ({ ...prev, error, connected: false }));
      });

    return () => {
      cancelled = true;
      chRef.current?.close();
      chRef.current = null;
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [topic, paramsSignature]);

  return { ...state, send, close };
}
