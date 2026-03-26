import type {
  CallOptions,
  Channel,
  ChannelOpenOptions,
  DataSubscriber,
  HostApi,
  LingXiaBridgeInterface,
  LxBridgeError,
  LxMethod,
  LxMethodParams,
  LxMethodResult,
  LxMethodStreamData,
  NativeComponentMessage,
  NotifyOptions,
  StreamCallOptions,
  StreamHandle,
  Subscription,
  SubscribeOptions,
} from "./types";
import { BRIDGE_ERROR } from "./types";
import { installNativeComponentCoverageMonitor } from "./nativecomponents/coverage-monitor";
import {
  BRIDGE_CONFIG,
  getCommunicationMethod,
  getPlatformOS,
  isAndroid,
  isHarmony,
  isIOS,
  isMacOS,
  isDesktop,
} from "./runtime-env";

const NATIVE_HANDLER_NAME = "LingXia";
const GLOBAL_RECEIVER_NAME = "__LingXiaRecvMessage";
const DEFAULT_TIMEOUT_MS = 5000;
const HANDSHAKE_TIMEOUT_MS = 10000;
const HANDSHAKE_MAX_RETRIES = 3;
const LOG_PREFIX = "[LX.Bridge]";
const MESSAGE_PORT_TYPE = "messageport";
const JS_INTERFACE_TYPE = "jsinterface";
const OUTBOX_LIMIT = 256;

const debugFlags = { data: false, proto: false, all: false };
const earlyNativeMessages: string[] = [];

function installEarlyReceiver(): void {
  if (typeof window === "undefined") return;
  if (typeof window[GLOBAL_RECEIVER_NAME] === "function") return;
  window[GLOBAL_RECEIVER_NAME] = (message: string): void => {
    earlyNativeMessages.push(message);
  };
}

function activateReceiver(receiver: (message: string) => void): void {
  if (typeof window === "undefined") return;
  window[GLOBAL_RECEIVER_NAME] = receiver;
  if (earlyNativeMessages.length === 0) return;

  const queued = earlyNativeMessages.splice(0, earlyNativeMessages.length);
  for (const message of queued) {
    try {
      receiver(message);
    } catch (err) {
      warn("Failed to replay queued native message:", err);
    }
  }
}

function isDebugEnabled(flag: keyof typeof debugFlags): boolean {
  return debugFlags.all || debugFlags[flag];
}

function log(...args: unknown[]): void {
  console.log(LOG_PREFIX, ...args);
}
function warn(...args: unknown[]): void {
  console.warn(LOG_PREFIX, ...args);
}
function error(...args: unknown[]): void {
  console.error(LOG_PREFIX, ...args);
}

function safeStringify(obj: unknown, space?: number): string {
  const seen = new WeakSet();
  return JSON.stringify(
    obj,
    (_key, value) => {
      if (typeof value === "object" && value !== null) {
        if (seen.has(value)) return "[Circular]";
        seen.add(value);
      }
      return value;
    },
    space,
  );
}

function stringifyForNative(obj: unknown): string {
  try {
    return JSON.stringify(obj);
  } catch {
    return safeStringify(obj);
  }
}

function deepCopy<T>(data: T): T {
  try {
    if (typeof structuredClone === "function") return structuredClone(data);
    return JSON.parse(JSON.stringify(data));
  } catch {
    return {} as T;
  }
}

function unknownToError(err: unknown, fallbackMsg: string): LxBridgeError {
  if (err && typeof err === "object") {
    const source = err as { code?: unknown; message?: unknown; data?: unknown };
    let code: string | number = BRIDGE_ERROR.INTERNAL_ERROR;
    if (typeof source.code === "string" && source.code.trim() !== "") {
      code = source.code;
    } else if (
      typeof source.code === "number" &&
      Number.isFinite(source.code)
    ) {
      code = source.code;
    }
    const message =
      typeof source.message === "string" && source.message.trim() !== ""
        ? source.message
        : fallbackMsg;
    const output: LxBridgeError = { code, message };
    if ("data" in source) output.data = source.data;
    return output;
  }

  const message =
    err instanceof Error
      ? err.message
      : typeof err === "string"
        ? err
        : fallbackMsg;
  return { code: BRIDGE_ERROR.INTERNAL_ERROR, message };
}

const communicationMethod = getCommunicationMethod();

installEarlyReceiver();

// Transport
let messagePort: MessagePort | null = null;
const portInitState = {
  listenerInstalled: false,
  promise: null as Promise<MessagePort> | null,
  resolve: null as ((port: MessagePort) => void) | null,
  reject: null as ((err: unknown) => void) | null,
  timer: null as number | null,
};
const portInitMessageListener: EventListenerObject = {
  handleEvent: (event: globalThis.Event): void => {
    const messageEvent = event as unknown as MessageEvent;
    if (messageEvent.data !== "LingXia-port-init") return;
    const port = messageEvent.ports?.[0];
    if (!port) return;
    LingXiaBridge._connectWebMessagePort(port);
    portInitState.resolve?.(port);
  },
};

function installMessagePortInitListener(): void {
  if (portInitState.listenerInstalled) return;
  portInitState.listenerInstalled = true;
  window.addEventListener("message", portInitMessageListener);
}

function cleanupPortInit(): void {
  if (portInitState.timer !== null) {
    window.clearTimeout(portInitState.timer);
    portInitState.timer = null;
  }
  portInitState.resolve = null;
  portInitState.reject = null;
  portInitState.promise = null;
}

function getMessagePort(): Promise<MessagePort> {
  if (messagePort) return Promise.resolve(messagePort);
  if (portInitState.promise) return portInitState.promise;

  const timeoutMs = 5000;
  installMessagePortInitListener();

  portInitState.promise = new Promise<MessagePort>((resolve, reject) => {
    portInitState.resolve = (port: MessagePort): void => {
      cleanupPortInit();
      resolve(port);
    };
    portInitState.reject = (err: unknown): void => {
      cleanupPortInit();
      reject(err);
    };
    portInitState.timer = window.setTimeout(() => {
      portInitState.reject?.(
        new Error(`MessagePort init timed out after ${timeoutMs}ms`),
      );
    }, timeoutMs);
  });

  try {
    window.LingXiaProxy?.getPort("LingXiaPort");
  } catch (e) {
    portInitState.reject?.(e);
  }

  return portInitState.promise;
}

