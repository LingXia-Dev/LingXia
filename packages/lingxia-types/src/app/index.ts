/**
 * Host app, app lifecycle, and page instance APIs.
 */

export interface AppBaseInfo {
  language: string;
  productName: string;
  version: string;
  SDKVersion: string;
}

export interface HostAppUpdateInfo {
  version: string;
  size?: number;
  releaseNotes?: string[];
  isForceUpdate: boolean;
  /**
   * Download and apply this checked update.
   *
   * `apply()` is single-use for this update object.
   *
   * The returned task can be awaited directly when progress is not needed, or
   * consumed with `for await...of` to render progress.
   *
   * Direct package handoff is currently supported on Android and macOS. Other
   * platforms reject with an unsupported-operation error; use `version` and
   * `releaseNotes` to guide users to the appropriate app marketplace.
   */
  apply(): HostAppUpdateTask;
}

export type HostAppUpdateApplyStage = 'download' | 'install';

export type HostAppUpdateCheckResult =
  | {
      hasUpdate: false;
      update?: never;
    }
  | {
      hasUpdate: true;
      update: HostAppUpdateInfo;
    };

export type HostAppUpdateEvent =
  | {
      state: 'downloading';
      downloadedBytes?: number;
      progress?: number;
    }
  | {
      state: 'downloaded' | 'installRequested';
    }
  | {
      state: 'failed';
      stage: HostAppUpdateApplyStage;
      error: string;
    };

export interface HostAppUpdateResult {
  state: 'installRequested';
}

export interface HostAppUpdateIteratorResult {
  done: boolean;
  value?: HostAppUpdateEvent;
}

export interface HostAppUpdateTask
  extends PromiseLike<HostAppUpdateResult>,
    AsyncIterable<HostAppUpdateEvent> {
  next(): Promise<HostAppUpdateIteratorResult>;
  /** Stops iteration only. It does not cancel an app update already handed to the platform. */
  return(): Promise<HostAppUpdateIteratorResult>;
  catch<TResult = never>(
    onrejected?: ((reason: unknown) => TResult | PromiseLike<TResult>) | null,
  ): Promise<HostAppUpdateResult | TResult>;
  finally(onfinally?: (() => void) | null): Promise<HostAppUpdateResult>;
  wait(): Promise<HostAppUpdateResult>;
}

export interface HostAppApi {
  getBaseInfo(): AppBaseInfo;

  /**
   * Check whether the host app has an update.
   *
   * This is a host app level capability and is only available to the home
   * lxapp. Non-home lxapps receive a permission error.
   *
   * Calling this API opts the host app into custom update handling for this
   * process. LingXia will not show its built-in host app update UI or
   * auto-install host app updates after that point.
   *
   * If no update is available, or if an update requires a newer LingXia
   * runtime than the current host app provides, `hasUpdate` is `false`.
   *
   * `checkUpdate()` may return version metadata on platforms that cannot apply
   * host app packages directly. In that case, `update.apply()` rejects with an
   * unsupported-operation error.
   */
  checkUpdate(): Promise<HostAppUpdateCheckResult>;

  /**
   * Exit the host app immediately.
   *
   * This API does not show a confirmation dialog. If the user should confirm
   * first, call `lx.showModal(...)` in page logic and invoke `lx.app.exit()`
   * only after the user confirms.
   */
  exit(): void;
}

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
