import type {
  CallOptions,
  ChannelOpenOptions,
  ChannelOptions,
  ChannelCloseEvent,
  DataSubscriber,
  LingXiaBridgeInterface,
  LxChannel,
  LxBridgeError,
  LxMethod,
  LxMethodParams,
  LxMethodResult,
  LxMethodStreamData,
  LxStream,
  NativeComponentMessage,
  NativeChannel,
  NativeError,
  NativeStream,
  NotifyOptions,
  InvokeOptions,
  StreamCallOptions,
  StreamOptions,
} from "./types";
import { BRIDGE_ERROR } from "./types";
import { toBridgeError, toNativeError } from "./invocation";
import { installNativeComponentCoverageMonitor } from "./nativecomponents/coverage-monitor";
import {
  BRIDGE_CONFIG,
  getCommunicationMethod,
  getPlatformOS,
  isAndroid,
  isHarmony,
  isIOS,
  isMacOS,
  isWindows,
  isDesktop,
  isApple,
  isDevSession,
  isRunner,
} from "./runtime-env";

const NATIVE_HANDLER_NAME = "LingXia";
const GLOBAL_RECEIVER_NAME = "__LingXiaRecvMessage";
const DEFAULT_TIMEOUT_MS = 5000;
const HANDSHAKE_TIMEOUT_MS = 10000;
const HANDSHAKE_MAX_RETRIES = 3;
const LOG_PREFIX = "[LX.Bridge]";
const MESSAGE_PORT_TYPE = "messageport";
const JS_INTERFACE_TYPE = "jsinterface";
const WEB_MESSAGE_TYPE = "webmessage";
const OUTBOX_LIMIT = 256;
const APPLE_DOWNSTREAM_URL = BRIDGE_CONFIG.appleDownstreamURL;
const APPLE_RECONNECT_BASE_MS = 200;
const APPLE_RECONNECT_MAX_MS = 2000;

const debugFlags = { data: false, proto: false, all: false };
const earlyNativeMessages: string[] = [];

// Plain-object equivalent of `new Proxy(debugFlags, ...)`. Avoids referencing
// the `Proxy` global so the module loads on older WebViews (Android 5.x stock
// WebView is Chromium 37–39, which predates Chromium 49's Proxy support).
function createDebugObject(flags: typeof debugFlags): typeof debugFlags {
  const target = {} as typeof debugFlags;
  (Object.keys(flags) as Array<keyof typeof debugFlags>).forEach((key) => {
    Object.defineProperty(target, key, {
      enumerable: true,
      configurable: false,
      get(): boolean {
        return flags[key];
      },
      set(value: boolean): void {
        flags[key] = !!value;
        console.log(`[LX] ${key}: ${value}`);
      },
    });
  });
  return target;
}

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