function postToNative(message: unknown): void {
  const kind = (message as { kind?: string }).kind;
  if (kind === "req" || kind === "notify")
    log(`postToNative: ${kind} ${(message as { method?: string }).method}`);
  if (isDebugEnabled("proto"))
    console.log("→", JSON.stringify(message, null, 2));
  try {
    if (communicationMethod === "webkit") {
      window.webkit?.messageHandlers[NATIVE_HANDLER_NAME]?.postMessage(message);
      return;
    }
    const messageString = stringifyForNative(message);
    if (communicationMethod === MESSAGE_PORT_TYPE && messagePort) {
      messagePort.postMessage(messageString);
      return;
    }
    if (
      communicationMethod === JS_INTERFACE_TYPE &&
      window.LingXiaProxy?.postMessage
    ) {
      window.LingXiaProxy.postMessage(messageString);
      return;
    }
    warn("Transport not ready");
  } catch (e) {
    error("Send error:", e);
  }
}

// V2 Protocol Types
type Trace = { traceId?: string; spanId?: string };
type Hello = {
  v: 2;
  kind: "hello";
  nonce: string;
  role: "view";
  protocolsSupported: number[];
  trace?: Trace;
};
type HelloAck = {
  v: 2;
  kind: "helloAck";
  nonce: string;
  protocol: number;
  sessionId: string;
};
type Ready = { v: 2; kind: "ready"; sessionId: string; hostMethods?: Record<string, string> };
type Req = {
  v: 2;
  kind: "req";
  id: string;
  method: string;
  params?: unknown;
  cap: string;
  trace?: Trace;
};
type Res = {
  v: 2;
  kind: "res";
  id: string;
  ok: boolean;
  result?: unknown;
  error?: LxBridgeError;
  trace?: Trace;
};
type Notify = {
  v: 2;
  kind: "notify";
  method: string;
  params?: unknown;
  cap: string;
  trace?: Trace;
};
type Cancel = { v: 2; kind: "cancel"; id: string };
type StreamEventMsg = {
  v: 2;
  kind: "event";
  id: string;
  seq: number;
  payload: unknown;
  trace?: Trace;
};
type Sub = {
  v: 2;
  kind: "sub";
  id: string;
  topic: string;
  params?: unknown;
  cap: string;
  trace?: Trace;
};
type Unsub = { v: 2; kind: "unsub"; id: string };
type SubClose = {
  v: 2;
  kind: "sub.close";
  id: string;
  error?: LxBridgeError;
  trace?: Trace;
};
type StateSnapshot = {
  v: 2;
  kind: "state.snapshot";
  scope?: string;
  rev: number;
  state: Record<string, unknown>;
};
type JsonPatchOp =
  | { op: "add"; path: string; value: unknown }
  | { op: "replace"; path: string; value: unknown }
  | { op: "remove"; path: string };
type StatePatch = {
  v: 2;
  kind: "state.patch";
  scope?: string;
  baseRev: number;
  rev: number;
  ops: JsonPatchOp[];
  ack?: boolean;
};
type StateAck = { v: 2; kind: "state.ack"; scope?: string; rev: number };
type ChOpen = {
  v: 2;
  kind: "ch.open";
  id: string;
  topic: string;
  params?: unknown;
  cap: string;
  trace?: Trace;
};
type ChAck = {
  v: 2;
  kind: "ch.ack";
  id: string;
  ok: boolean;
  error?: LxBridgeError;
  trace?: Trace;
};
type ChData = {
  v: 2;
  kind: "ch.data";
  id: string;
  seq: number;
  payload: unknown;
  trace?: Trace;
};
type ChClose = {
  v: 2;
  kind: "ch.close";
  id: string;
  code?: string;
  reason?: string;
  trace?: Trace;
};
type Incoming =
  | HelloAck
  | Ready
  | Res
  | Req
  | StreamEventMsg
  | SubClose
  | StateSnapshot
  | StatePatch
  | ChAck
  | ChData
  | ChClose;

// Handshake state
let handshakeSessionId: string | null = null;
let handshakeDone = false;
let helloSent = false;
let handshakeRetryCount = 0;
let handshakeTimer: ReturnType<typeof setTimeout> | null = null;

// Host method schema — populated from handshake `Ready` message.
// Maps "namespace.method" → "call" | "stream".
const hostMethodKinds: Record<string, string> = {};

// Request tracking
let requestCounter = 0;
type BridgeListenerBuckets = {
  data: Set<(payload: unknown) => void>;
  end: Set<(result: unknown) => void>;
  error: Set<(error: LxBridgeError) => void>;
  close: Set<(code?: string, reason?: string) => void>;
};

function createListenerBuckets(): BridgeListenerBuckets {
  return {
    data: new Set(),
    end: new Set(),
    error: new Set(),
    close: new Set(),
  };
}

type InternalStreamHandle = StreamHandle & {
  _emitData: (payload: unknown) => void;
  _resolve: (result: unknown) => void;
  _reject: (err: LxBridgeError) => void;
};

function createStreamHandle(
  id: string,
  cancelFn: () => void,
): InternalStreamHandle {
  const listeners = createListenerBuckets();
  let done = false;
  let resolveResult: (value: unknown) => void = () => {};
  let rejectResult: (reason: unknown) => void = () => {};
  const result = new Promise<unknown>((resolve, reject) => {
    resolveResult = resolve;
    rejectResult = reject;
  });

  const handle: InternalStreamHandle = {
    id,
    result,
    on(
      event: "data" | "end" | "error",
      listener:
        | ((payload: unknown) => void)
        | ((result: unknown) => void)
        | ((error: LxBridgeError) => void),
    ) {
      if (event === "data")
        listeners.data.add(listener as (payload: unknown) => void);
      if (event === "end")
        listeners.end.add(listener as (result: unknown) => void);
      if (event === "error")
        listeners.error.add(listener as (error: LxBridgeError) => void);
      return this;
    },
    cancel(): void {
      if (done) return;
      cancelFn();
    },
    _emitData(payload: unknown): void {
      if (done) return;
      for (const listener of listeners.data) {
        try {
          listener(payload);
        } catch (e) {
          warn("Stream data listener failed:", e);
        }
      }
    },
    _resolve(resultValue: unknown): void {
      if (done) return;
      done = true;
      resolveResult(resultValue);
      for (const listener of listeners.end) {
        try {
          listener(resultValue);
        } catch (e) {
          warn("Stream end listener failed:", e);
        }
      }
    },
    _reject(err: LxBridgeError): void {
      if (done) return;
      done = true;
      rejectResult(err);
      for (const listener of listeners.error) {
        try {
          listener(err);
        } catch (e) {
          warn("Stream error listener failed:", e);
        }
      }
    },
  };

  return handle;
}

