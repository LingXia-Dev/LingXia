// Rong 0.5 cannot yet express generic TypeScript-only aliases or correlated
// overloads. Keep only those irreducible declarations in this generated prelude.
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
   * Available when this page was opened as a surface via `lx.openSurface(...)`.
   */
  surface?: Surface;
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

export interface FileManager {
  readFile(options: ReadTextFileOptions): Promise<ReadTextFileResult>;
  readFile(options: ReadBinaryFileOptions): Promise<ReadBinaryFileResult>;
  readFile(options: ReadFileOptions): Promise<ReadFileResult>;
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
     * Launch-at-startup control. Present only on macOS / Windows with the
     * capability declared; gate access with `lx.app.autostart?.…`.
     */
    autostart?: AutostartApi;
  }

  /** Runtime environment constants backed by abstract `lx://` paths. */
  interface LxEnv {}

  interface Lx {
    /**
     * Open a surface. Browser tabs resolve to `null`, declared surfaces to a
     * host-managed handle, and page/aside surfaces to a full `Surface`.
     * `as: "window"` is desktop-only.
     */
    openSurface(spec: OpenUrlTabSpec): Promise<null>;
    openSurface(spec: OpenLxappSurfaceSpec | OpenNativeSurfaceSpec): Promise<SurfaceHandle>;
    openSurface(spec: OpenPageSurfaceSpec | OpenUrlAsideSpec): Promise<Surface>;
    openSurface(spec: OpenSurfaceSpec): Promise<Surface | SurfaceHandle | null>;

    /** Download to the downloads directory. */
    downloadFile(options: DownloadsDownloadOptions): DownloadTask<DownloadsDownloadResult>;
    /** Download to the lxapp-managed app directory. */
    downloadFile(options: AppDownloadOptions): DownloadTask<AppDownloadResult>;
    /** Download with a destination-correlated result type. */
    downloadFile<TDestination extends DownloadDestination = "app">(
      options: DownloadOptions<TDestination>,
    ): DownloadTask<DownloadResultForDestination<TDestination>>;
  }
}