// `log` is the bridge's own protocol/lifecycle trace. Native log capture
// forwards whatever the page emits to `console`, so the bridge itself decides
// whether to surface this framework chatter: only in a `lingxia dev` session
// (or when a debug flag is set). Shipped apps stay quiet, leaving the captured
// stream to the page's own output plus bridge warnings/errors.
function log(...args: unknown[]): void {
  if (
    !isDevSession() &&
    !debugFlags.all &&
    !debugFlags.proto &&
    !debugFlags.data
  ) {
    return;
  }
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

const communicationMethod = getCommunicationMethod();

installEarlyReceiver();

// Transport
let messagePort: MessagePort | null = null;
let appleDownstreamConnected = false;
let appleDownstreamTask: Promise<void> | null = null;
let appleDownstreamAbortController: AbortController | null = null;
let appleReconnectTimer: ReturnType<typeof setTimeout> | null = null;
let appleReconnectDelayMs = APPLE_RECONNECT_BASE_MS;
// Highest transport frame seq processed. Sent as `?from=` on reconnect so the
// host replays the gap; survives reconnects so a WebKit-replaced stream resumes
// without losing frames or tearing down the bridge session.
let appleLastFrameSeq = 0;
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

function useAppleDownstreamTransport(): boolean {
  return communicationMethod === "webkit" && (isIOS() || isMacOS());
}

function clearAppleReconnectTimer(): void {
  if (appleReconnectTimer !== null) {
    clearTimeout(appleReconnectTimer);
    appleReconnectTimer = null;
  }
}

function closeActiveChannelsFromTransport(reason: string): void {
  for (const [id, channel] of Array.from(activeChannels.entries())) {
    activeChannels.delete(id);
    channel._emitClose(BRIDGE_ERROR.STREAM_CLOSED, reason);
  }
}

function rejectAllPendingForTransport(reason: string): void {
  for (const id of Array.from(pendingReq.keys())) {
    rejectPendingRequest(id, {
      code: BRIDGE_ERROR.STREAM_CLOSED,
      message: reason,
    });
  }
  for (const id of Array.from(pendingChannels.keys())) {
    rejectPendingChannel(id, {
      code: BRIDGE_ERROR.STREAM_CLOSED,
      message: reason,
    });
  }
  closeActiveChannelsFromTransport(reason);
}

function resetHandshakeState(reason: string, rejectPending: boolean): void {
  clearHandshakeTimer();
  handshakeSessionId = null;
  handshakeDone = false;
  helloSent = false;
  handshakeRetryCount = 0;
  if (rejectPending) rejectAllPendingForTransport(reason);
}

function processAppleDownstreamBuffer(buffer: string): string {
  let remaining = buffer;
  while (true) {
    const newlineIndex = remaining.indexOf("\n");
    if (newlineIndex < 0) break;
    const line = remaining.slice(0, newlineIndex).trim();
    remaining = remaining.slice(newlineIndex + 1);
    if (!line) continue;
    try {
      handleAppleDownstreamFrame(JSON.parse(line));
    } catch (e) {
      warn("Apple downstream parse error:", e, line);
    }
  }
  return remaining;
}

// Unwrap the transport envelope: {"lxff":seq,"m":<message>} carries a business
// message with its frame seq; {"lxreset":true} means the host cannot replay our
// resume point and we must re-handshake.
function handleAppleDownstreamFrame(frame: unknown): void {
  if (!frame || typeof frame !== "object") return;
  const record = frame as { lxff?: unknown; m?: unknown; lxreset?: unknown };
  if (record.lxreset === true) {
    appleLastFrameSeq = 0;
    resetHandshakeState("Apple downstream reset", true);
    startHandshake();
    return;
  }
  if (typeof record.lxff === "number") {
    // Replayed frames after a reconnect can repeat the last-seen seq; ignore
    // anything we have already processed so the session is not double-fed.
    if (record.lxff <= appleLastFrameSeq) return;
    appleLastFrameSeq = record.lxff;
    handleIncomingMessage(record.m);
    return;
  }
  // Legacy/un-enveloped line (e.g. a bare keepalive); pass through.
  handleIncomingMessage(frame);
}

async function runAppleDownstream(): Promise<void> {
  if (!APPLE_DOWNSTREAM_URL) {
    throw new Error("Apple downstream URL is not configured");
  }

  const controller = new AbortController();
  appleDownstreamAbortController = controller;
  // Resume from the last frame we saw so the host replays the gap on reconnect.
  const separator = APPLE_DOWNSTREAM_URL.includes("?") ? "&" : "?";
  const url = `${APPLE_DOWNSTREAM_URL}${separator}from=${appleLastFrameSeq}`;
  const response = await fetch(url, {
    method: "GET",
    cache: "no-store",
    headers: { Accept: "application/x-ndjson" },
    signal: controller.signal,
  });

  if (!response.ok) {
    throw new Error(`Apple downstream HTTP ${response.status}`);
  }
  if (!response.body) {
    throw new Error("Apple downstream response body unavailable");
  }

  appleDownstreamConnected = true;
  appleReconnectDelayMs = APPLE_RECONNECT_BASE_MS;
  if (isDebugEnabled("proto")) log("Apple downstream connected");
  startHandshake();

  const reader = response.body.getReader();
  const decoder = new TextDecoder();
  let buffered = "";
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      buffered += decoder.decode(value, { stream: true });
      buffered = processAppleDownstreamBuffer(buffered);
    }
    buffered += decoder.decode();
    const tail = buffered.trim();
    if (tail) {
      try {
        handleIncomingMessage(JSON.parse(tail));
      } catch (e) {
        warn("Apple downstream trailing parse error:", e, tail);
      }
    }
  } catch (e) {
    if (controller.signal.aborted) return;
    throw e;
  } finally {
    try {
      reader.releaseLock();
    } catch {}
  }
}