function callHost(method: string, params?: unknown): Promise<unknown> {
  return LingXiaBridge.call(`host.${method}`, params, { cap: "host" });
}

type InternalSubscription = Subscription & {
  _emitData: (payload: unknown) => void;
  _reject: (err: LxBridgeError) => void;
  _markActive: () => void;
  _markInactive: () => void;
  _isActive: () => boolean;
};

function createSubscription(
  id: string,
  closeFn: () => void,
): InternalSubscription {
  const listeners = createListenerBuckets();
  let active = false;
  const subscription: InternalSubscription = {
    id,
    on(
      event: "data" | "error",
      listener: ((payload: unknown) => void) | ((error: LxBridgeError) => void),
    ) {
      if (event === "data")
        listeners.data.add(listener as (payload: unknown) => void);
      if (event === "error")
        listeners.error.add(listener as (error: LxBridgeError) => void);
      return this;
    },
    close(): void {
      if (!active) return;
      active = false;
      closeFn();
    },
    _emitData(payload: unknown): void {
      if (!active) return;
      for (const listener of listeners.data) {
        try {
          listener(payload);
        } catch (e) {
          warn("Subscription listener failed:", e);
        }
      }
    },
    _reject(err: LxBridgeError): void {
      for (const listener of listeners.error) {
        try {
          listener(err);
        } catch (e) {
          warn("Subscription error listener failed:", e);
        }
      }
    },
    _markActive(): void {
      active = true;
    },
    _markInactive(): void {
      active = false;
    },
    _isActive(): boolean {
      return active;
    },
  };
  return subscription;
}

type InternalChannel = Channel & {
  _emitData: (payload: unknown) => void;
  _emitClose: (code?: string, reason?: string) => void;
  _reject: (err: LxBridgeError) => void;
  _markOpen: () => void;
  _isOpen: () => boolean;
  _nextSeq: () => number;
};

function createChannel(
  id: string,
  sendFn: (payload: unknown, seq: number) => void,
  closeFn: (code?: string, reason?: string) => void,
): InternalChannel {
  const listeners = createListenerBuckets();
  let open = false;
  let outboundSeq = 0;
  const channel: InternalChannel = {
    id,
    send(payload: unknown): void {
      if (!open) {
        channel._reject({
          code: BRIDGE_ERROR.STREAM_CLOSED,
          message: `Channel '${id}' is closed`,
        });
        return;
      }
      sendFn(payload, outboundSeq++);
    },
    on(
      event: "data" | "close" | "error",
      listener:
        | ((payload: unknown) => void)
        | ((code?: string, reason?: string) => void)
        | ((error: LxBridgeError) => void),
    ) {
      if (event === "data")
        listeners.data.add(listener as (payload: unknown) => void);
      if (event === "close")
        listeners.close.add(
          listener as (code?: string, reason?: string) => void,
        );
      if (event === "error")
        listeners.error.add(listener as (error: LxBridgeError) => void);
      return this;
    },
    close(code?: string, reason?: string): void {
      if (!open) return;
      open = false;
      closeFn(code, reason);
      channel._emitClose(code, reason);
    },
    _emitData(payload: unknown): void {
      if (!open) return;
      for (const listener of listeners.data) {
        try {
          listener(payload);
        } catch (e) {
          warn("Channel listener failed:", e);
        }
      }
    },
    _emitClose(code?: string, reason?: string): void {
      for (const listener of listeners.close) {
        try {
          listener(code, reason);
        } catch (e) {
          warn("Channel close listener failed:", e);
        }
      }
    },
    _reject(err: LxBridgeError): void {
      for (const listener of listeners.error) {
        try {
          listener(err);
        } catch (e) {
          warn("Channel error listener failed:", e);
        }
      }
    },
    _markOpen(): void {
      open = true;
    },
    _isOpen(): boolean {
      return open;
    },
    _nextSeq(): number {
      return outboundSeq++;
    },
  };
  return channel;
}

interface PendingRequest {
  method: string;
  mode: "call" | "stream";
  resolve?: (v: unknown) => void;
  reject?: (e: LxBridgeError) => void;
  stream?: InternalStreamHandle;
  timeoutMs: number;
  timerId: ReturnType<typeof setTimeout> | null;
}
const pendingReq = new Map<string, PendingRequest>();
const pendingSubs = new Map<
  string,
  {
    topic: string;
    subscription: InternalSubscription;
    resolve: (subscription: Subscription) => void;
    reject: (err: LxBridgeError) => void;
    timerId: ReturnType<typeof setTimeout> | null;
  }
>();
const activeSubs = new Map<string, InternalSubscription>();
const pendingChannels = new Map<
  string,
  {
    topic: string;
    channel: InternalChannel;
    resolve: (channel: Channel) => void;
    reject: (err: LxBridgeError) => void;
    timerId: ReturnType<typeof setTimeout> | null;
  }
>();
const activeChannels = new Map<string, InternalChannel>();

// Outbox
interface OutboxItem {
  msg: unknown;
  reqId?: string;
}
const outbox: OutboxItem[] = [];

// State sync
let stateRev = -1;
let pageData: Record<string, unknown> = {};
const dataSubscribers = new Set<DataSubscriber>();
const subscriberInitStatus = new WeakMap<DataSubscriber, boolean>();

// Logic→View method handlers (for incoming req from native side)
const viewMethodHandlers = new Map<
  string,
  (params: unknown) => unknown | Promise<unknown>
>();

export function registerViewMethodHandler(
  method: string,
  handler: (params: unknown) => unknown | Promise<unknown>,
): void {
  viewMethodHandlers.set(method, handler);
}

function inferCap(method: string): string {
  if (method.startsWith("host.")) return "host";
  const i = method.indexOf(".");
  return i > 0 ? method.slice(0, i) : "page";
}

