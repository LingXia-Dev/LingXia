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
  onCancel(handler: () => void, options?: SignalOptions): () => void;
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
      /** Bytes arrived, or the transfer paused or resumed. */
      kind: 'progress' | 'paused' | 'resumed';
      /** Bytes written so far. */
      downloadedBytes?: number;
      /** The full size, when the server named it. */
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

export type AppTempDownloadResult = Extract<AppDownloadResult, { storage: 'temp' }>;
export type AppPersistedDownloadResult = Extract<AppDownloadResult, { storage: 'userdata' }>;

/** A running operation. Progress has one consumer; stopping observation does not cancel it. */
export interface Task<TResult, TProgress> {
  /** Settles once with the outcome. */
  readonly result: Promise<TResult>;
  /** Progress events while it runs; ends when it finishes. */
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
export type PageTarget<N = ConfiguredPageName> = {
    /** A page this lxapp declares. */
    page: N;
    tab?: never;
    app?: never;
    url?: never;
}
    & ({} extends NavParams<N> ? { params?: NavParams<N> } : { params: NavParams<N> });

/**
 * A page's handle, returned at once by `lx.nav.go` and `lx.surface.openPage`.
 * It is not a promise: await `.opened` to wait for the page.
 */
export type PageHandle<R = JsonValue, M = never> = {
    /**
     * Settles when the page is on screen; rejects when it could not open.
     * @throws {E_ABORT} `close()` was called first.
     */
    readonly opened: Promise<void>;
    /** Settles once, after the page's `onUnload`; `canceled` when it never opened because of `close()`. */
    readonly result: Promise<{
        /** The page gave a result. */
        status: 'ok';
        /** What it set by `setResult` or `close(value)`. */
        value: R;
    } | Extract<PageResult, { status: 'canceled' }>>;
    /**
     * Delivers `message` to the page's `page.opener.onMessage` handlers, at
     * once and uncopied. Until the page first subscribes, messages wait for it.
     */
    send(message: MessageIn<M>): void;
    /** Receives what the page sends with `page.opener.send`. Returns a function that removes the handler. */
    onMessage(handler: (message: MessageOut<M>) => void, options?: SignalOptions): () => void;
    /**
     * Leave the page; rejects for a pushed page that is not on top. Called
     * before `opened` settles, it stops the open instead.
     */
    close(): Promise<void>;
};

/** A page opened as a float or a window: its handle plus the surface's controls. */
export type PageSurface<R = JsonValue, M = never> = PageHandle<R, M> & Omit<SurfaceBase, 'close'> & SurfaceShowable & {
    /** One of this lxapp's pages. */
    readonly kind: 'page';
    /** Where it is shown. */
    readonly realized: 'float' | 'window';
};

/** `openPage`'s options with the page's params, required when the page declares a required param. */
export type OpenPageArgs<N> = {} extends NavParams<N>
    ? [options?: OpenPageOptions & { params?: NavParams<N> }]
    : [options: OpenPageOptions & { params: NavParams<N> }];

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
  interface SurfaceApi {
    /**
     * Open one of this lxapp's pages as a float or a window. Returns at once;
     * await `.opened` to wait for the page.
     * @throws {E_INVALID_ARG} Malformed options.
     * @throws {E_NOT_SUPPORTED} `as: 'window'` where the host has no windows.
     * @throws {E_NOT_FOUND} A page the app does not declare.
     */
    openPage<N extends ConfiguredPageName>(page: N, ...options: OpenPageArgs<N>): PageSurface<NavResult<N>, NavMessages<N>>;
  }

  interface NavApi {
    /**
     * Push a page. The handle is returned at once: `opened` settles when the
     * page is on screen, and rejects only when the platform cannot show it;
     * `result` settles when it leaves.
     * @throws {E_INVALID_ARG} An unknown key, params over 8 KB as JSON, or ten pages already stacked (`data.reason: 'stack_full'`).
     * @throws {E_NOT_FOUND} A page the app does not declare.
     */
    go<N extends ConfiguredPageName>(target: PageTarget<N>): PageHandle<NavResult<N>, NavMessages<N>>;
    /** Replace the current page; the new page inherits its opener and pending result. */
    go<N extends ConfiguredPageName>(target: PageTarget<N>, options: {
      /** Replace instead of push. */
      replace: true;
    }): Promise<void>;
    /**
     * Switch tabs, open another lxapp, or open a URL in the in-app browser.
     * A `{ tab }` takes no params.
     * @throws {E_NOT_FOUND} An undeclared tab.
     */
    go(target: TabTarget | AppTarget | UrlTarget): Promise<void>;
    /**
     * Pop `n` pages, stopping at the root. Resolves once the revealed page is ready.
     * @param [n=1] How many pages to pop.
     */
    back(n?: number): Promise<void>;
    /** Clear the stack and start at a page or tab; every discarded page's result settles canceled. */
    reset<N extends ConfiguredPageName>(target: PageTarget<N> | TabTarget): Promise<void>;
    /** Back to the lxapp that opened this one. */
    leaveApp(): Promise<void>;
  }

