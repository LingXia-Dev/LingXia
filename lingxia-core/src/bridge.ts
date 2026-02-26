import type {
  CallOptions,
  DataSubscriber,
  LingXiaBridgeInterface,
  LxBridgeError,
  LxMethod,
  LxMethodParams,
  LxMethodResult,
  NativeComponentMessage,
  NotifyOptions,
} from './types';
import { BRIDGE_ERROR } from './types';
import { installNativeComponentCoverageMonitor } from './nativecomponents/coverage-monitor';
import {
  BRIDGE_CONFIG,
  getCommunicationMethod,
  getPlatformOS,
  isAndroid,
  isHarmony,
  isIOS,
  isMacOS,
  isDesktop,
} from './runtime-env';

const NATIVE_HANDLER_NAME = 'LingXia';
const GLOBAL_RECEIVER_NAME = '__LingXiaRecvMessage';
const DEFAULT_TIMEOUT_MS = 5000;
const HANDSHAKE_TIMEOUT_MS = 10000;
const HANDSHAKE_MAX_RETRIES = 3;
const LOG_PREFIX = '[LX.Bridge]';
const MESSAGE_PORT_TYPE = 'messageport';
const JS_INTERFACE_TYPE = 'jsinterface';
const OUTBOX_LIMIT = 256;

const debugFlags = { data: false, proto: false, all: false };

function isDebugEnabled(flag: keyof typeof debugFlags): boolean {
  return debugFlags.all || debugFlags[flag];
}

function log(...args: unknown[]): void { console.log(LOG_PREFIX, ...args); }
function warn(...args: unknown[]): void { console.warn(LOG_PREFIX, ...args); }
function error(...args: unknown[]): void { console.error(LOG_PREFIX, ...args); }

function safeStringify(obj: unknown, space?: number): string {
  const seen = new WeakSet();
  return JSON.stringify(obj, (_key, value) => {
    if (typeof value === 'object' && value !== null) {
      if (seen.has(value)) return '[Circular]';
      seen.add(value);
    }
    return value;
  }, space);
}

function deepCopy<T>(data: T): T {
  try {
    if (typeof structuredClone === 'function') return structuredClone(data);
    return JSON.parse(JSON.stringify(data));
  } catch {
    return {} as T;
  }
}

function unknownToError(err: unknown, fallbackMsg: string): LxBridgeError {
  if (err && typeof err === 'object') {
    const source = err as { code?: unknown; message?: unknown; data?: unknown };
    const hasValidCode =
      (typeof source.code === 'string' && source.code.trim() !== '') ||
      (typeof source.code === 'number' && Number.isFinite(source.code));
    const code = hasValidCode ? source.code! : BRIDGE_ERROR.INTERNAL_ERROR;
    const message =
      typeof source.message === 'string' && source.message.trim() !== ''
        ? source.message
        : fallbackMsg;
    const output: LxBridgeError = { code, message };
    if ('data' in source) output.data = source.data;
    return output;
  }

  const message = err instanceof Error ? err.message : typeof err === 'string' ? err : fallbackMsg;
  return { code: BRIDGE_ERROR.INTERNAL_ERROR, message };
}

const communicationMethod = getCommunicationMethod();

// Transport
let messagePort: MessagePort | null = null;
const portInitState = {
  listenerInstalled: false,
  promise: null as Promise<MessagePort> | null,
  resolve: null as ((port: MessagePort) => void) | null,
  reject: null as ((err: unknown) => void) | null,
  timer: null as number | null,
};

function installMessagePortInitListener(): void {
  if (portInitState.listenerInstalled) return;
  portInitState.listenerInstalled = true;
  window.addEventListener('message', (event: MessageEvent) => {
    if (event.data !== 'LingXia-port-init') return;
    const port = event.ports?.[0];
    if (!port) return;
    LingXiaBridge._connectWebMessagePort(port);
    portInitState.resolve?.(port);
  });
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
    portInitState.resolve = (port: MessagePort): void => { cleanupPortInit(); resolve(port); };
    portInitState.reject = (err: unknown): void => { cleanupPortInit(); reject(err); };
    portInitState.timer = window.setTimeout(() => {
      portInitState.reject?.(new Error(`MessagePort init timed out after ${timeoutMs}ms`));
    }, timeoutMs);
  });

  try { window.LingXiaProxy?.getPort('LingXiaPort'); }
  catch (e) { portInitState.reject?.(e); }

  return portInitState.promise;
}