function normalizeParams(params: unknown): unknown | undefined {
  if (params === null || params === undefined) return undefined;
  return params;
}

function isTransportReady(): boolean {
  if (communicationMethod === MESSAGE_PORT_TYPE) return !!messagePort;
  return (
    communicationMethod === "webkit" ||
    communicationMethod === JS_INTERFACE_TYPE
  );
}

function canSendAppMessages(): boolean {
  return isTransportReady() && handshakeDone;
}

function rejectPendingRequest(reqId: string, err: LxBridgeError): void {
  const info = pendingReq.get(reqId);
  if (info) {
    pendingReq.delete(reqId);
    if (info.timerId !== null) clearTimeout(info.timerId);
    if (info.mode === "stream" && info.stream) {
      info.stream._reject(err);
      return;
    }
    info.reject?.(err);
  }
}

function rejectPendingSubscription(id: string, err: LxBridgeError): void {
  const pending = pendingSubs.get(id);
  if (!pending) return;
  pendingSubs.delete(id);
  if (pending.timerId !== null) clearTimeout(pending.timerId);
  pending.subscription._reject(err);
  pending.reject(err);
}

function rejectPendingChannel(id: string, err: LxBridgeError): void {
  const pending = pendingChannels.get(id);
  if (!pending) return;
  pendingChannels.delete(id);
  if (pending.timerId !== null) clearTimeout(pending.timerId);
  pending.channel._reject(err);
  pending.reject(err);
}

function rejectPendingOperation(id: string, err: LxBridgeError): void {
  if (pendingReq.has(id)) {
    rejectPendingRequest(id, err);
    return;
  }
  if (pendingSubs.has(id)) {
    rejectPendingSubscription(id, err);
    return;
  }
  if (pendingChannels.has(id)) {
    rejectPendingChannel(id, err);
  }
}

function armRequestTimer(reqId: string): void {
  const info = pendingReq.get(reqId);
  if (!info) return;
  if (!Number.isFinite(info.timeoutMs) || info.timeoutMs <= 0) {
    if (info.timerId !== null) {
      clearTimeout(info.timerId);
      info.timerId = null;
    }
    return;
  }
  if (info.timerId !== null) clearTimeout(info.timerId);
  info.timerId = setTimeout(() => {
    if (!pendingReq.has(reqId)) return;
    pendingReq.delete(reqId);
    const err = {
      code: BRIDGE_ERROR.TIMEOUT,
      message: `'${info.method}' timed out`,
    };
    if (info.mode === "stream" && info.stream) {
      info.stream._reject(err);
      return;
    }
    info.reject?.(err);
  }, info.timeoutMs);
}

function armSubscriptionTimer(id: string): void {
  const pending = pendingSubs.get(id);
  if (!pending) return;
  if (pending.timerId !== null) clearTimeout(pending.timerId);
  pending.timerId = setTimeout(() => {
    rejectPendingSubscription(id, {
      code: BRIDGE_ERROR.TIMEOUT,
      message: `Subscription '${pending.topic}' timed out`,
    });
  }, DEFAULT_TIMEOUT_MS);
}

function armChannelTimer(id: string): void {
  const pending = pendingChannels.get(id);
  if (!pending) return;
  if (pending.timerId !== null) clearTimeout(pending.timerId);
  pending.timerId = setTimeout(() => {
    rejectPendingChannel(id, {
      code: BRIDGE_ERROR.TIMEOUT,
      message: `Channel '${pending.topic}' timed out`,
    });
  }, DEFAULT_TIMEOUT_MS);
}

function armPendingOperationTimer(id: string): void {
  if (pendingReq.has(id)) {
    armRequestTimer(id);
    return;
  }
  if (pendingSubs.has(id)) {
    armSubscriptionTimer(id);
    return;
  }
  if (pendingChannels.has(id)) {
    armChannelTimer(id);
  }
}

function removeOutboxByReqId(reqId: string): boolean {
  for (let i = outbox.length - 1; i >= 0; i--) {
    if (outbox[i]?.reqId === reqId) {
      outbox.splice(i, 1);
      return true;
    }
  }
  return false;
}

function send(msg: unknown, reqId?: string): void {
  const kind = (msg as { kind?: string }).kind;
  const isHandshake =
    kind === "hello" || kind === "helloAck" || kind === "ready";

  if (!canSendAppMessages() && !isHandshake) {
    if (outbox.length >= OUTBOX_LIMIT) {
      error("Outbox full");
      if (reqId) {
        rejectPendingOperation(reqId, {
          code: BRIDGE_ERROR.OUTBOX_FULL,
          message: "Bridge outbox is full",
        });
      }
      return;
    }
    outbox.push({ msg, reqId });
    return;
  }
  if (reqId) armPendingOperationTimer(reqId);
  postToNative(msg);
}

function flushOutbox(): void {
  if (!canSendAppMessages()) return;
  while (outbox.length) {
    const item = outbox.shift();
    if (!item) continue;
    if (item.reqId) armPendingOperationTimer(item.reqId);
    postToNative(item.msg);
  }
}

function clearHandshakeTimer(): void {
  if (handshakeTimer !== null) {
    clearTimeout(handshakeTimer);
    handshakeTimer = null;
  }
}

function startHandshake(): void {
  if (handshakeDone) return;
  if (!isTransportReady()) return;
  clearHandshakeTimer();

  const hello: Hello = {
    v: 2,
    kind: "hello",
    nonce: BRIDGE_CONFIG.nonce || "",
    role: "view",
    protocolsSupported: [2],
  };

  helloSent = true;
  postToNative(hello);

  handshakeTimer = setTimeout(() => {
    if (handshakeDone) return;
    handshakeRetryCount++;
    if (handshakeRetryCount < HANDSHAKE_MAX_RETRIES) {
      warn(
        `Handshake timeout (${handshakeRetryCount}/${HANDSHAKE_MAX_RETRIES}), retrying...`,
      );
      helloSent = false;
      startHandshake();
    } else {
      error("Handshake failed");
      clearHandshakeTimer();
      helloSent = false;
      handshakeRetryCount = 0;
      while (outbox.length) {
        const item = outbox.shift();
        if (item?.reqId) {
          rejectPendingOperation(item.reqId, {
            code: BRIDGE_ERROR.HANDSHAKE_FAILED,
            message: "Bridge handshake failed",
          });
        }
      }
      for (const id of Array.from(pendingSubs.keys())) {
        rejectPendingSubscription(id, {
          code: BRIDGE_ERROR.HANDSHAKE_FAILED,
          message: "Bridge handshake failed",
        });
      }
      for (const id of Array.from(pendingChannels.keys())) {
        rejectPendingChannel(id, {
          code: BRIDGE_ERROR.HANDSHAKE_FAILED,
          message: "Bridge handshake failed",
        });
      }
    }
  }, HANDSHAKE_TIMEOUT_MS);
}