  // HostAppApi/FileSystemApi members are emitted from the Rust js_api
  // metadata; these merges only add what Rong cannot express — the
  // capability members' docs and doc comments (js_api consts cannot carry docs).
  // host.env, fs.userData and fs.userCache re-declare the generated members
  // doc-only; tsc rejects the merge if the types drift.
  interface HostAppApi {
    /**
     * The host's deployment environment from `app.json`, fixed at launch. Not
     * the lxapp's publish channel (`release` | `draft`).
     * @default 'prod'
     */
    readonly env: HostAppEnv;

    /**
     * Launch the host app when the user signs in.
     * @supports app.autostart
     * @throws {E_NOT_SUPPORTED} The host cannot register a startup item.
     */
    readonly autostart: AutostartApi;

    /**
     * Local notifications, posted now or scheduled.
     * @supports app.notification
     * @throws {E_NOT_SUPPORTED} The host cannot post them.
     */
    readonly notification: NotificationApi;

    /**
     * A banner the product draws at the top right of the desktop.
     * @supports app.banner
     * @throws {E_NOT_SUPPORTED} Off the desktop.
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
     * The product-wide cache, reported and cleared for a settings screen.
     * @throws {E_PERMISSION_DENIED} From an lxapp other than the Control app.
     */
    readonly cache: AppCacheApi;

    /**
     * Terminal settings, for the host-bundled Terminal Settings lxapp.
     * @supports terminal
     * @throws {E_NOT_SUPPORTED} Without `capabilities.terminal`.
     */
    readonly terminal: TerminalApi;

    /**
     * Take over host updates for the rest of this process. Irreversible: the
     * built-in auto-flow will not prompt or download again, including after
     * the calling page unloads. Does not cancel an already-started update task.
     * `update.apply()` claims as well. `checkUpdate()` does not.
     */
    claimCustomUpdate(): void;

    /** The host app's identity: OS, product name and version, runtime version. */
    readonly info: Readonly<HostInfo>;

    /** The menu-bar (macOS) or system-tray (Windows) item `lingxia.yaml` declares. */
    readonly tray: TrayApi;
  }

  interface LxAppApi {
    /** This lxapp's identity: id, name, version and channel. */
    readonly info: Readonly<AppInfo>;

    /** This lxapp's tab bar: items, badges and visibility. */
    readonly tabBar: TabBarApi;
  }

  interface DeviceApi {
    /** The device's maker, model and OS. */
    readonly info: Readonly<DeviceInfo>;

    /** Screen orientation: read it, lock it, follow it. */
    readonly orientation: OrientationApi;
  }

  interface ShellApi {
    /** The action rows the desktop sidebar shows for this lxapp. */
    readonly sidebarActions: ShellSidebarActionsApi;
  }

  interface FileSystemApi {
    /** The root of this lxapp's durable files; a relative path resolves here. */
    readonly userData: 'lx://userdata';
    /** The root of this lxapp's cache, which the host may clear. */
    readonly userCache: 'lx://usercache';
  }

  interface NetworkApi {
    /** Wi-Fi: scan, connect and follow the connected network. */
    readonly wifi: WifiApi;

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
    /** The host app: identity, notifications, badges, appearance, language, cache and updates. */
    readonly host: HostAppApi;
    /** Read and write the system clipboard. */
    readonly clipboard: ClipboardApi;
    /** The device: identity, screen, orientation, location, keys and haptics. */
    readonly device: DeviceApi;
    /** Network status, downloads, uploads and Wi-Fi. */
    readonly network: NetworkApi;
    /** This lxapp's files under `lx://` paths. */
    readonly fs: FileSystemApi;
    /** This lxapp: identity, tab bar, More actions and its own updates. */
    readonly app: LxAppApi;
    /** Image and video information, compression, preview and saving to the library. */
    readonly media: MediaApi;
    /** Ask the person for files, a directory, photos or videos, or a scanned code. */
    readonly pick: PickApi;
    /** Open pages, apps and URLs as floats, windows, asides and tabs. */
    readonly surface: SurfaceApi;
    /** Move between pages, tabs and lxapps. */
    readonly nav: NavApi;
    /** The desktop shell: running apps, declared surfaces, builtins and sidebar actions. */
    readonly shell: ShellApi;
    /** Dialogs, action sheets and toasts. */
    readonly ui: UiApi;

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