function postToNative(message: unknown): void {
  const kind = (message as { kind?: string }).kind;
  if (kind === 'req' || kind === 'notify') log(`postToNative: ${kind} ${(message as { method?: string }).method}`);
  if (isDebugEnabled('proto')) console.log('→', JSON.stringify(message, null, 2));
  try {
    if (communicationMethod === 'webkit') {
      window.webkit?.messageHandlers[NATIVE_HANDLER_NAME]?.postMessage(message);
      return;
    }
    const messageString = safeStringify(message);
    if (communicationMethod === MESSAGE_PORT_TYPE && messagePort) {
      messagePort.postMessage(messageString);
      return;
    }
    if (communicationMethod === JS_INTERFACE_TYPE && window.LingXiaProxy?.postMessage) {
      window.LingXiaProxy.postMessage(messageString);
      return;
    }
    warn('Transport not ready');
  } catch (e) {
    error('Send error:', e);
  }
}

// V2 Protocol Types
type Trace = { traceId?: string; spanId?: string };
type Hello = { v: 2; kind: 'hello'; nonce: string; role: 'view'; protocolsSupported: number[]; trace?: Trace };
type HelloAck = { v: 2; kind: 'helloAck'; nonce: string; protocol: number; sessionId: string };
type Ready = { v: 2; kind: 'ready'; sessionId: string };
type Req = { v: 2; kind: 'req'; id: string; method: string; params?: unknown; cap: string; trace?: Trace };
type Res = { v: 2; kind: 'res'; id: string; ok: boolean; result?: unknown; error?: LxBridgeError; trace?: Trace };
type Notify = { v: 2; kind: 'notify'; method: string; params?: unknown; cap: string; trace?: Trace };
type Cancel = { v: 2; kind: 'cancel'; id: string };
type StateSnapshot = { v: 2; kind: 'state.snapshot'; scope?: string; rev: number; state: Record<string, unknown> };
type JsonPatchOp = { op: 'add'; path: string; value: unknown } | { op: 'replace'; path: string; value: unknown } | { op: 'remove'; path: string };
type StatePatch = { v: 2; kind: 'state.patch'; scope?: string; baseRev: number; rev: number; ops: JsonPatchOp[]; ack?: boolean };
type StateAck = { v: 2; kind: 'state.ack'; scope?: string; rev: number };
type Incoming = HelloAck | Ready | Res | Req | StateSnapshot | StatePatch;

// Handshake state
let handshakeSessionId: string | null = null;
let handshakeDone = false;
let helloSent = false;
let handshakeRetryCount = 0;
let handshakeTimer: ReturnType<typeof setTimeout> | null = null;

// Request tracking
let requestCounter = 0;
interface PendingRequest {
  method: string;
  resolve: (v: unknown) => void;
  reject: (e: LxBridgeError) => void;
  timeoutMs: number;
  timerId: ReturnType<typeof setTimeout> | null;
}
const pendingReq = new Map<string, PendingRequest>();

// Outbox
interface OutboxItem { msg: unknown; reqId?: string; }
const outbox: OutboxItem[] = [];

// State sync
let stateRev = -1;
let pageData: Record<string, unknown> = {};
const dataSubscribers = new Set<DataSubscriber>();
const subscriberInitStatus = new WeakMap<DataSubscriber, boolean>();

// Logic→View method handlers (for incoming req from native side)
const viewMethodHandlers = new Map<string, (params: unknown) => unknown | Promise<unknown>>();

export function registerViewMethodHandler(
  method: string,
  handler: (params: unknown) => unknown | Promise<unknown>,
): void {
  viewMethodHandlers.set(method, handler);
}

function inferCap(method: string): string {
  if (method.startsWith('host.')) return 'host';
  const i = method.indexOf('.');
  return i > 0 ? method.slice(0, i) : 'page';
}

function normalizeParams(params: unknown): unknown | undefined {
  if (params === null || params === undefined) return undefined;
  return params;
}

function isTransportReady(): boolean {
  if (communicationMethod === MESSAGE_PORT_TYPE) return !!messagePort;
  return communicationMethod === 'webkit' || communicationMethod === JS_INTERFACE_TYPE;
}

function canSendAppMessages(): boolean {
  return isTransportReady() && handshakeDone;
}