function parseIncoming(msg: unknown): Incoming | null {
  if (!msg || typeof msg !== "object") return null;
  const v = (msg as { v?: unknown }).v;
  const kind = (msg as { kind?: unknown }).kind;
  if (v !== 2 || typeof kind !== "string") return null;
  return msg as Incoming;
}

// JSON Patch
function jsonPointerUnescape(seg: string): string {
  return seg.replace(/~1/g, "/").replace(/~0/g, "~");
}

function parseJsonPointer(path: string): string[] {
  if (path === "") return [];
  if (!path.startsWith("/")) throw new Error(`Invalid JSON pointer: ${path}`);
  return path.split("/").slice(1).map(jsonPointerUnescape);
}

function getContainerAndKey(
  root: Record<string, unknown>,
  pointer: string,
  autoCreate = false,
): { container: unknown; key: string } {
  const segments = parseJsonPointer(pointer);
  if (segments.length === 0)
    return { container: { $root: root }, key: "$root" };

  let current: unknown = root;
  for (let i = 0; i < segments.length - 1; i++) {
    const seg = segments[i]!;
    if (Array.isArray(current)) {
      current = current[Number(seg)];
    } else if (current && typeof current === "object") {
      const obj = current as Record<string, unknown>;
      if (autoCreate && obj[seg] === undefined) {
        const nextSeg = segments[i + 1];
        obj[seg] = nextSeg && /^\d+$/.test(nextSeg) ? [] : {};
      }
      current = obj[seg];
    } else {
      throw new Error(`Invalid container at ${seg}`);
    }
  }
  return { container: current, key: segments[segments.length - 1]! };
}

function applyJsonPatch(
  target: Record<string, unknown>,
  ops: JsonPatchOp[],
): void {
  for (const op of ops) {
    const autoCreate = op.op === "add" || op.op === "replace";
    const { container, key } = getContainerAndKey(target, op.path, autoCreate);
    if (key === "$root") {
      for (const k of Object.keys(target)) delete target[k];
      if (op.op !== "remove") {
        const v = (op as { value: unknown }).value;
        if (v && typeof v === "object")
          Object.assign(target, v as Record<string, unknown>);
      }
      continue;
    }

    if (Array.isArray(container)) {
      const idx = key === "-" ? container.length : Number(key);
      if (!Number.isFinite(idx) || idx < 0) throw new Error(`Invalid index`);
      if (op.op === "remove") container.splice(idx, 1);
      else if (op.op === "add") container.splice(idx, 0, op.value);
      else if (op.op === "replace") container[idx] = op.value;
      continue;
    }

    if (!container || typeof container !== "object")
      throw new Error(`Invalid container`);
    const obj = container as Record<string, unknown>;
    if (op.op === "remove") delete obj[key];
    else obj[key] = op.value;
  }
}

function notifyStateSubscribers(initial: boolean): void {
  dataSubscribers.forEach((listener) => {
    try {
      if (!subscriberInitStatus.has(listener)) {
        subscriberInitStatus.set(listener, true);
        listener(deepCopy(pageData), { rev: stateRev, initial: true });
        return;
      }
      listener(deepCopy(pageData), { rev: stateRev, initial });
    } catch (e) {
      warn("Subscriber error:", e);
    }
  });
}

function subscribeState(callback: DataSubscriber): () => void {
  if (typeof callback !== "function") return () => {};
  dataSubscribers.add(callback);
  if (stateRev >= 0) {
    subscriberInitStatus.set(callback, true);
    try {
      callback(deepCopy(pageData), { rev: stateRev, initial: true });
    } catch (e) {
      error("Callback error:", e);
    }
  }
  return () => {
    dataSubscribers.delete(callback);
    subscriberInitStatus.delete(callback);
  };
}

function requestStateRecovery(scope?: string): void {
  LingXiaBridge.call("state.getSnapshot", { scope }).catch(() => {});
}

function applySnapshotFromResult(result: unknown): boolean {
  if (!result || typeof result !== "object") return false;
  const obj = result as { rev?: unknown; state?: unknown };
  if (typeof obj.rev !== "number" || !Number.isFinite(obj.rev)) return false;
  if (!obj.state || typeof obj.state !== "object") return false;
  pageData = obj.state as Record<string, unknown>;
  stateRev = obj.rev;
  if (isDebugEnabled("data")) {
    console.group("[LX] snapshot(res)");
    console.log("rev:", stateRev, "state:", deepCopy(pageData));
    console.groupEnd();
  }
  notifyStateSubscribers(true);
  return true;
}

