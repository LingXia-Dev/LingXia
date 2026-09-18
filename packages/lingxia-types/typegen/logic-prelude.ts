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
 * Kept for documentation; nested writes go through `setPath` (checked) or
 * `setDataPath` (unchecked). `setData` no longer accepts these keys.
 */
export type PageDataPath = `${string}.${string}` | `${string}[${number}]${string}`;

/**
 * @deprecated Page `data` is no longer widened for `null` / `[]` initializers.
 * Annotate the field (`null as Profile | null`) when a later fill should be
 * checked.
 */
export type LazyInitField<T> = T;

type PageReadonlyDepth = [never, 0, 1, 2, 3, 4, 5, 6];

type PagePrimitive = string | number | boolean | bigint | symbol | null | undefined;

/**
 * Deep readonly view of page `data`. Static only — the runtime object is not
 * frozen.
 */
export type DeepReadonly<T, D extends number = 6> = [D] extends [never]
  ? T
  : T extends PagePrimitive
    ? T
    : T extends Function
      ? T
      : T extends readonly (infer U)[]
        ? ReadonlyArray<DeepReadonly<U, PageReadonlyDepth[D]>>
        : { readonly [K in keyof T]: DeepReadonly<T[K], PageReadonlyDepth[D]> };

/**
 * Tuple path into `data`, depth-capped so large page states stay completable.
 */
export type DataPath<T, D extends number = 5> = [D] extends [never]
  ? never
  : T extends readonly (infer U)[]
    ? [number] | [number, ...DataPath<U, PageReadonlyDepth[D]>]
    : T extends object
      ? {
          [K in keyof T & (string | number)]:
            | [K]
            | (DataPath<T[K], PageReadonlyDepth[D]> extends infer Rest
                ? Rest extends readonly PropertyKey[]
                  ? [K, ...Rest]
                  : never
                : never);
        }[keyof T & (string | number)]
      : never;

export type DataPathValue<T, P extends readonly PropertyKey[]> = P extends readonly [
  infer K,
  ...infer Rest,
]
  ? Rest extends readonly PropertyKey[]
    ? Rest['length'] extends 0
      ? K extends keyof T
        ? T[K]
        : K extends number
          ? T extends readonly (infer U)[]
            ? U
            : never
          : never
      : K extends keyof T
        ? DataPathValue<T[K], Rest>
        : K extends number
          ? T extends readonly (infer U)[]
            ? DataPathValue<U, Rest>
            : never
          : never
    : never
  : never;

/**
 * Top-level keys are checked against `data`. Nested writes use `setPath` or
 * the unchecked `setDataPath`.
 */
export type SetDataPatch<TData> = { [K in keyof TData]?: TData[K] };

type SetDataValue<TData, K> = K extends PageDataPath
  ? { "LingXia type error": "use setPath or setDataPath for nested writes" }
  : K extends keyof TData
    ? TData[K]
    : { "LingXia type error": "unknown data key" };

export interface PageInstance<TData extends Record<string, unknown> = Record<string, unknown>> {
  readonly data: { readonly [K in keyof TData]: DeepReadonly<TData[K]> };
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
  setData<TPatch extends Record<string, unknown>>(
    data: TPatch & { [K in keyof TPatch]: SetDataValue<TData, K> },
    callback?: () => void,
  ): void;
  setPath<const P extends DataPath<TData>>(
    path: P,
    value: DataPathValue<TData, P>,
    callback?: () => void,
  ): void;
  /** Nested write by a runtime-resolved string path. The value is not checked. */
  setDataPath(path: string, value: unknown, callback?: () => void): void;
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
  /**
   * Called once if View cancels before `end`/`error`. Returns unsubscribe.
   * The generator form still observes cancel in `finally`; use this for the
   * callback-based handle.
   */
  onCancel(handler: () => void): () => void;
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
  /** Register a listener for incoming events. Returns unsubscribe. */
  on(event: 'data', handler: (payload: TReceive) => void): () => void;
  on(event: 'close', handler: (info: { code: string; reason: string }) => void): () => void;
}

/**
 * Download options.
 *
 * - `app`: app-owned temporary output, or durable `lx://userdata` output when
 *   `filePath` is set
 * - `downloads`: user-visible system Downloads output, requiring a host
 *   privilege grant and a native Downloads grant
 *
 * Default: `app`.
 */
export type DownloadOptions<TDestination extends DownloadDestination = DownloadDestination> =
  TDestination extends 'downloads' ? DownloadsDownloadOptions : AppDownloadOptions;

export type DownloadResultForDestination<TDestination extends DownloadDestination> =
  TDestination extends 'downloads' ? DownloadsDownloadResult : AppDownloadResult;

export type DownloadProgressEvent<TResult extends DownloadResult = DownloadResult> =
  | {
      kind: 'progress' | 'paused' | 'resumed';
      downloadedBytes?: number;
      totalBytes?: number;
      /** Present only when the total size is known. */
      progress?: number;
    }
  | {
      kind: 'canceled';
      downloadedBytes?: number;
      totalBytes?: number;
      progress?: number;
    }
  | {
      kind: 'completed';
      downloadedBytes?: number;
      totalBytes?: number;
      progress?: number;
      result: TResult;
    };