function rejectPendingRequest(reqId: string, err: LxBridgeError): void {
  const info = pendingReq.get(reqId);
  if (info) {
    pendingReq.delete(reqId);
    if (info.timerId !== null) clearTimeout(info.timerId);
    info.reject(err);
  }
}

function startRequestTimer(reqId: string): void {
  const info = pendingReq.get(reqId);
  if (!info || info.timerId !== null) return;
  info.timerId = setTimeout(() => {
    if (!pendingReq.has(reqId)) return;
    pendingReq.delete(reqId);
    info.reject({ code: BRIDGE_ERROR.TIMEOUT, message: `'${info.method}' timed out` });
  }, info.timeoutMs);
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
  const isHandshake = kind === 'hello' || kind === 'helloAck' || kind === 'ready';

  if (!canSendAppMessages() && !isHandshake) {
    if (outbox.length >= OUTBOX_LIMIT) {
      error('Outbox full');
      if (reqId) {
        rejectPendingRequest(reqId, {
          code: BRIDGE_ERROR.OUTBOX_FULL,
          message: "Bridge outbox is full",
        });
      }
      return;
    }
    outbox.push({ msg, reqId });
    return;
  }
  if (reqId) startRequestTimer(reqId);
  postToNative(msg);
}

function flushOutbox(): void {
  if (!canSendAppMessages()) return;
  while (outbox.length) {
    const item = outbox.shift();
    if (!item) continue;
    if (item.reqId) startRequestTimer(item.reqId);
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
    kind: 'hello',
    nonce: BRIDGE_CONFIG.nonce || '',
    role: 'view',
    protocolsSupported: [2],
  };

  helloSent = true;
  postToNative(hello);

  handshakeTimer = setTimeout(() => {
    if (handshakeDone) return;
    handshakeRetryCount++;
    if (handshakeRetryCount < HANDSHAKE_MAX_RETRIES) {
      warn(`Handshake timeout (${handshakeRetryCount}/${HANDSHAKE_MAX_RETRIES}), retrying...`);
      helloSent = false;
      startHandshake();
    } else {
      error('Handshake failed');
      clearHandshakeTimer();
      helloSent = false;
      handshakeRetryCount = 0;
      while (outbox.length) {
        const item = outbox.shift();
        if (item?.reqId) {
          rejectPendingRequest(item.reqId, {
            code: BRIDGE_ERROR.HANDSHAKE_FAILED,
            message: "Bridge handshake failed",
          });
        }
      }
    }
  }, HANDSHAKE_TIMEOUT_MS);
}

function parseIncoming(msg: unknown): Incoming | null {
  if (!msg || typeof msg !== 'object') return null;
  const v = (msg as { v?: unknown }).v;
  const kind = (msg as { kind?: unknown }).kind;
  if (v !== 2 || typeof kind !== 'string') return null;
  return msg as Incoming;
}

// JSON Patch
function jsonPointerUnescape(seg: string): string {
  return seg.replace(/~1/g, '/').replace(/~0/g, '~');
}

function parseJsonPointer(path: string): string[] {
  if (path === '') return [];
  if (!path.startsWith('/')) throw new Error(`Invalid JSON pointer: ${path}`);
  return path.split('/').slice(1).map(jsonPointerUnescape);
}