function handleIncomingMessage(msg: unknown): void {
  // Handle native component events (from Android NativeBridge.sendEventToView)
  if (msg && typeof msg === "object") {
    const obj = msg as {
      type?: string;
      name?: string;
      payload?: NativeComponentMessage;
    };
    if (obj.type === "event" && obj.name === "nativecomponent" && obj.payload) {
      const payload = obj.payload;
      const componentId =
        (payload as { id?: string; componentId?: string }).id ||
        (payload as { componentId?: string }).componentId;
      if (typeof componentId === "string") {
        const handler = nativeComponentHandlers.get(componentId);
        if (handler) {
          try {
            handler(payload);
          } catch (e) {
            error("NC handler error:", e);
          }
        }
      }
      return;
    }
  }

  const message = parseIncoming(msg);
  if (!message) {
    warn("Invalid V2 message:", msg);
    return;
  }

  switch (message.kind) {
    case "helloAck":
      handshakeSessionId = message.sessionId;
      return;

    case "ready":
      if (handshakeSessionId && message.sessionId !== handshakeSessionId) {
        warn("sessionId mismatch");
        return;
      }
      clearHandshakeTimer();
      handshakeDone = true;
      handshakeRetryCount = 0;
      if (message.hostMethods) {
        for (const [k, v] of Object.entries(message.hostMethods)) {
          hostMethodKinds[k] = v;
        }
      }
      if (isDebugEnabled("proto")) log("Handshake complete, hostMethods:", Object.keys(hostMethodKinds).length);
      flushOutbox();
      return;

    case "res": {
      const pendingSub = pendingSubs.get(message.id);
      if (pendingSub) {
        pendingSubs.delete(message.id);
        if (pendingSub.timerId !== null) clearTimeout(pendingSub.timerId);
        if (message.ok) {
          pendingSub.subscription._markActive();
          activeSubs.set(message.id, pendingSub.subscription);
          pendingSub.resolve(pendingSub.subscription);
        } else {
          const err = message.error ?? {
            code: BRIDGE_ERROR.INTERNAL_ERROR,
            message: `Subscription '${pendingSub.topic}' failed`,
          };
          pendingSub.subscription._reject(err);
          pendingSub.reject(err);
        }
        return;
      }

      const info = pendingReq.get(message.id);
      if (!info) return;
      pendingReq.delete(message.id);
      if (info.timerId !== null) clearTimeout(info.timerId);
      if (message.ok) {
        if (info.method === "state.getSnapshot") {
          if (!applySnapshotFromResult(message.result)) {
            warn("Invalid state.getSnapshot result");
          }
        }
        if (info.mode === "stream" && info.stream) {
          info.stream._resolve(message.result);
          return;
        }
        info.resolve?.(message.result);
      } else {
        const err = message.error ?? {
          code: BRIDGE_ERROR.INTERNAL_ERROR,
          message: `Call '${info.method}' failed`,
        };
        if (info.mode === "stream" && info.stream) {
          info.stream._reject(err);
          return;
        }
        info.reject?.(err);
      }
      return;
    }

    case "event": {
      if (pendingReq.has(message.id)) {
        const req = pendingReq.get(message.id);
        if (req?.mode === "stream" && req.stream) {
          armRequestTimer(message.id);
          req.stream._emitData(message.payload);
        } else {
          warn(`Received event for non-stream request '${message.id}'`);
        }
        return;
      }

      const sub = activeSubs.get(message.id);
      if (sub) {
        sub._emitData(message.payload);
        return;
      }
      return;
    }

    case "sub.close": {
      const sub = activeSubs.get(message.id);
      if (!sub) return;
      activeSubs.delete(message.id);
      sub._markInactive();
      if (message.error) {
        sub._reject(message.error);
      }
      return;
    }

    case "state.snapshot":
      pageData = message.state || {};
      stateRev = message.rev;
      if (isDebugEnabled("data")) {
        console.group("[LX] snapshot");
        console.log("rev:", stateRev, "state:", deepCopy(pageData));
        console.groupEnd();
      }
      notifyStateSubscribers(true);
      return;

    case "state.patch":
      if (message.baseRev !== stateRev) {
        warn("baseRev mismatch", { have: stateRev, want: message.baseRev });
        requestStateRecovery(message.scope);
        return;
      }
      try {
        applyJsonPatch(pageData, message.ops || []);
        stateRev = message.rev;
      } catch (e) {
        error("Patch failed:", e);
        requestStateRecovery(message.scope);
        return;
      }
      if (isDebugEnabled("data")) {
        console.group("[LX] patch");
        console.log("rev:", stateRev, "ops:", message.ops);
        console.groupEnd();
      }
      notifyStateSubscribers(false);
      if (message.ack)
        send({
          v: 2,
          kind: "state.ack",
          scope: message.scope,
          rev: message.rev,
        } as StateAck);
      return;

    case "ch.ack": {
      const pendingChannel = pendingChannels.get(message.id);
      if (!pendingChannel) return;
      pendingChannels.delete(message.id);
      if (pendingChannel.timerId !== null) clearTimeout(pendingChannel.timerId);
      if (message.ok) {
        pendingChannel.channel._markOpen();
        activeChannels.set(message.id, pendingChannel.channel);
        pendingChannel.resolve(pendingChannel.channel);
      } else {
        const err = message.error ?? {
          code: BRIDGE_ERROR.INTERNAL_ERROR,
          message: `Channel '${pendingChannel.topic}' failed`,
        };
        pendingChannel.channel._reject(err);
        pendingChannel.reject(err);
      }
      return;
    }

    case "ch.data": {
      const channel = activeChannels.get(message.id);
      if (!channel) return;
      channel._emitData(message.payload);
      return;
    }

    case "ch.close": {
      const pendingChannel = pendingChannels.get(message.id);
      if (pendingChannel) {
        pendingChannels.delete(message.id);
        pendingChannel.channel._emitClose(message.code, message.reason);
        return;
      }
      const channel = activeChannels.get(message.id);
      if (!channel) return;
      activeChannels.delete(message.id);
      channel._emitClose(message.code, message.reason);
      return;
    }

    case "req": {
      const reqMsg = message as Req;
      const requiredCap = inferCap(reqMsg.method);
      if (!reqMsg.cap || reqMsg.cap !== requiredCap) {
        send({
          v: 2,
          kind: "res",
          id: reqMsg.id,
          ok: false,
          error: {
            code: BRIDGE_ERROR.CAPABILITY_DENIED,
            message: `Invalid cap for ${reqMsg.method}`,
          },
        } as Res);
        return;
      }
      const handler = viewMethodHandlers.get(reqMsg.method);
      if (!handler) {
        send({
          v: 2,
          kind: "res",
          id: reqMsg.id,
          ok: false,
          error: {
            code: BRIDGE_ERROR.METHOD_NOT_FOUND,
            message: `View handler not found: ${reqMsg.method}`,
          },
        } as Res);
        return;
      }
      Promise.resolve()
        .then(() => handler(reqMsg.params))
        .then((result) => {
          send({ v: 2, kind: "res", id: reqMsg.id, ok: true, result } as Res);
        })
        .catch((err) => {
          send({
            v: 2,
            kind: "res",
            id: reqMsg.id,
            ok: false,
            error: unknownToError(
              err,
              `View handler '${reqMsg.method}' failed`,
            ),
          } as Res);
        });
      return;
    }
  }
}

// Native components
const nativeComponentHandlers = new Map<
  string,
  (message: NativeComponentMessage) => void
>();
const nativeComponentQueue: NativeComponentMessage[] = [];
let nativeComponentReady = false;