function scheduleAppleDownstreamReconnect(reason: string): void {
  if (!useAppleDownstreamTransport()) return;
  clearAppleReconnectTimer();
  const delay = appleReconnectDelayMs;
  appleReconnectDelayMs = Math.min(
    appleReconnectDelayMs * 2,
    APPLE_RECONNECT_MAX_MS,
  );
  const message = `Apple downstream disconnected, retrying in ${delay}ms: ${reason}`;
  if (delay <= APPLE_RECONNECT_BASE_MS) {
    log(message);
  } else {
    warn(message);
  }
  appleReconnectTimer = setTimeout(() => {
    appleReconnectTimer = null;
    ensureAppleDownstream();
  }, delay);
}

function ensureAppleDownstream(): void {
  if (!useAppleDownstreamTransport()) return;
  if (appleDownstreamTask) return;
  clearAppleReconnectTimer();
  appleDownstreamTask = runAppleDownstream()
    .catch((e) => {
      if (appleDownstreamAbortController?.signal.aborted) return;
      warn("Apple downstream failed:", e);
    })
    .finally(() => {
      const aborted = appleDownstreamAbortController?.signal.aborted ?? false;
      appleDownstreamTask = null;
      appleDownstreamAbortController = null;
      appleDownstreamConnected = false;
      // A dropped transport is not a dead session: reconnect resumes from the
      // last seq and the host replays the gap, so the handshake and in-flight
      // streams stay intact. Only a host-sent reset (handled on the frame path)
      // or a real abort tears things down.
      if (!aborted) scheduleAppleDownstreamReconnect("stream closed");
    });
}

