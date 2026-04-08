/**
 * App & Page APIs
 * Corresponds to: lingxia-lxapp/src/appservice/
 */

export interface AppLifecycleEventArgs {
  source: 'host' | 'lxapp';
  reason:
    | 'foreground'
    | 'background'
    | 'screenshot'
    | 'open'
    | 'close'
    | 'switch_back'
    | 'switch_away';
}

export type LxAppReleaseType = 'release' | 'preview' | 'developer';

export interface LxAppInfo {
  appId: string;
  appName: string;
  version: string;
  releaseType: LxAppReleaseType;
}

export interface AppLaunchOptions {
  path?: string;
  query?: Record<string, string>;
  scene?: number;
  referrerInfo?: {
    appId?: string;
    extraData?: Record<string, unknown>;
  };
}

export interface AppConfig {
  globalData?: Record<string, unknown>;
  onLaunch?: (options?: AppLaunchOptions) => void | Promise<void>;
  onShow?: (args?: AppLifecycleEventArgs) => void | Promise<void>;
  onHide?: (args?: AppLifecycleEventArgs) => void | Promise<void>;
  onUserCaptureScreen?: () => void | Promise<void>;
  [key: string]: unknown;
}

export interface AppInstance extends AppConfig {
  globalData: Record<string, unknown>;
}

export interface PageLoadOptions {
  [key: string]: string | undefined;
}

export interface PageConfig<TData extends Record<string, unknown> = Record<string, unknown>> {
  data?: TData;
  onLoad?: (options?: PageLoadOptions) => void | Promise<void>;
  onShow?: () => void | Promise<void>;
  onReady?: () => void | Promise<void>;
  onHide?: () => void | Promise<void>;
  onUnload?: () => void | Promise<void>;
  onPullDownRefresh?: () => void | Promise<void>;
  [key: string]: unknown;
}

export interface PageInstance<TData extends Record<string, unknown> = Record<string, unknown>> {
  data: TData;
  route: string;
  setData(data: Partial<TData> | Record<string, unknown>, callback?: () => void): void;
  getEventEmitter(): EventEmitter;
}

export interface EventEmitter {
  on(event: string, handler: (...args: unknown[]) => void): void;
  off(event: string, handler: (...args: unknown[]) => void): void;
  emit(event: string, ...args: unknown[]): void;
  once(event: string, handler: (...args: unknown[]) => void): void;
}

/**
 * Injected by the runtime into methods listed in `stream_handlers` page metadata.
 *
 * Use this when your async source uses callbacks rather than an async iterator.
 * For the generator form (`async *method()`), no handle is needed — the runtime
 * pumps the generator automatically.
 */
export interface StreamHandle<T = unknown> {
  /** Send a chunk to View. */
  send(payload: T): void;
  /** End the stream with an optional final value. */
  end(result?: unknown): void;
  /** End the stream with an error. */
  error(code: string, message?: string): void;
}

/**
 * Injected by the runtime as the second parameter when View opens a channel.
 *
 * Use `ch.send()` to push data to View, `ch.on()` to receive data/close
 * events from View, and `ch.close()` to shut down the channel.
 */
export interface ChannelHandle<TSend = unknown, TReceive = unknown> {
  /** Push a message to View. */
  send(payload: TSend): void;
  /** Close the channel from Logic side. */
  close(code?: string, reason?: string): void;
  /** Register a listener for incoming events. */
  on(event: 'data', handler: (payload: TReceive) => void): void;
  on(event: 'close', handler: (info: { code: string; reason: string }) => void): void;
}