function hasNativeComponentHandler(): boolean {
  if (typeof window === "undefined") return false;
  return !!(
    window.webkit?.messageHandlers?.NativeComponent ||
    window.NativeComponentBridge?.postMessage
  );
}

function postNativeComponentMessage(message: NativeComponentMessage): void {
  try {
    if (window.webkit?.messageHandlers?.NativeComponent) {
      window.webkit.messageHandlers.NativeComponent.postMessage(message);
      return;
    }
    if (window.NativeComponentBridge?.postMessage) {
      window.NativeComponentBridge.postMessage(stringifyForNative(message));
      return;
    }
  } catch (e) {
    error("NativeComponent send error:", e);
  }
}

function flushNativeComponentQueue(): void {
  if (!hasNativeComponentHandler() || nativeComponentQueue.length === 0) return;
  nativeComponentReady = true;
  while (nativeComponentQueue.length) {
    const msg = nativeComponentQueue.shift()!;
    try {
      postNativeComponentMessage(msg);
    } catch {
      break;
    }
  }
}

function sendNativeComponentMessage(message: NativeComponentMessage): void {
  try {
    if (!hasNativeComponentHandler()) {
      nativeComponentQueue.push(message);
      return;
    }
    if (!nativeComponentReady) flushNativeComponentQueue();
    postNativeComponentMessage(message);
  } catch (e) {
    error("NC send failed:", e);
  }
}

function nextMessageId(prefix: "c" | "s" | "ch"): string {
  return `${prefix}_${Date.now()}_${requestCounter++}`;
}

function attachAbortSignal(
  id: string,
  signal: AbortSignal | undefined,
  onAbort: () => void,
): void {
  if (!signal) return;
  if (signal.aborted) {
    onAbort();
    return;
  }
  let abortListener: EventListenerObject;
  abortListener = {
    handleEvent: (): void => {
      signal.removeEventListener("abort", abortListener);
      onAbort();
    },
  };
  signal.addEventListener("abort", abortListener);
}

function bridgeSubscribe(
  topic: string,
  params?: unknown,
  options?: SubscribeOptions,
): Promise<Subscription> {
  if (!topic || typeof topic !== "string") {
    return Promise.reject({
      code: BRIDGE_ERROR.MALFORMED_MESSAGE,
      message: "Topic name must be a non-empty string",
    } satisfies LxBridgeError);
  }
  if (!helloSent) startHandshake();

  return new Promise((resolve, reject) => {
    const id = nextMessageId("s");
    const subscription = createSubscription(id, () => {
      activeSubs.delete(id);
      const pending = pendingSubs.get(id);
      if (pending && pending.timerId !== null) clearTimeout(pending.timerId);
      pendingSubs.delete(id);
      send({ v: 2, kind: "unsub", id } as Unsub);
    });
    pendingSubs.set(id, {
      topic,
      subscription,
      resolve,
      reject,
      timerId: null,
    });
    send(
      {
        v: 2,
        kind: "sub",
        id,
        topic,
        params: normalizeParams(params),
        cap: options?.cap || inferCap(topic),
      } as Sub,
      id,
    );
  });
}

