import * as React from "react";
import type {
  LxChannel,
  LxBridgeError,
  LxStream,
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
  type ParamsSource,
  type StreamData,
  type StreamResult,
} from "@lingxia/bridge/invocation";
import {
  ensurePageBridgeSubscription,
  getPageActions,
  getPageSnapshot,
  subscribePageSnapshot,
  type ActionMap,
  type Snapshot,
} from "@lingxia/page-runtime";

export function useLxPage<
  TData = Snapshot,
  TActions extends ActionMap = ActionMap,
>(): { data: TData; actions: TActions } {
  ensurePageBridgeSubscription();
  const [, setVersion] = React.useState(0);

  React.useEffect(() => {
    ensurePageBridgeSubscription();
    const listener = () => setVersion((v) => v + 1);
    const unsubscribe = subscribePageSnapshot(listener);
    setVersion((v) => v + 1);
    return unsubscribe;
  }, []);

  const actions = React.useMemo(() => getPageActions<TActions>(), []);
  return { data: getPageSnapshot<TData>(), actions };
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

export interface LxPlatform {
  os: string;
  isIOS: boolean;
  isMacOS: boolean;
  isApple: boolean;
  isAndroid: boolean;
  isHarmony: boolean;
  isWindows: boolean;
  isDesktop: boolean;
  isRunner: boolean;
}

function readPlatform(): LxPlatform {
  const p = window.LingXiaBridge?.platform;
  return {
    os: p?.getOS() ?? "unknown",
    isIOS: p?.isIOS() ?? false,
    isMacOS: p?.isMacOS() ?? false,
    isApple: p?.isApple() ?? false,
    isAndroid: p?.isAndroid() ?? false,
    isHarmony: p?.isHarmony() ?? false,
    isWindows: p?.isWindows() ?? false,
    isDesktop: p?.isDesktop() ?? false,
    isRunner: p?.isRunner() ?? false,
  };
}

// Typed platform detection for pages, so they never reach for the window global
// or hand-roll an OS check. Fixed for the session, so it resolves once.
export function usePlatform(): LxPlatform {
  return React.useMemo(readPlatform, []);
}