export type DownloadIteratorResult<TResult extends DownloadResult = DownloadResult> =
  IteratorResult<DownloadProgressEvent<TResult>, void>;

export type AppTempDownloadResult = Extract<AppDownloadResult, { tempFilePath: string }>;
export type AppPersistedDownloadResult = Extract<AppDownloadResult, { filePath: AppDownloadFilePath }>;

export type ChooseFileSingleResult = {
  canceled: false;
  paths: [string];
} | CanceledResult;

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
  /**
   * The lxapp's configured page names, one key per page.
   *
   * Empty here on purpose. `lingxia dev` / `lingxia build` generates the
   * project's own names into this interface; until then `ConfiguredPageName`
   * stays `string` and every navigation call compiles exactly as before.
   */
  interface LxAppPages {}

  // HostAppApi/LxEnv members are emitted from the Rust js_api metadata; these
  // merges only add what Rong cannot express — the cfg-gated autostart member
  // and doc comments (js_api consts cannot carry docs). env re-declares
  // the generated member doc-only; tsc rejects the merge if the types drift.
  interface HostAppApi {
    /**
     * The host deployment environment from `app.json::env` (`dev` | `prod`).
     * It is fixed at boot and defaults to `prod` for older artifacts.
     * Not the lxapp publish channel (`release` | `draft`).
     */
    readonly env: HostAppEnv;

    /**
     * Launch-at-startup control. Absent where the host cannot register a
     * startup item; its presence and `lx.supports('app.autostart')` always
     * agree, so `lx.app.autostart?.…` and the query are interchangeable.
     */
    autostart?: AutostartApi;

    /**
     * Local notifications. Absent where the host cannot post them; its presence
     * and `lx.supports('app.notification')` always agree.
     */
    notification?: NotificationApi;

    /**
     * Product-drawn desktop banner (top-right). Absent off desktop and in
     * guest lxapps; its presence and `lx.supports('app.banner')`
     * always agree.
     */
    banner?: BannerApi;

    /** The language this lxapp renders in. Every lxapp follows it. */
    readonly displayLanguage: DisplayLanguageApi;

    /** The light/dark scheme this lxapp renders in. */
    readonly appearance: AppearanceApi;

    /**
     * Product-wide settings, and their single writer. Present only in the
     * Control app the host sealed at build time. Use
     * `lx.app.control !== undefined` to inspect that identity.
     */
    readonly control?: ControlApi;

    /**
     * Product-wide cache reporting and clearing for a settings screen.
     * Present only in the Control app; presence agrees with
     * `lx.app.control !== undefined`.
     */
    cache?: AppCacheApi;

    /**
     * Take over host updates for the rest of this process. Irreversible: the
     * built-in auto-flow will not prompt or download again, including after
     * the calling page unloads. Does not cancel an already-started update task.
     * `update.apply()` claims as well. `checkUpdate()` does not.
     */
    claimCustomUpdate(): void;
  }

  /** Runtime environment constants backed by abstract `lx://` paths. */
  interface LxEnv {}

  interface Lx {
    /**
     * Terminal product settings. Present only in the host-bundled Terminal
     * Settings lxapp when the host declares `capabilities.terminal`; its
     * presence and `lx.supports('terminal')` always agree.
     */
    readonly terminal?: TerminalApi;

    /** Download to the downloads directory. */
    downloadFile(options: DownloadsDownloadOptions): DownloadTask<DownloadsDownloadResult>;
    /** Download to a durable app-owned path. */
    downloadFile(
      options: AppDownloadOptions & { filePath: string },
    ): DownloadTask<AppPersistedDownloadResult>;
    /** Download to a temporary app-owned path. */
    downloadFile(
      options: AppDownloadOptions & { filePath?: undefined },
    ): DownloadTask<AppTempDownloadResult>;
    /** Download to the lxapp-managed app directory. */
    downloadFile(options: AppDownloadOptions): DownloadTask<AppDownloadResult>;
    /** Download with a destination-correlated result type. */
    downloadFile<TDestination extends DownloadDestination = "app">(
      options: DownloadOptions<TDestination>,
    ): DownloadTask<DownloadResultForDestination<TDestination>>;
    /** Single-file picker: a completed selection is exactly one path. */
    chooseFile(options: ChooseFileOptions & { multiple: false }): Promise<ChooseFileSingleResult>;
    chooseFile(options: ChooseFileOptions & { multiple: true }): Promise<ChooseFileResult>;
    /**
     * Opens a file picker.
     * Resolves `{ canceled: true }` only when the user dismisses the picker. A
     * completed selection resolves `{ canceled: false, paths }` with at least one
     * path. Rejects when the picker fails or returns an invalid payload.
     */
    chooseFile(options?: ChooseFileOptions): Promise<ChooseFileResult>;

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
  has(key: StorageKey<S>): Promise<boolean>;
  delete(key: StorageKey<S>): Promise<void>;
  clear(): Promise<void>;
  list(prefix?: string): Promise<string[]>;
  info(): Promise<StorageInfo>;
};
