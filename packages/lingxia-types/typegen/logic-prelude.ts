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
  /** The compiler's message for the misspelled key. */
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

/** A value that changes. `watch` calls back with the current value at once, then on each change. */
export interface Live<T> {
  /** The value now. */
  get(): T;
  /**
   * Follow it, starting with the current value; returns an unsubscribe. The
   * first callback runs before `watch` returns, so the unsubscribe is not yet
   * bound inside it.
   */
  watch(callback: (value: T) => void, options?: SignalOptions): () => void;
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

/** A download into a temporary file this lxapp owns. */
export type AppTempDownloadResult = Extract<AppDownloadResult, { storage: 'temp' }>;
/** A download into a durable file under `lx://userdata`. */
export type AppPersistedDownloadResult = Extract<AppDownloadResult, { storage: 'userdata' }>;

/** How far a task has got. */
export interface TransferProgress {
  /** Units done: bytes for a transfer. */
  readonly loaded: number;
  /** Units in all, when known. */
  readonly total?: number;
  /** `loaded / total`, `0` to `1`; present only when `total` is known. */
  readonly fraction?: number;
}

/**
 * A running operation. Iterate it for progress; the iteration returns the
 * result, so a streaming page action can `return yield* task`. One consumer
 * iterates; leaving the iteration early cancels a task that can be canceled.
 */
export interface Task<R, P = TransferProgress> extends AsyncIterable<P> {
  /**
   * Resolves with the result; rejects with the failure.
   * @throws {E_ABORT} The task was canceled.
   */
  readonly result: Promise<R>;
  [Symbol.asyncIterator](): AsyncIterator<P, R, undefined>;
}

/** A task the caller can cancel. */
export interface CancelableTask<R, P = TransferProgress> extends Task<R, P> {
  /** Requests cancellation; `result` then rejects. */
  cancel(): Promise<void>;
}

/** A running download: progress, its result, and pause, resume and cancel. */
export interface DownloadTask<TResult extends DownloadResult = DownloadResult>
  extends CancelableTask<TResult> {
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
    & ({} extends NavParams<N> ? {
        /** The page's params, as its `definePage` declares them. */
        params?: NavParams<N>;
    } : {
        /** The page's params, as its `definePage` declares them. */
        params: NavParams<N>;
    });

/** `lx.nav.current`: a page and its params, one member per configured page. */
export type NavCurrent = { [N in ConfiguredPageName]: {
    /** The page's name in `lxapp.json`. */
    page: N;
    /** The instance's current params. */
    params: NavParams<N>;
} }[ConfiguredPageName];

/** A navigation target that switches to one of this lxapp's tabs. */
export type TabTarget<N = TabName> = {
    /** A tab root of this lxapp. */
    tab: N;
    page?: never;
    app?: never;
    url?: never;
    /** The tab page's params from now on, handed to its `onShow` for this switch; a tab not created yet also loads with them. */
    params?: TabParams<N>;
};

/** A tab page's params; a tab whose page declares a required one cannot be switched to with them. */
export type TabParams<N> = {} extends NavParams<N> ? NavParams<N>
    : { 'LingXia type error': `tab '${N & string}' declares a required param; a tab's params must all be optional` };

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
    /** Settles once, when the page is closed (before its `onUnload`); `canceled` when it never opened because of `close()`. */
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

/** Arguments `lx.app.t` takes for a message: its tags keep their text, so they take nothing. */
type LogicMessageArgs<A> = { [P in keyof A as A[P] extends (...args: any) => any ? never : P]: A[P] };

declare const messageKey: unique symbol;
/** A key built at run time, named `as LxAppMessageKey`: still one of the lxapp's keys, its arguments unchecked. */
export type LxAppMessageKey = (keyof LxAppMessages extends never ? string : keyof LxAppMessages & string) & { readonly [messageKey]: true };

/** `lx.app.t`'s key and arguments, checked against `LxAppMessages` once the build generates it. */
export type LxAppMessageCall = keyof LxAppMessages extends never
  ? readonly [key: string, args?: Readonly<Record<string, unknown>>]
  : {
      [K in keyof LxAppMessages & string]: {} extends LogicMessageArgs<LxAppMessages[K]>
        ? readonly [key: K, args?: LogicMessageArgs<LxAppMessages[K]>]
        : readonly [key: K, args: LogicMessageArgs<LxAppMessages[K]>];
    }[keyof LxAppMessages & string]
    | readonly [key: LxAppMessageKey, args?: Readonly<Record<string, unknown>>];

declare global {
  /**
   * The lxapp's messages, each with the arguments it takes. Empty here: the build generates the
   * source catalogue's keys into it, and until then any key compiles.
   */
  interface LxAppMessages {}

  interface LxAppApi {
    /** The language this lxapp renders in: the product's, narrowed to its lxapp.json `languages`. */
    readonly language: AppLanguageApi;
    /** The light/dark scheme this lxapp renders in. */
    readonly appearance: AppAppearanceApi;
  }

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

  /** `lx.surface`: open pages, other lxapps, URLs and declared surfaces beside or over the main area. */
  interface SurfaceApi {
    /**
     * Open one of this lxapp's pages as a float or a window. Returns at once;
     * await `.opened` to wait for the page.
     * @example
     * ```ts
     * const sheet = lx.surface.openPage('settings', { as: 'float' });
     * await sheet.opened;
     * ```
     * @throws {E_INVALID_ARG} Malformed options.
     * @supports surface.window
     * @throws {E_NOT_SUPPORTED} `as: 'window'` where the host has no windows; `reason` is `'placementUnsupported'`.
     * @throws {E_NOT_FOUND} A page the app does not declare.
     */
    openPage<N extends ConfiguredPageName>(page: N, ...options: OpenPageArgs<N>): PageSurface<NavResult<N>, NavMessages<N>>;
  }

  /** `lx.nav`: move this lxapp's page stack, or leave it for a tab, another app or a URL. */
  /**
   * Stack moves (`go` to a page or tab, `back`, `reset`, a handle's `close`) run one at a time in
   * the order issued, each on the stack the earlier ones left. A move identical to the one waiting
   * just before it returns that move's handle or promise; one more past 8 waiting throws
   * `E_INVALID_STATE`. Closing a page an earlier move closed does nothing.
   */
  interface NavApi {
    /**
     * Push a page. The handle is returned at once: `opened` settles when the page is on screen,
     * `result` when it leaves.
     * @example
     * ```ts
     * const picker = lx.nav.go({ page: 'pick-contact' });
     * const contact = await picker.result;
     * ```
     * @throws {E_INVALID_ARG} An unknown key or params over 8 KB as JSON; `opened` rejects with it for a tab already on the stack (`reason: 'tabAlreadyOpen'`) or ten pages stacked (`reason: 'stackFull'`).
     * @throws {E_NOT_FOUND} A page the app does not declare.
     * @throws {E_INVALID_STATE} 8 moves already waiting.
     */
    go<N extends ConfiguredPageName>(target: PageTarget<N>): PageHandle<NavResult<N>, NavMessages<N>>;
    /** Back to the page's topmost open instance, or open it; settles once it is on screen. */
    go<N extends ConfiguredPageName>(target: PageTarget<N>, options: {
      /**
       * Return to the topmost open instance: the pages above it close as `back(n)` closes them
       * (`onUnload`, results canceled), and `params` become its params and reach the `onShow`
       * that reveals it. Already on top: params show it again, none do nothing. With none open,
       * a plain `go`. No handle: the instance keeps its opener.
       */
      reuse: true;
      /** With no open instance, replace the current page instead of pushing; ignored when one is open. */
      replace?: boolean;
    }): Promise<void>;
    /** Replace the current page; the new page inherits its opener and pending result. */
    go<N extends ConfiguredPageName>(target: PageTarget<N>, options: {
      /** Replace instead of push. */
      replace: true;
    }): Promise<void>;
    /**
     * Switch tabs; `params` become the tab's params and reach this switch's `onShow`.
     * @throws {E_NOT_FOUND} An undeclared tab.
     */
    go<T extends TabName>(target: TabTarget<T>): Promise<void>;
    /** Open another lxapp, or open a URL in the in-app browser. */
    go(target: AppTarget | UrlTarget): Promise<void>;
    /**
     * Pop `n` pages, stopping at the root. Resolves once the revealed page is ready.
     * @param [n=1] How many pages to pop.
     * @throws {E_INVALID_ARG} `n` is not a positive whole number.
     */
    back(n?: number): Promise<void>;
    /**
     * Clear the stack and start at a page or tab; every discarded page's result settles canceled.
     * @throws {E_INVALID_ARG} A target that is neither `{ page }` nor `{ tab }`.
     * @throws {E_NOT_FOUND} A page or tab the app does not declare.
     */
    reset<N extends ConfiguredPageName, T extends TabName>(target: PageTarget<N> | TabTarget<T>): Promise<void>;
    /**
     * Back to the lxapp that opened this one.
     * @throws {E_INTERNAL} There is no lxapp to go back to, or it could not be shown.
     */
    leaveApp(): Promise<void>;
    /**
     * The page on top of this lxapp's main page stack, with its current params; `null` only
     * before the first page. It changes once per navigation: a back over several pages reports
     * where it lands. Narrow on `page` to get that page's params.
     * @example
     * ```ts
     * lx.nav.current.watch((current) => console.log(current?.page));
     * ```
     */
    readonly current: Live<NavCurrent | null>;
  }

  // HostAppApi/FileSystemApi members are emitted from the Rust js_api
  // metadata; these merges only add what Rong cannot express — the
  // capability members' docs and doc comments (js_api consts cannot carry docs).
  // host.env, fs.userData and fs.userCache re-declare the generated members
  // doc-only; tsc rejects the merge if the types drift.
  /** `lx.host`: the host app every lxapp runs in, shared by all of them. */
  interface HostAppApi {
    /**
     * The host's deployment environment from `app.json`, fixed at launch. Not
     * the lxapp's publish channel (`release` | `draft`).
     * @default 'prod'
     */
    readonly env: HostAppEnv;

    /**
     * Launch the host app when the user signs in. Control app only.
     * @supports host.autostart
     * @throws {E_NOT_SUPPORTED} The host cannot register a startup item.
     */
    readonly autostart: AutostartApi;

    /**
     * Local notifications, posted now or scheduled.
     * @supports host.notification
     * @throws {E_NOT_SUPPORTED} The host cannot post them.
     */
    readonly notification: NotificationApi;

    /**
     * A banner the product draws at the top right of the desktop. Phones have
     * none by design: use `lx.ui.toast` for a notice and a modal for a choice.
     * @supports host.banner
     * @throws {E_NOT_SUPPORTED} Off the desktop.
     */
    readonly banner: BannerApi;

    /**
     * A mark on the product's own system chrome, such as an unread count.
     * Control app only.
     * @supports host.badge
     */
    readonly badge: BadgeApi;

    /**
     * The product's language preference. Only the Control app may read or set
     * it; an lxapp reads the language it renders in from `lx.app.language`.
     */
    readonly languagePreference: HostLanguagePreferenceApi;

    /**
     * The product's light/dark preference. Only the Control app may read or set
     * it; an lxapp reads the scheme it renders in from `lx.app.appearance`.
     */
    readonly appearancePreference: HostAppearancePreferenceApi;

    /**
     * The product's contrast preference. Only the Control app may read or set
     * it; a page reads the contrast in effect from `env.highContrast`.
     */
    readonly contrastPreference: HostContrastPreferenceApi;

    /**
     * The product-wide cache, reported and cleared for a settings screen.
     * @throws {E_PERMISSION_DENIED} From an lxapp other than the Control app.
     */
    readonly cache: AppCacheApi;

    /**
     * Terminal settings, for the host-bundled Terminal Settings lxapp.
     * @supports host.terminal
     * @throws {E_NOT_SUPPORTED} Without `capabilities.terminal`.
     */
    readonly terminal: TerminalApi;

    /**
     * Newer host app versions: `check()` finds one, `claim()` takes over the
     * built-in auto-flow. Control app only.
     */
    readonly update: HostUpdateApi;

    /** The host app's identity: OS, product name and version, runtime version. */
    readonly info: Readonly<HostInfo>;

    /** The service environment the app talks to, now and after its next launch. Control app only. */
    readonly serviceEnv: HostServiceEnvApi;

    /**
     * The menu-bar (macOS) or system-tray (Windows) item `lingxia.yaml` declares. Control app only.
     * @supports host.tray
     * @throws {E_NOT_SUPPORTED} Off the desktop.
     */
    readonly tray: TrayApi;
  }

  /** `lx.app`: this lxapp: its identity, tab bar, updates and More actions. */
  interface LxAppApi {
    /** This lxapp's identity: id, name, version and channel. */
    readonly info: Readonly<AppInfo>;

    /** This lxapp's tab bar: items, badges and visibility. */
    readonly tabBar: TabBarApi;

    /** This lxapp's own updates: follow a downloaded bundle and restart onto it. */
    readonly update: LxAppUpdateApi;

    /** The actions this lxapp adds to the host's More menu. */
    readonly moreActions: MoreActionsApi;
  }

  /**
   * `lx.host.tray`: the menu-bar (macOS) or system-tray (Windows) item `lingxia.yaml`
   * declares. Control app only; off the desktop every call rejects.
   */
  interface TrayApi {
    /** The dropdown menu under the tray item. */
    readonly menu: TrayMenuApi;
  }

  /** `lx.device`: the device: identity, screen, orientation, location, keys and haptics. */
  interface DeviceApi {
    /** The device's maker, model and OS. */
    readonly info: Readonly<DeviceInfo>;

    /** Screen orientation: read it and follow it; a page locks it with `page.orientation`. */
    readonly orientation: OrientationApi;

    /** Device permissions: read one without prompting, or ask for it. */
    readonly permission: PermissionApi;
  }

  /** `lx.clipboard`: the system clipboard: text, images, and what it holds. */
  interface ClipboardApi {}
  /** `lx.media`: inspect, compress and preview images and videos. */
  interface MediaApi {}
  /** `lx.device.orientation`: the orientation in effect, read or followed. */
  interface OrientationApi extends Live<DeviceOrientation> {}
  /** `lx.pick`: let the person choose files, a folder, photos and videos, or scan a code. */
  interface PickApi {}
  /** `lx.app.tabBar`: this lxapp's tab bar: items, badges and visibility. */
  interface TabBarApi {}
  /** `lx.device.permission`: read a device permission without prompting, or ask for it. */
  interface PermissionApi {}
  /** `lx.host.serviceEnv`: the service environment the app talks to; Control app only. */
  interface HostServiceEnvApi {}
  /** `lx.shell.appDisplay`: the name and icon the host shows for this app; Control app only. */
  interface AppDisplayApi {}
  /** `lx.ui`: the system's own dialogs, sheets and toasts. */
  interface UiApi {}
  /** `lx.network.wifi`: scan, join and follow Wi-Fi networks; the radio starts on the first call. */
  interface WifiApi {}

  interface ShellApi {
    /** The action rows the desktop sidebar shows for this lxapp. */
    readonly sidebarActions: ShellSidebarActionsApi;

    /** The name and icon the host shows for this app. Control app only. */
    readonly appDisplay: AppDisplayApi;
  }

  /** `lx.fs`: read and write files under this lxapp's managed `lx://` paths. */
  interface FileSystemApi {
    /** The root of this lxapp's durable files; a relative path resolves here. */
    readonly userData: 'lx://userdata';
    /** The root of this lxapp's cache, which the host may clear. */
    readonly userCache: 'lx://usercache';
  }

  /** `lx.network`: connectivity, transfers and Wi-Fi. */
  interface NetworkApi {
    /** Wi-Fi: scan, connect and follow the connected network. */
    readonly wifi: WifiApi;

    /**
     * Connectivity: whether it is up, its type and addresses. `get()` returns
     * the last value the platform reported, with no native round trip.
     * @example
     * ```ts
     * const { isConnected, networkType } = lx.network.status.get();
     * ```
     */
    readonly status: Live<NetworkInfo>;

    /**
     * Download over HTTP(S), streamed to disk. Returns the task before the
     * transfer starts: `result` settles once, iterating streams progress,
     * `cancel()` aborts.
     * @throws {E_INVALID_ARG} A URL that is not absolute http(s), malformed options, or a `to` already in use.
     * @throws {E_PERMISSION_DENIED} `to: 'downloads'` without the host's Downloads grant.
     * @throws {E_ABORT} `cancel()` was called or `signal` was aborted (on `result`).
     * @throws {E_NETWORK} The server answered with an error status (`reason` `'serverErrorStatus'`, the status in `data.status`) or did not answer (`'connectionFailed'`), on `result`.
     * @throws {E_TIMEOUT} The server stopped answering (on `result`).
     */
    download(url: string | URL, options: DownloadsDownloadOptions): DownloadTask<DownloadsDownloadResult>;
    /** Download to a durable path under `lx://userdata`, named by `to`. */
    download(
      url: string | URL,
      options: AppDownloadOptions & { to: string },
    ): DownloadTask<AppPersistedDownloadResult>;
    /** Download to a temporary file this lxapp owns. */
    download(
      url: string | URL,
      options?: AppDownloadOptions & { to?: undefined },
    ): DownloadTask<AppTempDownloadResult>;
    /** Download where `to` says, when it is not known until run time. */
    download(url: string | URL, options?: AppDownloadOptions): DownloadTask<AppDownloadResult>;
    /** Download with any options, when the destination is not known until run time. */
    download(url: string | URL, options?: DownloadOptions): DownloadTask<DownloadResult>;
  }

  interface UiApi {
    /**
     * Show a short message, or a caught failure in the words the View's `errorText` gives its code. A failure is
     * `danger` unless `options.tone` says otherwise; a cancel (`E_ABORT`) shows nothing and returns
     * a handle that does nothing.
     *
     * @example
     * ```ts
     * try { await save(); } catch (e) { lx.ui.toast(e); }
     * ```
     */
    toast(error: unknown, options?: ToastOptions & {
      /** The words when the failure's code has none of its own, such as "Couldn't save the note". */
      fallback?: string;
    }): ToastHandle;
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
    /** The desktop shell: running apps, declared surfaces and sidebar actions. */
    readonly shell: ShellApi;
    /** Dialogs, action sheets and toasts. */
    readonly ui: UiApi;

    /**
     * This lxapp's persistent key-value store. Values are JSON; declare each
     * key's shape once in `LxStorageSchema`, or every key holds any `JsonValue`.
     * Use `lx.fs` for path-based data.
     * A plain file in the app's data, not encrypted by LingXia: only the OS protects it (iOS data
     * protection, Android 10+ file-based encryption, the HarmonyOS el2 area); on macOS and Windows
     * any process of the same user can read it. Keep tokens and passwords out of it.
     * @example
     * ```ts
     * declare global { interface LxStorageSchema { lastTab: string } }
     * await lx.storage.set('lastTab', 'orders');
     * ```
     */
    readonly storage: Storage<LxStorageValues>;
  }
}

declare global {
  /**
   * Each `lx.storage` key and the shape stored under it. Empty here on
   * purpose: until an app declares keys, any key holds any `JsonValue`.
   */
  interface LxStorageSchema {}
}

/** The schema `lx.storage` uses: the app's `LxStorageSchema`, or any key with a JSON value. */
export type LxStorageValues = keyof LxStorageSchema extends never ? Record<string, JsonValue> : LxStorageSchema;

/**
 * `T` where every value survives a JSON round trip; a function, Date, Map,
 * Set, promise or class instance becomes `never`, so it does not compile.
 * `undefined` members are allowed: serialization drops them, as an optional
 * member reads.
 */
export type JsonData<T> =
  0 extends 1 & T ? any
    : unknown extends T ? unknown
    : T extends string | number | boolean | null | undefined ? T
    : T extends (...args: never[]) => unknown ? never
    : T extends Date | RegExp | Map<unknown, unknown> | Set<unknown> | PromiseLike<unknown> | symbol | bigint ? never
    : T extends readonly unknown[] ? { [K in keyof T]: JsonData<T[K]> }
    : T extends object ? { [K in keyof T]: JsonData<T[K]> }
    : never;

type StorageKey<S extends object> = Extract<keyof S, string>;
/**
 * `set`'s arguments. A schema pins each key's value, and the tuple keeps a
 * union key correlated with its value; without one, any JSON-safe value fits.
 */
type StorageEntry<S extends object, T> = string extends keyof S
  ? [key: string, value: T & JsonData<T>]
  : { [K in StorageKey<S>]: [key: K, value: JsonData<S[K]> | DeepReadonly<JsonData<S[K]>>] }[StorageKey<S>];

/** How `lx.storage.get` turns the stored value into its result. */
export type StorageGetOptions<T> = {
  /** Checks the stored value and returns it typed; whatever it throws rejects the read. Not called for a missing key. */
  decode: (value: unknown) => T;
};

/**
 * A persistent key-value store of JSON values, typed by a schema.
 *
 * The schema constrains what this handle reads and writes, not what the store
 * holds: an earlier app version can have written other keys, so `list`
 * resolves plain strings and `get` takes a `decode` option to check a value.
 */
export type Storage<S extends object = Record<string, JsonValue>> = {
  /**
   * Reads a value; a missing key resolves `undefined`, which a stored `null` never does.
   * @throws {E_INVALID_DATA} The stored value could not be decoded.
   * @throws {E_INVALID_STATE} The host configured no store for this lxapp.
   * @throws {E_IO} The database could not be read or written.
   */
  get<K extends StorageKey<S>>(key: K): Promise<S[K] | undefined>;
  /**
   * Reads a value and passes it through `options.decode`, which checks the stored shape and gives the result its type.
   * @throws {E_INVALID_DATA} The stored value could not be decoded.
   * @throws {E_INVALID_STATE} The host configured no store for this lxapp.
   * @throws {E_IO} The database could not be read or written.
   */
  get<K extends StorageKey<S>, T>(key: K, options: StorageGetOptions<T>): Promise<T | undefined>;
  /**
   * Stores a JSON value under `key`, replacing any value there.
   * @throws {E_OUT_OF_RANGE} The key is over 1 KiB, the value over 5 MiB, or the store would pass 20 MiB.
   * @throws {E_INVALID_STATE} The host configured no store for this lxapp.
   * @throws {E_IO} The database could not be read or written.
   */
  set<T>(...entry: Readonly<StorageEntry<S, T>>): Promise<void>;
  /**
   * Resolves whether the key exists, without reading its value.
   * @throws {E_INVALID_STATE} The host configured no store for this lxapp.
   * @throws {E_IO} The database could not be read or written.
   */
  has(key: StorageKey<S>): Promise<boolean>;
  /**
   * Removes the key; a missing key is fine.
   * @throws {E_INVALID_STATE} The host configured no store for this lxapp.
   * @throws {E_IO} The database could not be read or written.
   */
  delete(key: StorageKey<S>): Promise<void>;
  /**
   * Removes every key, including ones outside the schema.
   * @throws {E_INVALID_STATE} The host configured no store for this lxapp.
   * @throws {E_IO} The database could not be read or written.
   */
  clear(): Promise<void>;
  /**
   * Resolves every stored key, optionally filtered by prefix.
   * @throws {E_INVALID_STATE} The host configured no store for this lxapp.
   * @throws {E_IO} The database could not be read or written.
   */
  list(prefix?: string): Promise<readonly string[]>;
  /**
   * Resolves how much is stored, the limit, and the key count.
   * @throws {E_INVALID_STATE} The host configured no store for this lxapp.
   * @throws {E_IO} The database could not be read or written.
   */
  info(): Promise<StorageInfo>;
};
