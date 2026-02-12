export interface BridgeConfig {
  os?: 'Harmony' | 'iOS' | 'Android' | 'macOS';
  nonce?: string;
}

export interface RuntimeConfig {
  error?: ErrorInfo;
}

export interface ErrorInfo {
  failedPath?: string;
  reason?: string;
  code?: number;
}

// V2 Error Codes (stable)
export const BRIDGE_ERROR = {
  NOT_READY: 'BRIDGE_NOT_READY',
  TIMEOUT: 'BRIDGE_TIMEOUT',
  CANCELED: 'BRIDGE_CANCELED',
  PROTOCOL_MISMATCH: 'BRIDGE_PROTOCOL_MISMATCH',
  HANDSHAKE_FAILED: 'BRIDGE_HANDSHAKE_FAILED',
  MALFORMED_MESSAGE: 'BRIDGE_MALFORMED_MESSAGE',
  METHOD_NOT_FOUND: 'BRIDGE_METHOD_NOT_FOUND',
  CAPABILITY_DENIED: 'BRIDGE_CAPABILITY_DENIED',
  INTERNAL_ERROR: 'BRIDGE_INTERNAL_ERROR',
  OUTBOX_FULL: 'BRIDGE_OUTBOX_FULL',
} as const;

export type BridgeErrorCode = (typeof BRIDGE_ERROR)[keyof typeof BRIDGE_ERROR];

export interface LxBridgeError {
  code: string;
  message?: string;
  data?: unknown;
  retryable?: boolean;
}

export interface StateInfo {
  rev: number;
  initial: boolean;
}

export type DataSubscriber = (data: Record<string, unknown>, info: StateInfo) => void;

// Type-safe method map (augment via declaration merging)
export interface LingXiaBridgeMethodMap {}

export type LxMethodParams<M extends keyof LingXiaBridgeMethodMap> =
  LingXiaBridgeMethodMap[M] extends { params: infer P } ? P :
  LingXiaBridgeMethodMap[M] extends { params?: infer P } ? P | undefined : unknown;

export type LxMethodResult<M extends keyof LingXiaBridgeMethodMap> =
  LingXiaBridgeMethodMap[M] extends { result: infer R } ? R : unknown;

export type LxMethod = keyof LingXiaBridgeMethodMap & string;

export interface NativeComponentMessage {
  id: string;
  action?: string;
  [key: string]: unknown;
}

export interface CallOptions {
  cap?: string;
  timeoutMs?: number;
  signal?: AbortSignal;
}

export interface NotifyOptions {
  cap?: string;
}

declare global {
  function useLingXia<
    TData = Record<string, unknown>,
    TActions = Record<string, (...args: unknown[]) => unknown>
  >(): { data: TData } & TActions;

  interface Window {
    __LX_BRIDGE_CFG?: BridgeConfig;
    __LX_RUNTIME_CONFIG?: RuntimeConfig;
    __LingXiaRecvMessage?: (message: string) => void;
    LingXiaBridge?: LingXiaBridgeInterface;
    useLingXia?: typeof useLingXia;
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
  call(method: string, params?: unknown, options?: CallOptions): Promise<unknown>;
  call<M extends LxMethod>(method: M, params?: LxMethodParams<M>, options?: CallOptions): Promise<LxMethodResult<M>>;
  notify(method: string, params?: unknown, options?: NotifyOptions): void;
  subscribe(callback: DataSubscriber): () => void;
  _connectWebMessagePort(port: MessagePort): void;
  _receiveEvaluateMessage(messageString: string): void;
  debug: { data: boolean; proto: boolean; all: boolean };
  platform: {
    isHarmony(): boolean;
    isIOS(): boolean;
    isAndroid(): boolean;
    isMacOS(): boolean;
    isDesktop(): boolean;
    getOS(): string;
  };
  dom: {
    measureById(id: string): [number, number, number, number, number] | null;
  };
  nativeComponents: {
    send(message: NativeComponentMessage): void;
    hasHandler(): boolean;
    flush(): void;
    register(id: string, handler: (message: NativeComponentMessage) => void): () => void;
    unregister(id: string): void;
  };
  isReady(): boolean;
}
