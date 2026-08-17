// Keep generic TypeScript-only aliases, correlated overloads, and types owned
// by external Rong modules in this generated prelude.
declare const appDownloadPathBrand: unique symbol;
declare const systemDownloadsPathBrand: unique symbol;

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
  /**
   * Available when this page was opened as a surface via
   * `lx.surface.openPage(...)`.
   */
  surface?: PageSurface;
  /**
   * Available when this page was opened by `lx.navigateTo(...)`.
   */
  opener?: PageMessagePort;
  setData(data: Partial<TData> | Record<string, unknown>, callback?: () => void): void;
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

/**
 * Download options.
 *
 * - `app`: app-owned temporary output, or durable `lx://userdata` output when
 *   `filePath` is set
 * - `downloads`: user-visible system Downloads output, requiring
 *   `security.privileges: ["downloads"]` in `lxapp.json`
 *
 * Default: `app`.
 */
export type DownloadOptions<TDestination extends DownloadDestination = DownloadDestination> =
  TDestination extends 'downloads' ? DownloadsDownloadOptions : AppDownloadOptions;

export type DownloadResultForDestination<TDestination extends DownloadDestination> =
  TDestination extends 'downloads' ? DownloadsDownloadResult : AppDownloadResult;

export interface DownloadProgressEvent<TResult extends DownloadResult = DownloadResult> {
  kind: 'progress' | 'paused' | 'resumed' | 'canceled' | 'completed';
  downloadedBytes?: number;
  totalBytes?: number;
  /** Present only when the total size is known. */
  progress?: number;
  result?: TResult;
}

export interface DownloadIteratorResult<TResult extends DownloadResult = DownloadResult> {
  done: boolean;
  value?: DownloadProgressEvent<TResult>;
}

export interface DownloadTask<TDownloadResult extends DownloadResult = DownloadResult>
  extends PromiseLike<TDownloadResult>,
    AsyncIterable<DownloadProgressEvent<TDownloadResult>> {
  next(): Promise<DownloadIteratorResult<TDownloadResult>>;
  /** Stops iteration only. Does not cancel the underlying download task. */
  return(): Promise<DownloadIteratorResult<TDownloadResult>>;
  catch<TRejected = never>(
    onrejected?: ((reason: unknown) => TRejected | PromiseLike<TRejected>) | null,
  ): Promise<TDownloadResult | TRejected>;
  finally(onfinally?: (() => void) | null): Promise<TDownloadResult>;
  pause(): Promise<void>;
  resume(): Promise<void>;
  cancel(): Promise<void>;
  /** Alias for cancel(), matching browser/mini-program abort naming. */
  abort(): Promise<void>;
  wait(): Promise<TDownloadResult>;
}

declare global {
  // HostAppApi/LxEnv members are emitted from the Rust js_api metadata; these
  // merges only add what Rong cannot express — the cfg-gated autostart member
  // and doc comments (js_api consts cannot carry docs). envVersion re-declares
  // the generated member doc-only; tsc rejects the merge if the types drift.
  interface HostAppApi {
    /**
     * The build environment from `app.json::envVersion`. It is fixed at boot
     * and defaults to `release` for older artifacts.
     */
    readonly envVersion: HostAppEnvVersion;

    /**
     * Launch-at-startup control. Absent where the host cannot register a
     * startup item; its presence and `lx.supports({ capability: 'autostart' })` always
     * agree, so `lx.app.autostart?.…` and the query are interchangeable.
     */
    autostart?: AutostartApi;
  }

  /** Runtime environment constants backed by abstract `lx://` paths. */
  interface LxEnv {}

  interface Lx {
    /**
     * Terminal product settings. Present only in the host-bundled Terminal
     * Settings lxapp when the host declares `capabilities.terminal`; its
     * presence and `lx.supports({ capability: 'terminal' })` always agree.
     */
    readonly terminal?: TerminalApi;

    /** Download to the downloads directory. */
    downloadFile(options: DownloadsDownloadOptions): DownloadTask<DownloadsDownloadResult>;
    /** Download to the lxapp-managed app directory. */
    downloadFile(options: AppDownloadOptions): DownloadTask<AppDownloadResult>;
    /** Download with a destination-correlated result type. */
    downloadFile<TDestination extends DownloadDestination = "app">(
      options: DownloadOptions<TDestination>,
    ): DownloadTask<DownloadResultForDestination<TDestination>>;

    /**
     * Open this lxapp's store with every key's shape pinned on the handle.
     * `get` / `set` / `delete` then share that schema instead of
     * repeating `get<T>()` at each call site.
     */
    getStorage<S extends StorageSchema>(): TypedStorage<S>;
  }
}

/** A map of storage keys to stored value shapes. */
export type StorageSchema = Record<string, unknown>;

/**
 * Schema-typed view of the same store `lx.getStorage()` returns.
 * Runtime is identical; only the key/value types are pinned.
 */
export type TypedStorage<S extends object> = {
  get<K extends Extract<keyof S, string>>(key: K): Promise<S[K] | undefined>;
  set<K extends Extract<keyof S, string>>(key: K, value: S[K]): Promise<void>;
  delete(key: Extract<keyof S, string>): Promise<void>;
  clear(): Promise<void>;
  list(prefix?: string): Promise<Array<Extract<keyof S, string>>>;
  info(): Promise<StorageInfo>;
};
