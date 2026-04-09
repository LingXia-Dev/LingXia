import type {
  DataSubscriber,
  LxBridgeError,
  LxChannel,
  LxStream,
  StateInfo,
} from "@lingxia/bridge";

export type ActionMap = Record<string, (...args: unknown[]) => unknown>;
export type Snapshot = Record<string, unknown>;
export type Listener = () => void;
export type ParamsSource<T> = T | (() => T);
type BridgeMode = "notify" | "call" | "stream";
type PageBridgeMetadata = {
  __names: string[];
  __modes?: Record<string, BridgeMode>;
  [key: string]: unknown;
};
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

let snapshot: Snapshot = {};
let stateInfo: StateInfo = { rev: -1, initial: true };
let subscribed = false;
let subscribeRetryTimer: ReturnType<typeof setTimeout> | null = null;
let initialSnapshotResolved = false;
let snapshotRequestInFlight = false;
const listeners = new Set<Listener>();

export function toBridgeError(err: unknown): LxBridgeError {
  return err && typeof err === "object" && "code" in err
    ? (err as LxBridgeError)
    : {
        code: "BRIDGE_INTERNAL_ERROR",
        message: err instanceof Error ? err.message : String(err),
      };
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

function notifyListeners(): void {
  listeners.forEach((listener) => {
    try {
      listener();
    } catch {
      // Ignore listener errors to avoid breaking state fanout.
    }
  });
}

function updateSnapshot(next: unknown, info: StateInfo): void {
  snapshot = next && typeof next === "object" ? (next as Snapshot) : {};
  stateInfo = info;
  notifyListeners();
}

function scheduleSubscribeRetry(): void {
  if (subscribeRetryTimer !== null || subscribed) return;
  subscribeRetryTimer = setTimeout(() => {
    subscribeRetryTimer = null;
    ensurePageBridgeSubscription();
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

export function ensurePageBridgeSubscription(): void {
  if (subscribed) return;
  const bridge = window.LingXiaBridge;
  const subscribeState = bridge?.state?.subscribe;
  if (!subscribeState) {
    scheduleSubscribeRetry();
    return;
  }
  subscribeState((next, info) => {
    updateSnapshot(next, info);
  });
  subscribed = true;
  requestInitialSnapshot(bridge);
}

export function subscribePageSnapshot(listener: Listener): () => void {
  ensurePageBridgeSubscription();
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

export function subscribePageData(
  callback: DataSubscriber,
): () => void {
  if (typeof callback !== "function") return () => {};
  ensurePageBridgeSubscription();

  if (stateInfo.rev >= 0) {
    callback(snapshot, { rev: stateInfo.rev, initial: true });
  }

  return subscribePageSnapshot(() => {
    callback(snapshot, stateInfo);
  });
}

export function getPageSnapshot<TData = Snapshot>(): TData {
  ensurePageBridgeSubscription();
  return snapshot as TData;
}

export function getPageActions<TActions extends ActionMap>(): TActions {
  const actions: ActionMap = {};
  const bridge = window.__pageBridge as PageBridgeMetadata | undefined;
  if (!bridge?.__names) {
    return actions as TActions;
  }

  for (const name of bridge.__names) {
    if (typeof name !== "string") continue;
    const fn = getOrCreatePageAction(bridge, name);
    if (typeof fn === "function") {
      actions[name] = fn;
    }
  }

  return actions as TActions;
}

export function getPageStateInfo(): StateInfo {
  return stateInfo;
}

function getOrCreatePageAction(
  bridge: PageBridgeMetadata,
  name: string,
): ((...args: unknown[]) => unknown) | undefined {
  const existing = bridge[name];
  if (typeof existing === "function") {
    return existing as (...args: unknown[]) => unknown;
  }

  const mode = resolvePageActionMode(bridge, name);
  const created = definePageBridgeAction(name, mode);
  bridge[name] = created;
  return created;
}

function resolvePageActionMode(
  bridge: PageBridgeMetadata,
  name: string,
): BridgeMode {
  const mode = bridge.__modes?.[name];
  return mode === "call" || mode === "stream" ? mode : "notify";
}

function definePageBridgeAction(
  name: string,
  mode: BridgeMode,
): (...args: unknown[]) => unknown {
  function action(...args: unknown[]): unknown {
    const payload = filterPayload(name, args);
    const bridge = window.LingXiaBridge;
    if (!bridge) {
      throw new Error(`LingXiaBridge is not ready for page action '${name}'`);
    }
    if (mode === "stream") {
      const handle = bridge.stream(name, payload);
      if (handle && handle.result && typeof handle.result.catch === "function") {
        handle.result.catch((err: unknown) => {
          console.warn(`[PageFunc] ${name} failed:`, err instanceof Error ? err.message : err);
        });
      }
      return handle;
    }
    if (mode === "call") {
      const promise = bridge.call(name, payload);
      if (promise && typeof promise.catch === "function") {
        promise.catch((err: unknown) => {
          console.warn(`[PageFunc] ${name} failed:`, err instanceof Error ? err.message : err);
        });
      }
      return promise;
    }
    bridge.notify(name, payload);
    return undefined;
  }

  Object.assign(action, {
    __logicFunc: true,
    __funcName: name,
    __bridgeMode: mode,
  });
  return action;
}

function filterPayload(name: string, args: unknown[]): unknown {
  const clean: unknown[] = [];
  for (const value of args) {
    if (value instanceof Event) continue;
    if (
      value &&
      typeof value === "object" &&
      "stopPropagation" in value &&
      typeof (value as { stopPropagation?: unknown }).stopPropagation === "function"
    ) {
      continue;
    }
    clean.push(value);
  }
  if (clean.length > 1) {
    throw new Error(`Page action '${name}' accepts at most one payload argument`);
  }
  return clean[0];
}
