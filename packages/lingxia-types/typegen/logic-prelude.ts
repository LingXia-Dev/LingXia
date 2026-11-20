// Keep generic TypeScript-only aliases, correlated overloads, and types owned
// by external Rong modules in this generated prelude.
declare const appDownloadPathBrand: unique symbol;
declare const systemDownloadsPathBrand: unique symbol;
/** Managed paths and relative userdata paths; system Downloads references are excluded. */
export type ManagedPath = string & { readonly [systemDownloadsPathBrand]?: never };

/** Lifecycle hook names an `App({...})` config may declare. */
export type AppLifecycleName = keyof AppConfig;

/**
 * What a key that differs from a lifecycle hook only in case resolves to, so
 * the compiler names the mistake instead of silently accepting a new method.
 */
export type MisspelledLifecycle<K extends string> = {
  'LingXia type error': `'${K}' differs only in case from a lifecycle hook`;
};

/**
 * Applied to the custom half of an `App` config. `onload` and `onShow`
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
      : T extends readonly unknown[]
        ? { readonly [K in keyof T]: DeepReadonly<T[K], PageReadonlyDepth[D]> }
        : T extends object
          ? { readonly [K in keyof T]: DeepReadonly<T[K], PageReadonlyDepth[D]> }
          : T;

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

export type DataPathValue<T, P extends readonly PropertyKey[]> = T extends null | undefined
  ? never
  : T extends unknown
  ? P extends readonly [
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
    ? TData[K] | DeepReadonly<TData[K]>
    : { "LingXia type error": "unknown data key" };

export interface PageInstance<TData extends object = Record<string, unknown>> {
  readonly data: { readonly [K in keyof TData]: DeepReadonly<TData[K]> };
  route: string;
  /**
   * Available when this page was opened as a surface via
   * `lx.surface.openPage(...)`.
   */
  surface?: PageSurface;
  /**
   * Available when this page was pushed by `lx.nav.go(...)`.
   */
  opener?: PageMessagePort;
  /**
   * Set what the opener's `result` resolves with when this page leaves, by
   * Back or any other way. JSON only.
   */
  setResult(value: JsonValue): void;
  /**
   * Leave this page, with `value` as its result when given. A pushed page
   * goes back; a surface page closes its surface. Throws for a root or tab
   * page, and for a pushed page that is not on top.
   */
  close(value?: JsonValue): Promise<void>;
  setData<TPatch extends Record<string, unknown>>(
    data: TPatch & { [K in keyof TPatch]: SetDataValue<TData, K> },
  ): void;
  setPath<const P extends DataPath<TData>>(
    path: P,
    value: DataPathValue<TData, P>,
  ): void;
  /** Nested write by a runtime-resolved string path. JSON shape is checked; the path/value relationship is not. */
  setDataPath(path: string, value: JsonValue | undefined): void;
  /**
   * Drain pending state writes. A WebView acknowledges application, not paint. A GPU page resolves
   * once the frame carrying the writes is committed for display (one refresh before it shows),
   * never before they are in a frame and in write order; a hidden page resolves at once, and a
   * write no frame can show resolves after 250 ms. Rejects on unload.
   */
  flush(): Promise<void>;
  /**
   * Aborted as the page unloads, before `onUnload` runs, however the page
   * leaves. Pass it to work the page starts — `fetch(url, { signal:
   * this.signal })` — so that work stops with the page, rejecting with an
   * `AbortError`, instead of resolving into a page that is gone. When the
   * whole lxapp shuts down, its Logic ends with it and the signal does not fire.
   */
  readonly signal: AbortSignal;
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
 * Where `lx.network.download` writes, chosen by `to`:
 *
 * - omitted: a temporary file this lxapp owns
 * - a path: durable output under `lx://userdata` (relative paths resolve there)
 * - `'downloads'`: the user's Downloads folder, which needs the host's
 *   downloads grant
 */
export type DownloadOptions = AppDownloadOptions | DownloadsDownloadOptions;

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

export type AppTempDownloadResult = Extract<AppDownloadResult, { storage: 'temp' }>;
export type AppPersistedDownloadResult = Extract<AppDownloadResult, { storage: 'userdata' }>;

/** A running operation. Progress has one consumer; stopping observation does not cancel it. */
export interface Task<TResult, TProgress> {
  readonly result: Promise<TResult>;
  readonly progress: AsyncIterable<TProgress>;
}

export interface CancelableTask<TResult, TProgress> extends Task<TResult, TProgress> {
  /** Requests cancellation. Await result to observe the terminal outcome. */
  cancel(): Promise<void>;
}

