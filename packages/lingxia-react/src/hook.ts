import * as React from "react";
import type {
  LxChannel,
  LxBridgeError,
  LxStream,
} from "@lingxia/bridge";
import {
  getHost,
  holdLeaveGuard,
  subscribeHost,
  type LxHost,
  type LxLeaveHandler,
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

/**
 * This page's Logic state and actions — `this.data` and the page's methods.
 * The page mounts once its first state has arrived, so `data` is whole from
 * the first render; it follows every `setData`. `data` is readonly: Logic
 * owns it (in a dev session a write throws). `actions` is one object for the
 * page.
 */
export function useLxPage<
  TData = Snapshot,
  TActions extends ActionMap = ActionMap,
>(): LxPage<TData, TActions> {
  const data = React.useSyncExternalStore(
    subscribePageSnapshot,
    getPageSnapshot<DeepReadonly<TData>>,
    getPageSnapshot<DeepReadonly<TData>>,
  );
  return { data, actions: getPageActions<TActions>() } as LxPage<TData, TActions>;
}

/**
 * `useLxPage<typeof page>()` with the page's `definePage` type imported
 * (`import type page from './index.ts'`), or `useLxPage<Data, Actions>()`.
 */
export type LxPage<TData, TActions extends ActionMap> = TData extends {
  readonly __lxPage: unknown;
  readonly data: infer D;
  readonly actions: infer A extends ActionMap;
}
  ? { data: DeepReadonly<D>; actions: PageActions<A> }
  : { data: DeepReadonly<TData>; actions: PageActions<TActions> };

/**
 * What the host decided for this page: `sizeClass`, `canDock`,
 * `displayLanguage`, `contentLanguage`, `formFactor`, `os`, `runner`. Re-renders only when one
 * of them changes — never while a window is dragged. Geometry is CSS:
 * `--lx-page-chrome-*` and container queries.
 */
export function useLxHost(): LxHost {
  return React.useSyncExternalStore(subscribeHost, getHost, getHost);
}

/**
 * Hold the page against user back and home while `dirty` — unsaved changes.
 * When the user tries to leave, `onRequest` is asked: return `true` to let
 * them go, `false` to stay (or a promise of either, after a dialog).
 * Released when `dirty` turns false or the component unmounts.
 */
export function useLxLeaveGuard(dirty: boolean, onRequest: LxLeaveHandler): void {
  const latest = React.useRef(onRequest);
  React.useEffect(() => {
    latest.current = onRequest;
  });
  React.useEffect(
    () => (dirty ? holdLeaveGuard((request) => latest.current(request)) : undefined),
    [dirty],
  );
}

type StreamMethod = (...args: any[]) => LxStream<any, any>;
type ChannelMethod = (...args: any[]) => Promise<LxChannel<any, any>>;

/** The Logic end of a page channel, shaped as `PageChannel<Send, Receive>`. */
interface PageChannelEnd<Send, Receive> {
  send(payload: Send): void;
  close(code?: string, reason?: string): void;
  on(event: "data", handler: (payload: Receive) => void): () => void;
  on(event: "close", handler: (info: { code: string; reason: string }) => void): () => void;
}

export type LxStreamOptions<TMethod> = ParamsOption<TMethod, ParamsSource<MethodParams<TMethod>>> & {
  manual?: boolean;
};

/** A reducer folds chunks into `data`, starting from `initial`. */
export type LxReducedStreamOptions<TMethod, TReduced> = LxStreamOptions<TMethod> & {
  reduce: (accumulated: TReduced, chunk: StreamData<TMethod>) => TReduced;
  initial: TReduced;
};

export interface LxStreamState<TData, TResult = unknown> {
  data: TData | undefined;
  result: TResult | undefined;
  error: LxBridgeError | undefined;
  streaming: boolean;
  cancel: () => void;
  start: () => void;
}

export function useLxStream<TMethod extends StreamMethod, TReduced>(
  method: TMethod,
  options: LxReducedStreamOptions<TMethod, TReduced>,
): LxStreamState<TReduced, StreamResult<TMethod>>;
export function useLxStream<TMethod extends StreamMethod>(
  method: TMethod,
  ...options: OptionsArg<TMethod, LxStreamOptions<TMethod>>
): LxStreamState<StreamData<TMethod>, StreamResult<TMethod>>;
export function useLxStream(
  method: StreamMethod,
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

export type LxChannelOptions<TMethod> = ParamsOption<TMethod, ParamsSource<MethodParams<TMethod>>> & {
  manual?: boolean;
};

export interface LxChannelState<TData, TOut = TData> {
  last: TData | undefined;
  error: LxBridgeError | undefined;
  connecting: boolean;
  connected: boolean;
  /** Sends when connected; `false` when there is no open channel to send on. */
  send: (payload: TOut) => boolean;
  close: (code?: string, reason?: string) => void;
  reopen: () => void;
}

// A page action crosses as JSON, so one passed here is the Logic end of a channel, opened by name.
function openChannel(method: (...args: any[]) => unknown, params: unknown): Promise<LxChannel<unknown, unknown>> {
  const name = (method as { __logicFunc?: unknown }).__logicFunc === true ? getMethodKey(method) : undefined;
  if (name === undefined) return invokeMethod(method, params) as Promise<LxChannel<unknown, unknown>>;
  const channel = window.LingXiaBridge?.raw?.channel;
  if (!channel?.open) return Promise.reject(new Error("LingXiaBridge channel API is not ready"));
  return channel.open(name, params);
}

export function useLxChannel<TMethod extends ChannelMethod>(
  method: TMethod,
  ...args: OptionsArg<TMethod, LxChannelOptions<TMethod>>
): LxChannelState<ChannelIn<TMethod>, ChannelOut<TMethod>>;
/**
 * A page action written `(params, channel: PageChannel<Send, Receive>)` in
 * Logic opens as a channel: `last` is what Logic sends, `send` takes what it receives.
 */
export function useLxChannel<P, TIn, TOut>(
  action: (params: P, channel: PageChannelEnd<TIn, TOut>) => unknown,
  ...args: undefined extends P
    ? [options?: { params?: ParamsSource<P>; manual?: boolean }]
    : [options: { params: ParamsSource<P>; manual?: boolean }]
): LxChannelState<TIn, TOut>;
export function useLxChannel(
  method: (...args: any[]) => unknown,
  options?: { params?: ParamsSource<unknown>; manual?: boolean },
): LxChannelState<unknown, unknown> {
  type TIn = unknown;
  type TOut = unknown;

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
    const ch = chRef.current;
    if (!ch) return false;
    ch.send(payload);
    return true;
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

    // In the executor: a method that throws synchronously lands in `.catch`.
    new Promise<LxChannel<TIn, TOut>>((resolve) => {
      resolve(openChannel(methodRef.current, paramsRef.current));
    })
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
