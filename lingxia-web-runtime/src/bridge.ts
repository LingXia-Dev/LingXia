import type {
  BridgeConfig,
  BridgeMessage,
  DataSubscriber,
  LingXiaBridgeInterface,
  PendingCall,
  ReplyPayload,
  SameLevelMessage,
} from './types';

const NATIVE_HANDLER_NAME = 'LingXia';
const GLOBAL_RECEIVER_NAME = '__LingXiaRecvMessage';
const CALL_TIMEOUT_MS = 5000;
const LOG_PREFIX = '[LX.Bridge]';
const MESSAGE_PORT_TYPE = 'messageport';

const debugFlags = {
  data: false,
  proto: false,
  all: false,
};

function isDebugEnabled(flag: keyof typeof debugFlags): boolean {
  return debugFlags.all || debugFlags[flag];
}

function safeStringify(obj: unknown, space?: number): string {
  const seen = new WeakSet();
  return JSON.stringify(
    obj,
    (_key, value) => {
      if (typeof value === 'object' && value !== null) {
        if (seen.has(value)) {
          return '[Circular Reference]';
        }
        seen.add(value);
      }
      return value;
    },
    space
  );
}

let messageCounter = 0;
const pendingCalls = new Map<string, PendingCall>();
let pageData: Record<string, unknown> = {};
const dataSubscribers = new Set<DataSubscriber>();
const subscriberInitStatus = new WeakMap<DataSubscriber, boolean>();
let messagePort: MessagePort | null = null;

const BRIDGE_CONFIG: BridgeConfig =
  (typeof window !== 'undefined' && window.__LX_BRIDGE_CFG) || {};

const communicationMethod = ((): string => {
  if (BRIDGE_CONFIG.method === 'messageport') return MESSAGE_PORT_TYPE;
  if (BRIDGE_CONFIG.method === 'webkit') return 'webkit';
  return 'unknown';
})();

