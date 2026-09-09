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
}

/** Lifecycle hook names a `Page({...})` config may declare. */
export type PageLifecycleName = Exclude<keyof PageConfig, 'data'>;

/** Lifecycle hook names an `App({...})` config may declare. */
export type AppLifecycleName = Exclude<keyof AppConfig, 'globalData'>;

/**
 * What a key that differs from a lifecycle hook only in case resolves to, so
 * the compiler names the mistake instead of silently accepting a new method.
 */
export type MisspelledLifecycle<K extends string> = {
  'LingXia type error': `'${K}' differs only in case from a lifecycle hook`;
};

/**
 * Applied to the custom half of a `Page`/`App` config. `onload` and `onShow`
 * are one keystroke apart from real hooks and the runtime would simply never
 * call the misspelling, so the closest case-insensitive match is rejected.
 * Genuinely different names stay ordinary methods.
 */
export type NoLifecycleTypos<TCustom, TNames extends string> = {
  [K in keyof TCustom]: K extends TNames
    ? TCustom[K]
    : Lowercase<K & string> extends Lowercase<TNames>
      ? MisspelledLifecycle<K & string>
      : TCustom[K];
};

/**
 * A `setData` key that addresses inside `data` — `'a.b'` or `'rows[0].name'`.
 * Values behind a path stay `unknown`: the runtime resolves the path, so the
 * type cannot.
 */
export type PageDataPath = `${string}.${string}` | `${string}[${number}]${string}`;

/**
 * A field initialized to `null` or `[]` states nothing about what will fill it
 * later, so it stays open. Annotate it (`null as Profile | null`) to have the
 * fill checked.
 */
export type LazyInitField<T> = [T] extends [null | undefined]
  ? unknown
  : [T] extends [never[]]
    ? unknown[]
    : T;

/**
 * Top-level keys are checked against `data`; only path-shaped keys stay open.
 * A misspelled or wrongly typed top-level key is a compile error.
 */
export type SetDataPatch<TData> = { [K in keyof TData]?: LazyInitField<TData[K]> } &
  Partial<Record<PageDataPath, unknown>>;

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
  setData(data: SetDataPatch<TData>, callback?: () => void): void;
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

    /** The language this lxapp renders in. Every lxapp follows it. */
    readonly displayLanguage: DisplayLanguageApi;

    /** The light/dark scheme this lxapp renders in. */
    readonly appearance: AppearanceApi;

    /**
     * Product-wide settings, and their single writer. Present only in the
     * Control app the host sealed at build time; its presence and
     * `lx.supports({ capability: 'control' })` always agree, so
     * `lx.app.control?.…` and the query are interchangeable.
     */
    readonly control?: ControlApi;

    /**
     * Product-wide cache reporting and clearing for a settings screen.
     * Present only in the Control app; its presence and
     * `lx.supports({ capability: 'control' })` always agree, so
     * `lx.app.cache?.…` and the query are interchangeable.
     */
    cache?: AppCacheApi;
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

/**
 * A map of storage keys to stored value shapes.
 *
 * `object` deliberately accepts both type aliases and interfaces. Requiring a
 * string index signature would reject ordinary interface-based schemas.
 */
export type StorageSchema = object;

type StorageKey<S extends object> = Extract<keyof S, string>;
type StorageEntry<S extends object> = {
  [K in StorageKey<S>]: [key: K, value: S[K]];
}[StorageKey<S>];

/**
 * Schema-typed view of the same store `lx.getStorage()` returns.
 * Runtime is identical; only the key/value types are pinned.
 *
 * The schema constrains what this handle writes and reads, not what the store
 * contains: a previous app version, or another code path holding the untyped
 * handle, can have written keys outside it. That is why `list` still resolves
 * plain strings — narrowing it to the schema's keys would be the same
 * unchecked assertion this type exists to remove from `get<T>()`.
 */
export type TypedStorage<S extends object> = {
  get<K extends StorageKey<S>>(key: K): Promise<S[K] | undefined>;
  set(...entry: StorageEntry<S>): Promise<void>;
  delete(key: StorageKey<S>): Promise<void>;
  clear(): Promise<void>;
  list(prefix?: string): Promise<string[]>;
  info(): Promise<StorageInfo>;
};