// Public interface
export const LingXiaBridge: LingXiaBridgeInterface = {
  call<M extends LxMethod>(
    method: M | string,
    params?: LxMethodParams<M> | unknown,
    options?: CallOptions,
  ): Promise<LxMethodResult<M> | unknown> {
    return new Promise((resolve, reject) => {
      if (!method || typeof method !== "string") {
        reject({
          code: BRIDGE_ERROR.MALFORMED_MESSAGE,
          message: "Method name must be a non-empty string",
        });
        return;
      }
      if (!helloSent) startHandshake();

      const id = nextMessageId("c");
      const cap = options?.cap || inferCap(method);
      const timeoutMs = options?.timeoutMs ?? DEFAULT_TIMEOUT_MS;
      pendingReq.set(id, {
        method,
        mode: "call",
        resolve,
        reject: (e) => reject(e),
        timeoutMs,
        timerId: null,
      });

      const req: Req = {
        v: 2,
        kind: "req",
        id,
        method,
        params: normalizeParams(params),
        cap,
      };
      send(req, id);

      attachAbortSignal(id, options?.signal, () => {
        const removed = removeOutboxByReqId(id);
        rejectPendingRequest(id, {
          code: BRIDGE_ERROR.CANCELED,
          message: "Bridge request aborted",
        });
        if (!removed && handshakeDone)
          send({ v: 2, kind: "cancel", id } as Cancel);
      });
    });
  },

  callStream<M extends LxMethod>(
    method: M | string,
    params?: LxMethodParams<M> | unknown,
    options?: StreamCallOptions,
  ): StreamHandle<LxMethodStreamData<M>, LxMethodResult<M>> | StreamHandle {
    if (!method || typeof method !== "string") {
      const invalid = createStreamHandle("invalid", () => {});
      invalid._reject({
        code: BRIDGE_ERROR.MALFORMED_MESSAGE,
        message: "Method name must be a non-empty string",
      });
      return invalid as StreamHandle<LxMethodStreamData<M>, LxMethodResult<M>>;
    }
    if (!helloSent) startHandshake();

    const id = nextMessageId("c");
    const handle = createStreamHandle(id, () => {
      const removed = removeOutboxByReqId(id);
      if (removed) {
        rejectPendingRequest(id, {
          code: BRIDGE_ERROR.CANCELED,
          message: "Bridge request aborted",
        });
        return;
      }
      if (handshakeDone) send({ v: 2, kind: "cancel", id } as Cancel);
    });
    const cap = options?.cap || inferCap(method);
    const timeoutMs = options?.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    pendingReq.set(id, {
      method,
      mode: "stream",
      stream: handle,
      timeoutMs,
      timerId: null,
    });

    send(
      {
        v: 2,
        kind: "req",
        id,
        method,
        params: normalizeParams(params),
        cap,
      } as Req,
      id,
    );
    attachAbortSignal(id, options?.signal, () => handle.cancel());
    return handle as StreamHandle<LxMethodStreamData<M>, LxMethodResult<M>>;
  },

  notify(method: string, params?: unknown, options?: NotifyOptions): void {
    if (!method || typeof method !== "string") return;
    if (!helloSent) startHandshake();
    send({
      v: 2,
      kind: "notify",
      method,
      params: normalizeParams(params),
      cap: options?.cap || inferCap(method),
    } as Notify);
  },

  subscribe: bridgeSubscribe,

  state: {
    subscribe: subscribeState,
  },

  channel: {
    open(
      topic: string,
      params?: unknown,
      options?: ChannelOpenOptions,
    ): Promise<Channel> {
      if (!topic || typeof topic !== "string") {
        return Promise.reject({
          code: BRIDGE_ERROR.MALFORMED_MESSAGE,
          message: "Topic name must be a non-empty string",
        } satisfies LxBridgeError);
      }
      if (!helloSent) startHandshake();

      return new Promise((resolve, reject) => {
        const id = nextMessageId("ch");
        const channel = createChannel(
          id,
          (payload, seq) => {
            send({ v: 2, kind: "ch.data", id, seq, payload } as ChData);
          },
          (code, reason) => {
            const pending = pendingChannels.get(id);
            if (pending && pending.timerId !== null)
              clearTimeout(pending.timerId);
            pendingChannels.delete(id);
            activeChannels.delete(id);
            send({ v: 2, kind: "ch.close", id, code, reason } as ChClose);
          },
        );
        pendingChannels.set(id, {
          topic,
          channel,
          resolve,
          reject,
          timerId: null,
        });
        send(
          {
            v: 2,
            kind: "ch.open",
            id,
            topic,
            params: normalizeParams(params),
            cap: options?.cap || inferCap(topic),
          } as ChOpen,
          id,
        );
      });
    },
  },

  _connectWebMessagePort(port: MessagePort): void {
    if (communicationMethod !== MESSAGE_PORT_TYPE) return;
    if (messagePort && messagePort !== port) {
      try {
        messagePort.onmessage = null;
        messagePort.close();
      } catch {}
    }
    messagePort = port;
    port.onmessage = (event: MessageEvent) => {
      let data = event.data;
      if (typeof data === "string") {
        try {
          data = JSON.parse(data);
        } catch {
          return;
        }
      }
      handleIncomingMessage(data);
    };
    // Some WebView MessagePort implementations (notably Android WebMessagePort)
    // require an explicit start() to begin dispatching onmessage events.
    try {
      port.start();
    } catch {}
    log("Port connected");
    startHandshake();
  },

  _receiveEvaluateMessage(messageString: string): void {
    try {
      if (messageString) handleIncomingMessage(JSON.parse(messageString));
    } catch (e) {
      error("Parse error:", e);
    }
  },

  debug: new Proxy(debugFlags, {
    get(target, prop: keyof typeof debugFlags) {
      return target[prop];
    },
    set(target, prop: keyof typeof debugFlags, value: boolean) {
      if (prop in target) {
        target[prop] = !!value;
        console.log(`[LX] ${prop}: ${value}`);
        return true;
      }
      return false;
    },
  }),

  platform: {
    isHarmony,
    isIOS,
    isAndroid,
    isMacOS,
    isDesktop,
    getOS: getPlatformOS,
  },

  dom: {
    measureById(id: string): [number, number, number, number, number] | null {
      try {
        if (!id) return null;
        const el = document.getElementById(id);
        if (!el) return null;
        const r = el.getBoundingClientRect();
        let radius = 0;
        try {
          radius = parseFloat(getComputedStyle(el).borderRadius) || 0;
        } catch {}
        return [
          r.left + window.scrollX,
          r.top + window.scrollY,
          r.width,
          r.height,
          radius,
        ];
      } catch {
        return null;
      }
    },
  },

  nativeComponents: {
    send: sendNativeComponentMessage,
    hasHandler: hasNativeComponentHandler,
    flush: flushNativeComponentQueue,
    register(
      id: string,
      handler: (message: NativeComponentMessage) => void,
    ): () => void {
      if (!id || typeof handler !== "function") return () => {};
      nativeComponentHandlers.set(id, handler);
      return () => nativeComponentHandlers.delete(id);
    },
    unregister(id: string): void {
      nativeComponentHandlers.delete(id);
    },
  },

  isReady(): boolean {
    return handshakeDone;
  },
};

function createHostPathProxy(path: string[]): unknown {
  const callable = (): void => {};
  return new Proxy(callable, {
    apply(_target, _thisArg, args: unknown[]) {
      const method = path.join(".");
      const payload =
        args.length === 0 ? undefined : args.length === 1 ? args[0] : args;
      // Always use callStream — works for both unary and stream host handlers.
      // Unary handlers return a single `res`; stream handlers emit `event`s then `res`.
      // The returned handle is also thenable so `await host.x.y()` just works.
      const handle = LingXiaBridge.callStream(`host.${method}`, payload, { cap: "host", timeoutMs: 0 });
      // Make thenable: `await host.x.y()` resolves to the final result,
      // while `host.x.y().on('data', ...)` works for streams.
      const result = handle.result;
      return new Proxy(handle, {
        get(target, prop) {
          if (prop === "then") return result.then.bind(result);
          if (prop === "catch") return result.catch.bind(result);
          const val = (target as unknown as Record<string | symbol, unknown>)[prop];
          return typeof val === "function" ? (val as Function).bind(target) : val;
        },
      });
    },
    get(_target, prop) {
      if (prop === "then") return undefined;
      if (typeof prop !== "string") return undefined;
      return createHostPathProxy([...path, prop]);
    },
  });
}

export const host: HostApi = new Proxy({} as HostApi, {
  get(_target, prop) {
    if (typeof prop !== "string") {
      return undefined;
    }
    return createHostPathProxy([prop]);
  },
}) as HostApi;

export function initBridge(): void {
  log(`Method: ${communicationMethod}`);
  activateReceiver(LingXiaBridge._receiveEvaluateMessage);

  if (communicationMethod === MESSAGE_PORT_TYPE) {
    installMessagePortInitListener();
    getMessagePort().catch((e) => warn("Port init failed:", e));
  } else if (
    communicationMethod === "webkit" ||
    communicationMethod === JS_INTERFACE_TYPE
  ) {
    startHandshake();
  } else {
    warn("Unknown method");
  }

  window.LingXiaBridge = LingXiaBridge;
  window.host = host;
  installNativeComponentCoverageMonitor({
    os: getPlatformOS(),
    send: sendNativeComponentMessage,
  });
  log("Init complete");
}