export interface DownloadTask<TResult extends DownloadResult = DownloadResult>
  extends CancelableTask<TResult, DownloadProgressEvent<TResult>> {
  /** Pauses a running download; does nothing in any other state. Rejects when the platform cannot pause. */
  pause(): Promise<void>;
  /** Resumes a paused download from where it stopped; does nothing in any other state. */
  resume(): Promise<void>;
}

/** The params page `N` declares; any JSON params before the registry exists. */
export type NavParams<N> = N extends keyof LxAppPageTypes
    ? LxAppPageTypes[N] extends { readonly __lxPage: { readonly params: infer T } } ? T : PageQuery
    : PageQuery;

/** The result page `N` declares; any JSON value before the registry exists. */
export type NavResult<N> = N extends keyof LxAppPageTypes
    ? LxAppPageTypes[N] extends { readonly __lxPage: { readonly result: infer T } } ? T : JsonValue
    : JsonValue;

/** The messages page `N` declares as `{ in, out }`; undeclared when it has none. */
export type NavMessages<N> = N extends keyof LxAppPageTypes
    ? LxAppPageTypes[N] extends { readonly __lxPage: { readonly messages: infer M } } ? M : never
    : never;

/** What the opener sends to a page declaring `messages: type<{ in; out }>()`; anything when undeclared. */
export type MessageIn<M> = [M] extends [never] ? unknown : M extends { in: infer I } ? I : never;
/** What such a page sends back to its opener; anything when undeclared. */
export type MessageOut<M> = [M] extends [never] ? unknown : M extends { out: infer O } ? O : never;

/** A page to push or replace. `params` is required when the page's declared params have a required key. */
export type PageTarget<N = ConfiguredPageName> = { page: N; tab?: never; app?: never; url?: never }
    & ({} extends NavParams<N> ? { params?: NavParams<N> } : { params: NavParams<N> });

/** What `lx.nav.go` returns for a pushed page. */
export type PageHandle<R = JsonValue, M = never> = {
    /** Settles when the page is on screen; rejects when it could not open. */
    readonly opened: Promise<void>;
    /** Settles once, after the page's `onUnload`. */
    readonly result: Promise<{ status: 'ok'; value: R } | Extract<PageResult, { status: 'canceled' }>>;
    /**
     * Delivers \`message\` to the page's \`page.opener.onMessage\` handlers, at
     * once and uncopied. Until the page first subscribes, messages wait for it.
     */
    send(message: MessageIn<M>): void;
    /** Receives what the page sends with \`page.opener.send\`. Returns a function that removes the handler. */
    onMessage(handler: (message: MessageOut<M>) => void): () => void;
    /** Leave the page. Throws before `opened` settles, and for a page that is not on top. */
    close(): void;
};