function getContainerAndKey(root: Record<string, unknown>, pointer: string, autoCreate = false): { container: unknown; key: string } {
  const segments = parseJsonPointer(pointer);
  if (segments.length === 0) return { container: { $root: root }, key: '$root' };

  let current: unknown = root;
  for (let i = 0; i < segments.length - 1; i++) {
    const seg = segments[i]!;
    if (Array.isArray(current)) {
      current = current[Number(seg)];
    } else if (current && typeof current === 'object') {
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

function applyJsonPatch(target: Record<string, unknown>, ops: JsonPatchOp[]): void {
  for (const op of ops) {
    const autoCreate = op.op === 'add' || op.op === 'replace';
    const { container, key } = getContainerAndKey(target, op.path, autoCreate);
    if (key === '$root') {
      for (const k of Object.keys(target)) delete target[k];
      if (op.op !== 'remove') {
        const v = (op as { value: unknown }).value;
        if (v && typeof v === 'object') Object.assign(target, v as Record<string, unknown>);
      }
      continue;
    }

    if (Array.isArray(container)) {
      const idx = key === '-' ? container.length : Number(key);
      if (!Number.isFinite(idx) || idx < 0) throw new Error(`Invalid index`);
      if (op.op === 'remove') container.splice(idx, 1);
      else if (op.op === 'add') container.splice(idx, 0, op.value);
      else if (op.op === 'replace') container[idx] = op.value;
      continue;
    }

    if (!container || typeof container !== 'object') throw new Error(`Invalid container`);
    const obj = container as Record<string, unknown>;
    if (op.op === 'remove') delete obj[key];
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
      warn('Subscriber error:', e);
    }
  });
}

function requestStateRecovery(scope?: string): void {
  LingXiaBridge.call('state.getSnapshot', { scope }, { cap: 'state', timeoutMs: 10000 }).catch(() => {});
}

function applySnapshotFromResult(result: unknown): boolean {
  if (!result || typeof result !== 'object') return false;
  const obj = result as { rev?: unknown; state?: unknown };
  if (typeof obj.rev !== 'number' || !Number.isFinite(obj.rev)) return false;
  if (!obj.state || typeof obj.state !== 'object') return false;
  pageData = obj.state as Record<string, unknown>;
  stateRev = obj.rev;
  if (isDebugEnabled('data')) { console.group('[LX] snapshot(res)'); console.log('rev:', stateRev, 'state:', deepCopy(pageData)); console.groupEnd(); }
  notifyStateSubscribers(true);
  return true;
}

function handleIncomingMessage(msg: unknown): void {
  // Handle native component events (from Android NativeBridge.sendEventToView)
  if (msg && typeof msg === 'object') {
    const obj = msg as { type?: string; name?: string; payload?: NativeComponentMessage };
    if (obj.type === 'event' && obj.name === 'nativecomponent' && obj.payload) {
      const payload = obj.payload;
      const componentId = (payload as { id?: string; componentId?: string }).id || (payload as { componentId?: string }).componentId;
      if (typeof componentId === 'string') {
        const handler = nativeComponentHandlers.get(componentId);
        if (handler) {
          try { handler(payload); } catch (e) { error('NC handler error:', e); }
        }
      }
      return;
    }
  }

  const message = parseIncoming(msg);
  if (!message) { warn('Invalid V2 message:', msg); return; }

  switch (message.kind) {
    case 'helloAck':
      handshakeSessionId = message.sessionId;
      return;

    case 'ready':
      if (handshakeSessionId && message.sessionId !== handshakeSessionId) { warn('sessionId mismatch'); return; }
      clearHandshakeTimer();
      handshakeDone = true;
      handshakeRetryCount = 0;
      if (isDebugEnabled('proto')) log('Handshake complete');
      flushOutbox();
      return;

    case 'res': {
      const info = pendingReq.get(message.id);
      if (!info) return;
      pendingReq.delete(message.id);
      if (info.timerId !== null) clearTimeout(info.timerId);
      if (message.ok) {
        if (info.method === 'state.getSnapshot') {
          if (!applySnapshotFromResult(message.result)) {
            warn('Invalid state.getSnapshot result');
          }
        }
        info.resolve(message.result);
      } else {
        info.reject(message.error ?? { code: BRIDGE_ERROR.INTERNAL_ERROR, message: `Call '${info.method}' failed` });
      }
      return;
    }

    case 'state.snapshot':
      pageData = message.state || {};
      stateRev = message.rev;
      if (isDebugEnabled('data')) { console.group('[LX] snapshot'); console.log('rev:', stateRev, 'state:', deepCopy(pageData)); console.groupEnd(); }
      notifyStateSubscribers(true);
      return;

    case 'state.patch':
      if (message.baseRev !== stateRev) {
        warn('baseRev mismatch', { have: stateRev, want: message.baseRev });
        requestStateRecovery(message.scope);
        return;
      }
      try {
        applyJsonPatch(pageData, message.ops || []);
        stateRev = message.rev;
      } catch (e) {
        error('Patch failed:', e);
        requestStateRecovery(message.scope);
        return;
      }
      if (isDebugEnabled('data')) { console.group('[LX] patch'); console.log('rev:', stateRev, 'ops:', message.ops); console.groupEnd(); }
      notifyStateSubscribers(false);
      if (message.ack) send({ v: 2, kind: 'state.ack', scope: message.scope, rev: message.rev } as StateAck);
      return;

    case 'req': {
      const reqMsg = message as Req;
      const requiredCap = inferCap(reqMsg.method);
      if (!reqMsg.cap || reqMsg.cap !== requiredCap) {
        send({ v: 2, kind: 'res', id: reqMsg.id, ok: false, error: { code: BRIDGE_ERROR.CAPABILITY_DENIED, message: `Invalid cap for ${reqMsg.method}` } } as Res);
        return;
      }
      const handler = viewMethodHandlers.get(reqMsg.method);
      if (!handler) {
        send({ v: 2, kind: 'res', id: reqMsg.id, ok: false, error: { code: BRIDGE_ERROR.METHOD_NOT_FOUND, message: `View handler not found: ${reqMsg.method}` } } as Res);
        return;
      }
      Promise.resolve()
        .then(() => handler(reqMsg.params))
        .then((result) => {
          send({ v: 2, kind: 'res', id: reqMsg.id, ok: true, result } as Res);
        })
        .catch((err) => {
          send({ v: 2, kind: 'res', id: reqMsg.id, ok: false, error: unknownToError(err, `View handler '${reqMsg.method}' failed`) } as Res);
        });
      return;
    }
  }
}

// Native components
const nativeComponentHandlers = new Map<string, (message: NativeComponentMessage) => void>();
const nativeComponentQueue: NativeComponentMessage[] = [];
let nativeComponentReady = false;

function hasNativeComponentHandler(): boolean {
  if (typeof window === 'undefined') return false;
  return !!(window.webkit?.messageHandlers?.NativeComponent || window.NativeComponentBridge?.postMessage);
}

function postNativeComponentMessage(message: NativeComponentMessage): void {
  try {
    if (window.webkit?.messageHandlers?.NativeComponent) { window.webkit.messageHandlers.NativeComponent.postMessage(message); return; }
    if (window.NativeComponentBridge?.postMessage) { window.NativeComponentBridge.postMessage(safeStringify(message)); return; }
  } catch (e) { error('NativeComponent send error:', e); }
}

function flushNativeComponentQueue(): void {
  if (!hasNativeComponentHandler() || nativeComponentQueue.length === 0) return;
  nativeComponentReady = true;
  while (nativeComponentQueue.length) {
    const msg = nativeComponentQueue.shift()!;
    try { postNativeComponentMessage(msg); }
    catch { break; }
  }
}

function sendNativeComponentMessage(message: NativeComponentMessage): void {
  try {
    if (!hasNativeComponentHandler()) { nativeComponentQueue.push(message); return; }
    if (!nativeComponentReady) flushNativeComponentQueue();
    postNativeComponentMessage(message);
  } catch (e) { error('NC send failed:', e); }
}

// Public interface
export const LingXiaBridge: LingXiaBridgeInterface = {
  call<M extends LxMethod>(method: M | string, params?: LxMethodParams<M> | unknown, options?: CallOptions): Promise<LxMethodResult<M> | unknown> {
    return new Promise((resolve, reject) => {
      if (!method || typeof method !== 'string') {
        reject({ code: BRIDGE_ERROR.MALFORMED_MESSAGE, message: "Method name must be a non-empty string" });
        return;
      }
      if (!helloSent) startHandshake();

      const id = `c_${Date.now()}_${requestCounter++}`;
      const cap = options?.cap || inferCap(method);
      const timeoutMs = options?.timeoutMs ?? DEFAULT_TIMEOUT_MS;
      pendingReq.set(id, { method, resolve, reject: (e) => reject(e), timeoutMs, timerId: null });

      const req: Req = { v: 2, kind: 'req', id, method, params: normalizeParams(params), cap };
      send(req, id);

      const signal = options?.signal;
      if (signal) {
        if (signal.aborted) {
          removeOutboxByReqId(id);
          rejectPendingRequest(id, {
            code: BRIDGE_ERROR.CANCELED,
            message: "Bridge request aborted",
          });
          if (handshakeDone) send({ v: 2, kind: 'cancel', id } as Cancel);
          return;
        }
        const onAbort = (): void => {
          signal.removeEventListener('abort', onAbort);
          const removed = removeOutboxByReqId(id);
          rejectPendingRequest(id, {
            code: BRIDGE_ERROR.CANCELED,
            message: "Bridge request aborted",
          });
          if (!removed && handshakeDone) send({ v: 2, kind: 'cancel', id } as Cancel);
        };
        signal.addEventListener('abort', onAbort);
      }
    });
  },

  notify(method: string, params?: unknown, options?: NotifyOptions): void {
    if (!method || typeof method !== 'string') return;
    if (!helloSent) startHandshake();
    send({ v: 2, kind: 'notify', method, params: normalizeParams(params), cap: options?.cap || inferCap(method) } as Notify);
  },

  subscribe(callback: DataSubscriber): () => void {
    if (typeof callback !== 'function') return () => {};
    dataSubscribers.add(callback);
    if (stateRev >= 0) {
      subscriberInitStatus.set(callback, true);
      try { callback(deepCopy(pageData), { rev: stateRev, initial: true }); }
      catch (e) { error('Callback error:', e); }
    }
    return () => { dataSubscribers.delete(callback); subscriberInitStatus.delete(callback); };
  },

  _connectWebMessagePort(port: MessagePort): void {
    if (communicationMethod !== MESSAGE_PORT_TYPE) return;
    if (messagePort && messagePort !== port) { try { messagePort.onmessage = null; messagePort.close(); } catch {} }
    messagePort = port;
    port.onmessage = (event: MessageEvent) => {
      let data = event.data;
      if (typeof data === 'string') { try { data = JSON.parse(data); } catch { return; } }
      handleIncomingMessage(data);
    };
    // Some WebView MessagePort implementations (notably Android WebMessagePort)
    // require an explicit start() to begin dispatching onmessage events.
    try { port.start(); } catch {}
    log('Port connected');
    startHandshake();
  },

  _receiveEvaluateMessage(messageString: string): void {
    try { if (messageString) handleIncomingMessage(JSON.parse(messageString)); }
    catch (e) { error('Parse error:', e); }
  },

  debug: new Proxy(debugFlags, {
    get(target, prop: keyof typeof debugFlags) { return target[prop]; },
    set(target, prop: keyof typeof debugFlags, value: boolean) {
      if (prop in target) { target[prop] = !!value; console.log(`[LX] ${prop}: ${value}`); return true; }
      return false;
    },
  }),

  platform: { isHarmony, isIOS, isAndroid, isMacOS, isDesktop, getOS: getPlatformOS },

  dom: {
    measureById(id: string): [number, number, number, number, number] | null {
      try {
        if (!id) return null;
        const el = document.getElementById(id);
        if (!el) return null;
        const r = el.getBoundingClientRect();
        let radius = 0;
        try { radius = parseFloat(getComputedStyle(el).borderRadius) || 0; } catch {}
        return [r.left + window.scrollX, r.top + window.scrollY, r.width, r.height, radius];
      } catch { return null; }
    },
  },

  nativeComponents: {
    send: sendNativeComponentMessage,
    hasHandler: hasNativeComponentHandler,
    flush: flushNativeComponentQueue,
    register(id: string, handler: (message: NativeComponentMessage) => void): () => void {
      if (!id || typeof handler !== 'function') return () => {};
      nativeComponentHandlers.set(id, handler);
      return () => nativeComponentHandlers.delete(id);
    },
    unregister(id: string): void { nativeComponentHandlers.delete(id); },
  },

  isReady(): boolean { return handshakeDone; },
};

export const host: Record<string, (...args: unknown[]) => Promise<unknown>> = new Proxy({}, {
  get(_target, prop: string) {
    return (...args: unknown[]): Promise<unknown> => {
      const payload = args.length === 0 ? undefined : args.length === 1 ? args[0] : args;
      return LingXiaBridge.call(`host.${prop}`, payload, { cap: 'host' });
    };
  },
});

export function initBridge(): void {
  log(`Method: ${communicationMethod}`);
  window[GLOBAL_RECEIVER_NAME] = LingXiaBridge._receiveEvaluateMessage;

  if (communicationMethod === MESSAGE_PORT_TYPE) {
    installMessagePortInitListener();
    getMessagePort().catch((e) => warn('Port init failed:', e));
  } else if (communicationMethod === 'webkit' || communicationMethod === JS_INTERFACE_TYPE) {
    startHandshake();
  } else {
    warn('Unknown method');
  }

  window.LingXiaBridge = LingXiaBridge;
  window.host = host;
  installNativeComponentCoverageMonitor({ os: getPlatformOS(), send: sendNativeComponentMessage });
  log('Init complete');
}
