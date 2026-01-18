export interface BridgeConfig {
  os?: 'Harmony' | 'iOS' | 'Android' | 'macOS';
}

export interface RuntimeConfig {
  error?: ErrorInfo;
}

export interface ErrorInfo {
  failedPath?: string;
  reason?: string;
  code?: number;
}

export interface BridgeMessage {
  msgId: string | null;
  type: 'call' | 'reply' | 'event' | 'callback';
  name?: string;
  payload?: unknown;
  callbackId?: string;
}

export interface ReplyPayload {
  success: boolean;
  result?: unknown;
  error?: { message: string };
}

export interface PendingCall {
  resolve: (value?: unknown) => void;
  reject: (reason?: unknown) => void;
  timerId: ReturnType<typeof setTimeout>;
}

export type DataSubscriber = (
  data: Record<string, unknown>,
  callbackId: string | null,
  isInitialData: boolean
) => void;

export interface NativeComponentMessage {
  id: string;
  action?: string;
  [key: string]: unknown;
}

declare global {
  interface Window {
    __LX_BRIDGE_CFG?: BridgeConfig;
    __LX_RUNTIME_CONFIG?: RuntimeConfig;
    __LingXiaRecvMessage?: (message: string) => void;
    LingXiaBridge?: LingXiaBridgeInterface;
    LingXiaProxy?: {
      supportsMessagePort: () => boolean;
      getPort: (name: string) => string;
      postMessage: (message: string) => void;
    };
    NativeComponentBridge?: {
      postMessage: (message: string) => void;
    };
    host?: Record<string, (...args: unknown[]) => Promise<unknown>>;
    webkit?: {
      messageHandlers: {
        [key: string]: {
          postMessage: (message: unknown) => void;
        };
      };
    };
  }
}

export interface LingXiaBridgeInterface {
  call: (name: string, payload?: unknown) => Promise<unknown>;
  event: (name: string, payload?: unknown) => void;
  subscribe: (callback: DataSubscriber) => () => void;
  _connectWebMessagePort: (port: MessagePort) => void;
  _receiveEvaluateMessage: (messageString: string) => void;
  debug: {
    data: boolean;
    proto: boolean;
    all: boolean;
  };
  platform: {
    isHarmony: () => boolean;
    isIOS: () => boolean;
    isAndroid: () => boolean;
    getOS: () => string;
  };
  dom: {
    measureById: (id: string) => [number, number, number, number, number] | null;
  };
  nativeComponents: {
    send: (message: NativeComponentMessage) => void;
    hasHandler: () => boolean;
    flush: () => void;
    register: (id: string, handler: (message: NativeComponentMessage) => void) => () => void;
    unregister: (id: string) => void;
  };
}