declare global {
  /**
   * The lxapp's configured page names, one key per page.
   *
   * Empty here on purpose. `lingxia dev` / `lingxia build` generates the
   * project's own names into this interface; until then `ConfiguredPageName`
   * stays `string` and every navigation call compiles exactly as before.
   */
  interface LxAppPages {}

  /** The lxapp's tab roots, one key per tab page. Generated beside `LxAppPages`. */
  interface LxAppTabs {}

  /** Each page's `definePage` type, generated for Logic; `lx.nav` reads params and result from it. */
  interface LxAppPageTypes {}

  /** `lx.nav`: move this lxapp's page stack, or leave it for a tab, another app or a URL. */
  interface NavApi {
    /**
     * Push a page. The handle is returned at once: `opened` settles when the
     * page is on screen, `result` when it leaves.
     *
     * Throws `E_INVALID_ARG` for a malformed target (an unknown key, params
     * over 8 KB as JSON) or when ten pages are already stacked
     * (`data.reason: 'stack_full'`), and `E_NOT_FOUND` for a page the app does
     * not declare. `opened` rejects only when the platform fails to show it.
     */
    go<N extends ConfiguredPageName>(target: PageTarget<N>): PageHandle<NavResult<N>, NavMessages<N>>;
    /** Replace the current page; the new page inherits its opener and pending result. */
    go<N extends ConfiguredPageName>(target: PageTarget<N>, options: { replace: true }): Promise<void>;
    /**
     * Switch tabs, open another lxapp, or open a URL in the in-app browser.
     * A `{ tab }` takes no params. Rejects `E_NOT_FOUND` for an undeclared tab.
     */
    go(target: TabTarget | AppTarget | UrlTarget): Promise<void>;
    /** Pop `n` pages (default 1), stopping at the root. Resolves once the revealed page is ready. */
    back(n?: number): Promise<void>;
    /** Clear the stack and start at a page or tab; every discarded page's result settles canceled. */
    reset<N extends ConfiguredPageName>(target: PageTarget<N> | TabTarget): Promise<void>;
    /** Back to the lxapp that opened this one. */
    leaveApp(): Promise<void>;
  }

  // HostAppApi/FileSystemApi members are emitted from the Rust js_api
  // metadata; these merges only add what Rong cannot express — the cfg-gated
  // autostart member and doc comments (js_api consts cannot carry docs).
  // host.env, fs.userData and fs.userCache re-declare the generated members
  // doc-only; tsc rejects the merge if the types drift.
  interface HostAppApi {
    /**
     * The host deployment environment from `app.json::env` (`dev` | `prod`).
     * It is fixed at boot and defaults to `prod` for older artifacts.
     * Not the lxapp publish channel (`release` | `draft`).
     */
    readonly env: HostAppEnv;

    /**
     * Launch-at-startup control. Where the host cannot register a startup
     * item every call fails `E_NOT_SUPPORTED`; ask `lx.supports('app.autostart')`
     * first.
     */
    readonly autostart: AutostartApi;

    /**
     * Local notifications. Where the host cannot post them every call fails
     * `E_NOT_SUPPORTED`; ask `lx.supports('app.notification')` first.
     */
    readonly notification: NotificationApi;

    /**
     * Product-drawn desktop banner (top-right). Off desktop every call fails
     * `E_NOT_SUPPORTED`; ask `lx.supports('app.banner')` first.
     */
    readonly banner: BannerApi;

    /**
     * The language this lxapp renders in. Every lxapp follows it; only the
     * Control app changes it.
     */
    readonly displayLanguage: DisplayLanguageApi;

    /**
     * The light/dark scheme this lxapp renders in. Every lxapp follows it;
     * only the Control app changes it.
     */
    readonly appearance: AppearanceApi;

    /**
     * Product-wide cache reporting and clearing for a settings screen. Control
     * app only: another lxapp's calls fail `E_PERMISSION_DENIED`.
     */
    readonly cache: AppCacheApi;

    /**
     * Terminal product settings, for the host-bundled Terminal Settings lxapp.
     * Without `capabilities.terminal` every call fails `E_NOT_SUPPORTED`; ask
     * `lx.supports('terminal')` first.
     */
    readonly terminal: TerminalApi;

    /**
     * Take over host updates for the rest of this process. Irreversible: the
     * built-in auto-flow will not prompt or download again, including after
     * the calling page unloads. Does not cancel an already-started update task.
     * `update.apply()` claims as well. `checkUpdate()` does not.
     */
    claimCustomUpdate(): void;
  }

  interface FileSystemApi {
    /** The root of this lxapp's durable files; a relative path resolves here. */
    readonly userData: 'lx://userdata';
    /** The root of this lxapp's cache, which the host may clear. */
    readonly userCache: 'lx://usercache';
  }

  interface NetworkApi {
    /**
     * Download over HTTP(S), streamed to disk. Returns the task before the
     * transfer starts: `result` settles once, `progress` streams, `cancel()`
     * aborts.
     */
    download(url: string, options: DownloadsDownloadOptions): DownloadTask<DownloadsDownloadResult>;
    /** Download to a durable path under `lx://userdata`. */
    download(
      url: string,
      options: AppDownloadOptions & { to: string },
    ): DownloadTask<AppPersistedDownloadResult>;
    /** Download to a temporary file this lxapp owns. */
    download(
      url: string,
      options?: AppDownloadOptions & { to?: undefined },
    ): DownloadTask<AppTempDownloadResult>;
    download(url: string, options?: AppDownloadOptions): DownloadTask<AppDownloadResult>;
    download(url: string, options?: DownloadOptions): DownloadTask<DownloadResult>;
  }

  interface Lx {
    /**
     * Open this lxapp's store with every key's shape pinned on the handle.
     * `get` / `set` / `delete` then share that schema instead of
     * repeating `get<T>()` at each call site.
     */
    storage<S extends StorageSchema>(): TypedStorage<S>;
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
  [K in StorageKey<S>]: [key: K, value: S[K] | DeepReadonly<S[K]>];
}[StorageKey<S>];

/**
 * Schema-typed view of the same store `lx.storage()` returns.
 * Runtime is identical; only the key/value types are pinned.
 *
 * The schema constrains what this handle writes and reads, not what the store
 * contains: a previous app version, or another code path holding the untyped
 * handle, can have written keys outside it. That is why `list` still resolves
 * plain strings — narrowing it to the schema's keys would be the same
 * unchecked assertion this type exists to remove from `get<T>()`.
 */
export type TypedStorage<S extends object> = {
  get<K extends StorageKey<S>>(key: K, decode?: (value: unknown) => S[K]): Promise<S[K] | undefined>;
  set(...entry: StorageEntry<S>): Promise<void>;
  has(key: StorageKey<S>): Promise<boolean>;
  delete(key: StorageKey<S>): Promise<void>;
  clear(): Promise<void>;
  list(prefix?: string): Promise<string[]>;
  info(): Promise<StorageInfo>;
};