// Dev hook: drop the current downstream and reconnect, exactly as WebKit does
// when it replaces the streaming fetch. The reconnect carries the real
// `from=<lastSeq>`, so this exercises the true resume path. Exposed for repro
// harnesses; a no-op off the Apple transport.
function forceDownstreamReconnect(): void {
  if (!useAppleDownstreamTransport()) return;
  const task = appleDownstreamTask;
  appleDownstreamAbortController?.abort();
  if (task) task.finally(() => ensureAppleDownstream());
  else ensureAppleDownstream();
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
      (communicationMethod === JS_INTERFACE_TYPE ||
        communicationMethod === WEB_MESSAGE_TYPE) &&
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

type InternalStreamHandle = LxStream & {
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
  const pendingData: unknown[] = [];
  const pendingReads: Array<{
    resolve: (value: IteratorResult<unknown, unknown>) => void;
    reject: (reason: unknown) => void;
  }> = [];
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
    [Symbol.asyncIterator](): AsyncIterator<unknown, unknown, void> {
      return {
        next(): Promise<IteratorResult<unknown, unknown>> {
          if (pendingData.length > 0) {
            return Promise.resolve({
              done: false,
              value: pendingData.shift(),
            });
          }
          if (done) {
            return result.then(
              (finalValue) => ({ done: true, value: finalValue }),
              (err) => Promise.reject(err),
            );
          }
          return new Promise<IteratorResult<unknown, unknown>>((resolve, reject) => {
            pendingReads.push({ resolve, reject });
          });
        },
      };
    },
    cancel(): void {
      if (done) return;
      cancelFn();
    },
    _emitData(payload: unknown): void {
      if (done) return;
      if (pendingReads.length > 0) {
        pendingReads.shift()!.resolve({ done: false, value: payload });
      } else {
        pendingData.push(payload);
      }
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
      while (pendingReads.length > 0) {
        pendingReads.shift()!.resolve({ done: true, value: resultValue });
      }
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
      while (pendingReads.length > 0) {
        pendingReads.shift()!.reject(err);
      }
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

type InternalChannel = LxChannel & {
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
  let closed = false;
  const pendingData: unknown[] = [];
  const pendingReads: Array<{
    resolve: (value: IteratorResult<unknown, void>) => void;
    reject: (reason: unknown) => void;
  }> = [];
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
      if (event === "data") {
        listeners.data.add(listener as (payload: unknown) => void);
        // Flush data buffered before this listener attached. The host can send
        // ch.data immediately after the open ack, before the consumer (e.g. a
        // wrapped channel's onMessage) attaches its listener; without this those
        // early events would sit in pendingData and never reach the listener.
        if (pendingData.length > 0) {
          const buffered = pendingData.splice(0);
          for (const payload of buffered) {
            try {
              (listener as (payload: unknown) => void)(payload);
            } catch (e) {
              warn("Channel listener failed:", e);
            }
          }
        }
      }
      if (event === "close")
        listeners.close.add(
          listener as (code?: string, reason?: string) => void,
        );
      if (event === "error")
        listeners.error.add(listener as (error: LxBridgeError) => void);
      return this;
    },
    [Symbol.asyncIterator](): AsyncIterator<unknown, void, void> {
      return {
        next(): Promise<IteratorResult<unknown, void>> {
          if (pendingData.length > 0) {
            return Promise.resolve({
              done: false,
              value: pendingData.shift(),
            });
          }
          if (closed) {
            return Promise.resolve({ done: true, value: undefined });
          }
          return new Promise<IteratorResult<unknown, void>>((resolve, reject) => {
            pendingReads.push({ resolve, reject });
          });
        },
      };
    },
    close(code?: string, reason?: string): void {
      if (!open) return;
      open = false;
      closed = true;
      closeFn(code, reason);
      channel._emitClose(code, reason);
    },
    _emitData(payload: unknown): void {
      if (!open) return;
      if (pendingReads.length > 0) {
        pendingReads.shift()!.resolve({ done: false, value: payload });
      } else {
        pendingData.push(payload);
      }
      for (const listener of listeners.data) {
        try {
          listener(payload);
        } catch (e) {
          warn("Channel listener failed:", e);
        }
      }
    },
    _emitClose(code?: string, reason?: string): void {
      closed = true;
      while (pendingReads.length > 0) {
        pendingReads.shift()!.resolve({ done: true, value: undefined });
      }
      for (const listener of listeners.close) {
        try {
          listener(code, reason);
        } catch (e) {
          warn("Channel close listener failed:", e);
        }
      }
    },
    _reject(err: LxBridgeError): void {
      closed = true;
      while (pendingReads.length > 0) {
        pendingReads.shift()!.reject(err);
      }
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
      closed = false;
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
const pendingChannels = new Map<
  string,
  {
    topic: string;
    channel: InternalChannel;
    resolve: (channel: LxChannel<any, any>) => void;
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
  if (useAppleDownstreamTransport()) return appleDownstreamConnected;
  return (
    communicationMethod === "webkit" ||
    communicationMethod === JS_INTERFACE_TYPE ||
    communicationMethod === WEB_MESSAGE_TYPE
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
    const normalized = toBridgeError(err);
    if (info.mode === "stream" && info.stream) {
      info.stream._reject(normalized);
      return;
    }
    info.reject?.(normalized);
  }
}

function rejectPendingChannel(id: string, err: LxBridgeError): void {
  const pending = pendingChannels.get(id);
  if (!pending) return;
  pendingChannels.delete(id);
  if (pending.timerId !== null) clearTimeout(pending.timerId);
  const normalized = toBridgeError(err);
  pending.channel._reject(normalized);
  pending.reject(normalized);
}

function rejectPendingOperation(id: string, err: LxBridgeError): void {
  if (pendingReq.has(id)) {
    rejectPendingRequest(id, err);
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
    rejectPendingRequest(reqId, {
      code: BRIDGE_ERROR.TIMEOUT,
      message: `'${info.method}' timed out`,
    });
  }, info.timeoutMs);
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
  rawBridge.call("state.getSnapshot", { scope }).catch(() => {});
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
        const err = toBridgeError(message.error);
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
        const err = toBridgeError(message.error);
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
            error: toBridgeError(err),
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

// Low-level protocol interface. Public consumers should use LingXiaBridge.invoke/stream/channel.
const rawBridge = {
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

  stream<M extends LxMethod>(
    method: M | string,
    params?: LxMethodParams<M> | unknown,
    options?: StreamCallOptions,
  ): LxStream<LxMethodStreamData<M>, LxMethodResult<M>> | LxStream {
    if (!method || typeof method !== "string") {
      const invalid = createStreamHandle("invalid", () => {});
      invalid._reject({
        code: BRIDGE_ERROR.MALFORMED_MESSAGE,
        message: "Method name must be a non-empty string",
      });
      return invalid as LxStream<LxMethodStreamData<M>, LxMethodResult<M>>;
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
    return handle as LxStream<LxMethodStreamData<M>, LxMethodResult<M>>;
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

  state: {
    subscribe: subscribeState,
  },

  channel: {
    open<TIn = unknown, TOut = TIn>(
      topic: string,
      params?: unknown,
      options?: ChannelOpenOptions,
    ): Promise<LxChannel<TIn, TOut>> {
      if (!topic || typeof topic !== "string") {
        return Promise.reject(
          {
            code: BRIDGE_ERROR.MALFORMED_MESSAGE,
            message: "Topic name must be a non-empty string",
          },
        );
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
        ) as InternalChannel & LxChannel<TIn, TOut>;
        pendingChannels.set(id, {
          topic,
          channel,
          resolve: resolve as (channel: LxChannel<TIn, TOut>) => void,
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
};

// Public interface
export const LingXiaBridge: LingXiaBridgeInterface = {
  invoke<TResult = unknown, TInput = void>(
    route: string,
    input?: TInput,
    options?: InvokeOptions,
  ): Promise<TResult> {
    return rawBridge
      .call(nativeRoute(route), input, nativeOptions(options))
      .then((value) => value as TResult)
      .catch((error) => Promise.reject(toNativeError(error)));
  },

  notify<TInput = void>(
    route: string,
    input?: TInput,
    options?: NotifyOptions,
  ): void {
    rawBridge.notify(nativeRoute(route), input, nativeOptions(options));
  },

  stream<TEvent = unknown, TResult = void, TInput = void>(
    route: string,
    input?: TInput,
    options?: StreamOptions,
  ): NativeStream<TEvent, TResult> {
    return wrapNativeStream<TEvent, TResult>(
      rawBridge.stream(nativeRoute(route), input, nativeOptions(options)) as LxStream<
        TEvent,
        TResult
      >,
    );
  },

  channel<TIn = unknown, TOut = unknown>(
    route: string,
    input?: unknown,
    options?: ChannelOptions,
  ): Promise<NativeChannel<TIn, TOut>> {
    return rawBridge.channel
      .open<TOut, TIn>(nativeRoute(route), input, nativeOptions(options))
      .then((handle) => wrapNativeChannel<TIn, TOut>(handle))
      .catch((error) => Promise.reject(toNativeError(error)));
  },

  raw: rawBridge,

  state: {
    subscribe: subscribeState,
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

  debug: createDebugObject(debugFlags),

  platform: {
    isHarmony,
    isIOS,
    isAndroid,
    isMacOS,
    isWindows,
    isDesktop,
    isApple,
    isRunner,
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

function nativeRoute(route: string): string {
  return route.startsWith("host.") ? route : `host.${route}`;
}

function nativeOptions<T extends { cap?: string }>(options?: T): T {
  return { ...(options || ({} as T)), cap: options?.cap || "host" };
}

function wrapNativeStream<TEvent, TResult>(
  handle: LxStream<TEvent, TResult>,
): NativeStream<TEvent, TResult> {
  const eventListeners = new Set<(event: TEvent) => void>();
  const errorListeners = new Set<(error: NativeError) => void>();
  handle.on("data", (event) => {
    for (const listener of eventListeners) listener(event);
  });
  handle.on("error", (error) => {
    const nativeError = toNativeError(error);
    for (const listener of errorListeners) listener(nativeError);
  });
  return {
    onEvent(listener: (event: TEvent) => void): () => void {
      eventListeners.add(listener);
      return () => {
        eventListeners.delete(listener);
      };
    },
    onError(listener: (error: NativeError) => void): () => void {
      errorListeners.add(listener);
      return () => {
        errorListeners.delete(listener);
      };
    },
    result: handle.result.catch((error) => Promise.reject(toNativeError(error))),
    cancel(): void {
      handle.cancel();
    },
  };
}

function wrapNativeChannel<TIn, TOut>(
  channel: LxChannel<TOut, TIn>,
): NativeChannel<TIn, TOut> {
  const messageListeners = new Set<(message: TOut) => void>();
  const closeListeners = new Set<(event: ChannelCloseEvent) => void>();
  channel.on("data", (message) => {
    for (const listener of messageListeners) listener(message);
  });
  channel.on("close", (code, reason) => {
    const event = { code, reason };
    for (const listener of closeListeners) listener(event);
  });
  return {
    send(message: TIn): void {
      channel.send(message);
    },
    onMessage(listener: (message: TOut) => void): () => void {
      messageListeners.add(listener);
      return () => {
        messageListeners.delete(listener);
      };
    },
    onClose(listener: (event: ChannelCloseEvent) => void): () => void {
      closeListeners.add(listener);
      return () => {
        closeListeners.delete(listener);
      };
    },
    close(code?: string, reason?: string): void {
      channel.close(code, reason);
    },
  };
}

export function invoke<TResult = unknown, TInput = void>(
  route: string,
  input?: TInput,
  options?: InvokeOptions,
): Promise<TResult> {
  return LingXiaBridge.invoke(route, input, options);
}

export function notify<TInput = void>(
  route: string,
  input?: TInput,
  options?: NotifyOptions,
): void {
  rawBridge.notify(nativeRoute(route), input, nativeOptions(options));
}

export function stream<TEvent = unknown, TResult = void, TInput = void>(
  route: string,
  input?: TInput,
  options?: StreamOptions,
): NativeStream<TEvent, TResult> {
  return wrapNativeStream<TEvent, TResult>(
    rawBridge.stream(nativeRoute(route), input, nativeOptions(options)) as LxStream<
      TEvent,
      TResult
    >,
  );
}

export function channel<TIn = unknown, TOut = unknown>(
  route: string,
  input?: unknown,
  options?: ChannelOptions,
): Promise<NativeChannel<TIn, TOut>> {
  return LingXiaBridge.channel(route, input, options);
}

export function initBridge(): void {
  if (window.__LX_BRIDGE_INIT_STATE) {
    log(
      `Bridge already ${window.__LX_BRIDGE_INIT_STATE}, skipping duplicate init`,
    );
    return;
  }

  window.__LX_BRIDGE_INIT_STATE = "initializing";

  try {
    log(`Method: ${communicationMethod}`);
    activateReceiver(LingXiaBridge._receiveEvaluateMessage);

    if (useAppleDownstreamTransport()) {
      ensureAppleDownstream();
    } else if (communicationMethod === MESSAGE_PORT_TYPE) {
      installMessagePortInitListener();
      getMessagePort().catch((e) => warn("Port init failed:", e));
    } else if (
      communicationMethod === "webkit" ||
      communicationMethod === JS_INTERFACE_TYPE ||
      communicationMethod === WEB_MESSAGE_TYPE
    ) {
      startHandshake();
    } else {
      warn("Unknown method");
    }

    window.LingXiaBridge = LingXiaBridge;
    // Dev hook so repro/test pages can simulate a WebKit stream replacement.
    (window as unknown as Record<string, unknown>).__lxForceDownstreamReconnect =
      forceDownstreamReconnect;
    installNativeComponentCoverageMonitor({
      os: getPlatformOS(),
      send: sendNativeComponentMessage,
    });
    window.__LX_BRIDGE_INIT_STATE = "initialized";
    log("Init complete");
  } catch (e) {
    delete window.__LX_BRIDGE_INIT_STATE;
    throw e;
  }
}
