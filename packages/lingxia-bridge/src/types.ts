export interface BridgeConfig {
  os?: 'Harmony' | 'iOS' | 'Android' | 'macOS';
  nonce?: string;
  appleDownstreamURL?: string;
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
  TOPIC_NOT_FOUND: 'BRIDGE_TOPIC_NOT_FOUND',
  CAPABILITY_DENIED: 'BRIDGE_CAPABILITY_DENIED',
  INTERNAL_ERROR: 'BRIDGE_INTERNAL_ERROR',
  OUTBOX_FULL: 'BRIDGE_OUTBOX_FULL',
  STREAM_OVERFLOW: 'BRIDGE_STREAM_OVERFLOW',
  STREAM_CLOSED: 'BRIDGE_STREAM_CLOSED',
} as const;

export type BridgeErrorCode = (typeof BRIDGE_ERROR)[keyof typeof BRIDGE_ERROR];

export interface LxBridgeError {
  code: string | number;
  message?: string;
  data?: unknown;
}

export interface NativeError {
  code: string;
  message: string;
  data?: unknown;
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

export type LxMethodStreamData<M extends keyof LingXiaBridgeMethodMap> =
  LingXiaBridgeMethodMap[M] extends { stream: infer S } ? S : unknown;

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

export interface StreamCallOptions extends CallOptions {}

export interface ChannelOpenOptions {
  cap?: string;
}

export type StreamEventName = 'data' | 'end' | 'error';
export type ChannelEventName = 'data' | 'close' | 'error';

export interface LxStream<TData = unknown, TResult = unknown> extends AsyncIterable<TData> {
  on(event: 'data', listener: (payload: TData) => void): this;
  on(event: 'end', listener: (result: TResult) => void): this;
  on(event: 'error', listener: (error: LxBridgeError) => void): this;
  cancel(): void;
  [Symbol.asyncIterator](): AsyncIterator<TData, TResult, void>;
  readonly id: string;
  readonly result: Promise<TResult>;
}

export interface NativeStream<TEvent = unknown, TResult = void> {
  onEvent(listener: (event: TEvent) => void): () => void;
  onError(listener: (error: NativeError) => void): () => void;
  result: Promise<TResult>;
  cancel(): void;
}

export interface LxChannel<TIn = unknown, TOut = TIn> extends AsyncIterable<TIn> {
  send(payload: TOut): void;
  on(event: 'data', listener: (payload: TIn) => void): this;
  on(event: 'close', listener: (code?: string, reason?: string) => void): this;
  on(event: 'error', listener: (error: LxBridgeError) => void): this;
  close(code?: string, reason?: string): void;
  [Symbol.asyncIterator](): AsyncIterator<TIn, void, void>;
  readonly id: string;
}

export interface ChannelCloseEvent {
  code?: string;
  reason?: string;
}

export interface NativeChannel<TIn = unknown, TOut = unknown> {
  send(message: TIn): void;
  onMessage(listener: (message: TOut) => void): () => void;
  onClose(listener: (event: ChannelCloseEvent) => void): () => void;
  close(code?: string, reason?: string): void;
}

declare global {
  interface Window {
    __LX_BRIDGE_CFG?: BridgeConfig;
    __LX_BRIDGE_INIT_STATE?: 'initializing' | 'initialized';
    __LX_RUNTIME_CONFIG?: RuntimeConfig;
    __pageBridge?: { __names: string[]; [key: string]: unknown };
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
  invoke<TResult = unknown, TInput = void>(route: string, input?: TInput, options?: InvokeOptions): Promise<TResult>;
  stream<TEvent = unknown, TResult = void, TInput = void>(route: string, input?: TInput, options?: StreamOptions): NativeStream<TEvent, TResult>;
  notify<TInput = void>(route: string, input?: TInput, options?: NotifyOptions): void;
  channel<TIn = unknown, TOut = unknown>(route: string, input?: unknown, options?: ChannelOptions): Promise<NativeChannel<TIn, TOut>>;
  raw: LingXiaBridgeRawInterface;
  state: {
    subscribe(callback: DataSubscriber): () => void;
  };
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

export interface LingXiaBridgeRawInterface {
  call(method: string, params?: unknown, options?: CallOptions): Promise<unknown>;
  call<M extends LxMethod>(method: M, params?: LxMethodParams<M>, options?: CallOptions): Promise<LxMethodResult<M>>;
  stream(method: string, params?: unknown, options?: StreamCallOptions): LxStream<unknown, unknown>;
  stream<M extends LxMethod>(method: M, params?: LxMethodParams<M>, options?: StreamCallOptions): LxStream<LxMethodStreamData<M>, LxMethodResult<M>>;
  notify(method: string, params?: unknown, options?: NotifyOptions): void;
  channel: {
    open<TIn = unknown, TOut = TIn>(topic: string, params?: unknown, options?: ChannelOpenOptions): Promise<LxChannel<TIn, TOut>>;
  };
}

export interface InvokeOptions extends CallOptions {}
export interface StreamOptions extends StreamCallOptions {}
export interface ChannelOptions extends ChannelOpenOptions {}