function isHarmony(): boolean {
  return BRIDGE_CONFIG.os === 'Harmony';
}
function isIOS(): boolean {
  return BRIDGE_CONFIG.os === 'iOS';
}
function isAndroid(): boolean {
  return BRIDGE_CONFIG.os === 'Android';
}
function getPlatformOS(): string {
  return BRIDGE_CONFIG.os || 'unknown';
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

function deepCopy<T>(data: T): T {
  try {
    if (typeof structuredClone === 'function') {
      return structuredClone(data);
    } else {
      return JSON.parse(JSON.stringify(data));
    }
  } catch (e) {
    error('Failed to deep copy data:', e);
    return {} as T;
  }
}

function setValueByPath(
  obj: Record<string, unknown>,
  path: string,
  value: unknown
): boolean {
  if (
    typeof path !== 'string' ||
    path === '' ||
    typeof obj !== 'object' ||
    obj === null
  ) {
    return false;
  }

  const parts = path.replace(/\[(\d+)\]/g, '.$1').split('.');
  let current: Record<string, unknown> = obj;

  for (let i = 0; i < parts.length - 1; i++) {
    const key = parts[i];
    const nextKey = parts[i + 1];
    const isNextKeyArrayIndex = /^\d+$/.test(nextKey);

    if (current[key] === undefined || current[key] === null) {
      current[key] = isNextKeyArrayIndex ? [] : {};
    } else if (typeof current[key] !== 'object') {
      current[key] = isNextKeyArrayIndex ? [] : {};
    } else if (isNextKeyArrayIndex && !Array.isArray(current[key])) {
      current[key] = [];
    }
    current = current[key] as Record<string, unknown>;
    if (typeof current !== 'object' || current === null) {
      return false;
    }
  }

  const finalKey = parts[parts.length - 1];
  current[finalKey] = value;
  return true;
}

function deleteValueByPath(
  obj: Record<string, unknown>,
  path: string
): boolean {
  if (
    typeof path !== 'string' ||
    path === '' ||
    typeof obj !== 'object' ||
    obj === null
  ) {
    return false;
  }

  const parts = path.replace(/\[(\d+)\]/g, '.$1').split('.');
  let current: Record<string, unknown> = obj;

  for (let i = 0; i < parts.length - 1; i++) {
    const key = parts[i];
    if (typeof current[key] !== 'object' || current[key] === null) {
      return false;
    }
    current = current[key] as Record<string, unknown>;
  }

  const finalKey = parts[parts.length - 1];
  if (Array.isArray(current)) {
    const index = parseInt(finalKey, 10);
    if (!isNaN(index) && index >= 0 && index < current.length) {
      current.splice(index, 1);
      return true;
    }
  } else if (typeof current === 'object') {
    delete current[finalKey];
    return true;
  }
  return false;
}

function applyPatch(
  target: Record<string, unknown>,
  patch: Record<string, unknown>
): Record<string, unknown> {
  if (
    typeof target !== 'object' ||
    target === null ||
    typeof patch !== 'object' ||
    patch === null
  ) {
    return patch;
  }

  let changesApplied = false;
  for (const path in patch) {
    if (Object.prototype.hasOwnProperty.call(patch, path)) {
      const value = patch[path];
      if (value === undefined) {
        if (deleteValueByPath(target, path)) changesApplied = true;
      } else {
        if (setValueByPath(target, path, value)) changesApplied = true;
      }
    }
  }
  return changesApplied ? patch : {};
}

function sendMessageToNative(message: BridgeMessage): void {
  if (isDebugEnabled('proto')) {
    console.log('→', JSON.stringify(message, null, 2));
  }
  try {
    if (communicationMethod === 'webkit') {
      window.webkit?.messageHandlers[NATIVE_HANDLER_NAME]?.postMessage(message);
    } else if (communicationMethod === MESSAGE_PORT_TYPE && messagePort) {
      const messageString = safeStringify(message);
      messagePort.postMessage(messageString);
    } else {
      warn('Bridge not ready for sending');
    }
  } catch (e) {
    error('Send message error:', e, message);
  }
}

function getMessagePort(): Promise<MessagePort> {
  return new Promise((resolve) => {
    window.LingXiaProxy?.getPort('LingXiaPort');

    const handlePortInit = (event: MessageEvent): void => {
      if (event.data === 'LingXia-port-init') {
        window.removeEventListener('message', handlePortInit);
        const port = event.ports[0];
        LingXiaBridge._connectWebMessagePort(port);
        resolve(port);
      }
    };

    window.addEventListener('message', handlePortInit);
  });
}

function handleReply(replyMessage: BridgeMessage): void {
  const msgId = replyMessage.msgId;
  if (!msgId || !pendingCalls.has(msgId)) {
    warn('Reply for unknown msgId:', replyMessage);
    return;
  }

  const callInfo = pendingCalls.get(msgId)!;
  pendingCalls.delete(msgId);
  clearTimeout(callInfo.timerId);

  try {
    const payload = replyMessage.payload as ReplyPayload;
    if (payload?.success === true) {
      if (payload.hasOwnProperty('result')) {
        callInfo.resolve(payload.result);
      } else {
        callInfo.resolve();
      }
    } else if (payload?.success === false) {
      callInfo.reject(payload.error || { message: 'Unknown error' });
    } else {
      callInfo.reject({ message: 'Invalid reply payload' });
    }
  } catch (e) {
    error('Reply processing error:', e);
  }
}

function sendCallback(callbackId: string): void {
  sendMessageToNative({
    msgId: null,
    type: 'callback',
    callbackId: callbackId,
  });
}

const sameLevelHandlers = new Map<
  string,
  (message: SameLevelMessage) => void
>();
const sameLevelQueue: SameLevelMessage[] = [];
let sameLevelReady = false;

function hasSameLevelHandler(): boolean {
  if (typeof window === 'undefined') return false;

  if (window.webkit?.messageHandlers?.SameLevel) {
    return true;
  }

  if (
    window.SameLevelNative &&
    typeof window.SameLevelNative.postMessage === 'function'
  ) {
    return true;
  }

  return false;
}

function postSameLevelMessage(message: SameLevelMessage): void {
  if (window.webkit?.messageHandlers?.SameLevel) {
    window.webkit.messageHandlers.SameLevel.postMessage(message);
    return;
  }

  if (
    window.SameLevelNative &&
    typeof window.SameLevelNative.postMessage === 'function'
  ) {
    const msgString = safeStringify(message);
    window.SameLevelNative.postMessage(msgString);
    return;
  }
}

function flushSameLevelQueue(): void {
  if (!hasSameLevelHandler() || sameLevelQueue.length === 0) return;
  sameLevelReady = true;
  while (sameLevelQueue.length) {
    const msg = sameLevelQueue.shift()!;
    try {
      if (isDebugEnabled('proto')) {
        console.log('[SameLevel] flush → native:', msg);
      }
      postSameLevelMessage(msg);
    } catch (e) {
      error('Failed to flush SameLevel message:', e);
      break;
    }
  }
}

function sendSameLevelMessage(message: SameLevelMessage): void {
  try {
    const hasHandler = hasSameLevelHandler();
    if (!hasHandler) {
      console.log('[SameLevel] queue message (no handler yet):', message);
      sameLevelQueue.push(message);
      return;
    }
    if (!sameLevelReady) {
      flushSameLevelQueue();
    }
    if (isDebugEnabled('proto')) {
      console.log('[SameLevel] → native:', message);
    }
    postSameLevelMessage(message);
  } catch (e) {
    error('Failed to send SameLevel message:', e);
  }
}

function handleSameLevelEvent(msg: unknown): void {
  try {
    const message: SameLevelMessage | null =
      typeof msg === 'string'
        ? JSON.parse(msg)
        : msg && typeof msg === 'object'
          ? (msg as SameLevelMessage)
          : null;

    if (!message || !message.id) {
      warn('SameLevel receive: invalid message', msg);
      return;
    }
    if (message.action !== 'component.event') return;

    const handler = sameLevelHandlers.get(message.id);
    if (typeof handler !== 'function') return;
    if (isDebugEnabled('proto')) {
      console.log('[SameLevel] ← native:', message);
    }
    handler(message);
  } catch (e) {
    error('SameLevel receive error:', e);
  }
}

function handleEvent(eventMessage: BridgeMessage): void {
  const { name, payload } = eventMessage;

  if (name === 'setData') {
    let dataToApply: Record<string, unknown>;
    let callbackId: string | null = null;

    const p = payload as { data?: Record<string, unknown>; callbackId?: string };
    if (p && typeof p.data !== 'undefined') {
      dataToApply = p.data;
      callbackId = p.callbackId || null;
    } else {
      dataToApply = payload as Record<string, unknown>;
    }

    if (isDebugEnabled('data')) {
      console.group('[LingXia Debug] setData Update');
      console.log('Previous data:', JSON.parse(safeStringify(pageData)));
      console.log('Applying patch:', dataToApply);
    }

    applyPatch(pageData, dataToApply);

    if (isDebugEnabled('data')) {
      console.log('Updated data:', JSON.parse(safeStringify(pageData)));
      console.log('Active subscribers:', dataSubscribers.size);
      console.groupEnd();
    }

    dataSubscribers.forEach((listener) => {
      try {
        if (!subscriberInitStatus.has(listener)) {
          subscriberInitStatus.set(listener, true);
          listener(pageData, null, true);
        } else {
          listener(pageData, callbackId, false);
        }
      } catch (e) {
        warn('Data subscriber error:', e);
      }
    });

    if (callbackId) {
      sendCallback(callbackId);
    }
  } else if (name === 'samelevel') {
    handleSameLevelEvent(payload);
  } else {
    warn('Unknown event:', name);
  }
}

function handleIncomingMessage(message: BridgeMessage): void {
  if (isDebugEnabled('proto')) {
    console.log('←', JSON.stringify(message, null, 2));
  }
  if (!message || typeof message !== 'object' || !message.type) {
    warn('Invalid message format:', message);
    return;
  }

  switch (message.type) {
    case 'reply':
      handleReply(message);
      break;
    case 'event':
      handleEvent(message);
      break;
    default:
      warn('Unknown message type:', message.type);
  }
}

export const LingXiaBridge: LingXiaBridgeInterface = {
  call(name: string, payload: unknown = null): Promise<unknown> {
    return new Promise((resolve, reject) => {
      const msgId = `view-${Date.now()}-${messageCounter++}`;
      const timerId = setTimeout(() => {
        if (pendingCalls.has(msgId)) {
          pendingCalls.get(msgId)!.reject({ message: `Call '${name}' timed out` });
          pendingCalls.delete(msgId);
        }
      }, CALL_TIMEOUT_MS);

      pendingCalls.set(msgId, { resolve, reject, timerId });
      sendMessageToNative({
        msgId: msgId,
        type: 'call',
        name: name,
        payload: payload,
      });
    });
  },

  event(name: string, payload: unknown = null): void {
    sendMessageToNative({
      msgId: null,
      type: 'event',
      name: name,
      payload: payload,
    });
  },

  subscribe(callback: DataSubscriber): () => void {
    if (typeof callback !== 'function') {
      error('Subscriber must be a function');
      return () => {};
    }

    dataSubscribers.add(callback);

    if (Object.keys(pageData).length > 0) {
      if (dataSubscribers.has(callback)) {
        subscriberInitStatus.set(callback, true);
        try {
          callback(deepCopy(pageData), null, true);
        } catch (e) {
          error('Initial data callback error:', e);
        }
      }
    }

    return () => {
      dataSubscribers.delete(callback);
      subscriberInitStatus.delete(callback);
    };
  },

  _connectWebMessagePort(port: MessagePort): void {
    if (communicationMethod !== MESSAGE_PORT_TYPE) return;

    log('Connecting WebMessage port...');
    messagePort = port;

    port.onmessage = (event: MessageEvent) => {
      let messageData = event.data;
      if (typeof messageData === 'string') {
        try {
          messageData = JSON.parse(messageData);
        } catch (e) {
          error('Invalid JSON from MessagePort:', e);
          return;
        }
      }
      handleIncomingMessage(messageData);
    };

    log('MessagePort connected and ready');
    this.event('LXPortRdy');
  },

  _receiveEvaluateMessage(messageString: string): void {
    try {
      if (!messageString) return;
      const message = JSON.parse(messageString);
      handleIncomingMessage(message);
    } catch (e) {
      error('Invalid JSON from evaluate_javascript:', e);
    }
  },

  debug: new Proxy(debugFlags, {
    get(target, prop: keyof typeof debugFlags) {
      return target[prop];
    },
    set(target, prop: keyof typeof debugFlags, value: boolean) {
      if (prop in target) {
        target[prop] = !!value;
        console.log(
          `[LingXia Debug] ${prop} debugging ${value ? 'enabled' : 'disabled'}`
        );
        return true;
      }
      return false;
    },
  }),

  platform: {
    isHarmony,
    isIOS,
    isAndroid,
    getOS: getPlatformOS,
  },

  dom: {
    measureById(id: string): [number, number, number, number, number] | null {
      try {
        if (!id || typeof id !== 'string') return null;
        const el = document.getElementById(id);
        if (!el) return null;
        const r = el.getBoundingClientRect();

        let cornerRadius = 0;
        try {
          const radiusStr = getComputedStyle(el).borderRadius;
          const parsed = parseFloat(radiusStr);
          if (!Number.isNaN(parsed)) cornerRadius = parsed;
        } catch (_e) {
        }

        return [
          r.left + window.scrollX,
          r.top + window.scrollY,
          r.width,
          r.height,
          cornerRadius,
        ];
      } catch (_e) {
        return null;
      }
    },
  },

  sameLevel: {
    send: sendSameLevelMessage,
    hasHandler: hasSameLevelHandler,
    flush: flushSameLevelQueue,
    register(
      id: string,
      handler: (message: SameLevelMessage) => void
    ): () => void {
      if (!id || typeof handler !== 'function') return () => {};
      sameLevelHandlers.set(id, handler);
      return () => {
        sameLevelHandlers.delete(id);
      };
    },
    unregister(id: string): void {
      sameLevelHandlers.delete(id);
    },
  },
};

export const lx: Record<string, (...args: unknown[]) => Promise<unknown>> =
  new Proxy(
    {},
    {
      get(_target, prop: string) {
        return function (...args: unknown[]): Promise<unknown> {
          let payload: unknown = null;
          if (
            args.length === 1 &&
            typeof args[0] === 'object' &&
            args[0] !== null
          ) {
            payload = args[0];
          } else if (args.length > 1) {
            warn(
              `lx.${prop} called with multiple arguments, only the first object argument will be used`
            );
            if (typeof args[0] === 'object' && args[0] !== null) {
              payload = args[0];
            }
          }

          return LingXiaBridge.call(`lx.${prop}`, payload);
        };
      },
    }
  );

export function initBridge(): void {
  log(`Detected communication method: ${communicationMethod}`);

  window[GLOBAL_RECEIVER_NAME] = LingXiaBridge._receiveEvaluateMessage;

  if (communicationMethod === 'webkit') {
    LingXiaBridge.event('LXPortRdy');
  } else if (communicationMethod === MESSAGE_PORT_TYPE) {
    getMessagePort().catch((e) => {
      warn('Failed to initialize MessagePort:', e);
    });
  } else {
    warn('Unknown communication method, bridge may not work properly');
  }

  window.LingXiaBridge = LingXiaBridge;
  window.lx = lx;

  log('LingXia Bridge initialization completed');
}
