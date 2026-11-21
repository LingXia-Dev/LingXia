//! Public TypeScript-only API types consumed by `rong-typegen`.
//! Runtime-backed structs and classes are extracted from their Rust definitions;
//! semantic unions, callbacks, and handles live here as the remaining source metadata.

use rong::{JSContext, JSResult};

pub(crate) fn init(ctx: &JSContext) -> JSResult<()> {
    register_public_types(ctx)
}

rong::js_api! {
    fn register_public_types(ctx) {
        namespace Lx = ctx.global().get::<_, rong::JSObject>("lx")?;

        /// The `{ signal }` every subscription (`watch`, `on<Event>`) takes. Aborting
        /// it unsubscribes; `{ signal: page.signal }` ends it with the page.
        type SignalOptions = r###"{
    /** Aborting it ends the call or subscription. */
    signal?: AbortSignal;
}"###;

        /// The user dismissed the operation. Never an error.
        ///
        type CanceledResult = r###"{
    /** The person dismissed it. */
    status: 'canceled';
}"###;

        /// Result of `lx.ui.actionSheet`. Branch on `status` before reading
        /// the selected item id.
        type ActionSheetResult = r###"{
    /** An action was picked. */
    status: 'ok';
    /** Stable id of the selected action. */
    id: string;
} | CanceledResult"###;

        type AppConfig = r###"{
    /** Runs once when the lxapp starts, before its first page. */
    onLaunch?: (options?: AppLaunchOptions) => void | Promise<void>;
    /**
     * An App Link from outside, cold (after `onLaunch` settles) or warm
     * (before `onShow`). Turn its untrusted strings into a target here; the
     * framework navigates to it, and returning nothing stays where the app
     * is (the landing page on a cold start).
     */
    onOpen?: (link: OpenLink) => NavTarget | void | Promise<NavTarget | void>;
    /** Runs each time the lxapp comes to the front. */
    onShow?: (args?: AppLifecycleEventArgs) => void | Promise<void>;
    /** Runs each time the lxapp leaves the front. */
    onHide?: (args?: AppLifecycleEventArgs) => void | Promise<void>;
    /** Runs after the person takes a screenshot while the lxapp is in front. */
    onUserCaptureScreen?: () => void | Promise<void>;
}"###;

        /// Runtime-managed app download path, usually under `lx://userdata`.
        type AppDownloadFilePath = r###"string & {
    readonly [appDownloadPathBrand]: 'app-download-file-path';
}"###;

        type AppDownloadOptions = r###"DownloadOptionsBase & {
    /**
     * Durable output path under `lx://userdata`; relative paths resolve there.
     * The file must not exist yet. Omit for a temporary file. `'downloads'`
     * is the user's Downloads folder instead (`DownloadsDownloadOptions`).
     */
    to?: string;
    suggestedName?: never;
}"###;

        type AppDownloadResult = r###"{
    uri: AppDownloadFilePath;
    storage: 'temp';
    mimeType?: string;
    sizeBytes: number;
} | {
    uri: AppDownloadFilePath;
    storage: 'userdata';
    mimeType?: string;
    sizeBytes: number;
}"###;

        /// Canonical platform-family label shared by `lx.host.info.os`
        /// and `lx.device.info.osName`. `"unknown"` is a non-product build.
        type HostOs = r###"'iOS' | 'macOS' | 'Android' | 'Windows' | 'Harmony' | 'unknown'"###;

        /// Configured page name from `lxapp.json` / `lingxia.yaml`. JavaScript
        /// navigation accepts only this name; full routes such as
        /// `/pages/home/index` are internal runtime details. Discover names
        /// with `lxdev lxapp pages`.
        ///
        /// Narrows to the project's own names once `lingxia dev` or
        /// `lingxia build` has generated them; plain `string` before that, so
        /// a project that never ran a build still compiles.
        type ConfiguredPageName = r###"keyof LxAppPages extends never ? string : keyof LxAppPages"###;

        /// Configured page name belonging to *another* lxapp. This app's own
        /// page union cannot check it, so it stays a plain string and the
        /// target runtime rejects a name it does not have.
        type ExternalPageName = r###"string"###;

        /// Launch scene. `8003` is AppLink (cold: `onLaunch`; warm: `onShow`).
        /// Other numeric scenes stay valid; completion offers `8003`.
        type AppLaunchScene = r###"8003 | (number & {})"###;

        type AppLaunchOptions = r###"{
    /** The page the lxapp opens at. */
    path?: string;
    /** That page's query. */
    query?: Record<string, string>;
    /** `8003` = AppLink (cold: onLaunch; warm: onShow). */
    scene?: AppLaunchScene;
    /**
     * Inbound link exactly as the OS delivered it, fragment included. Present
     * only with `scene: 8003`. Untrusted: route from an allowlist of paths.
     */
    url?: string;
    /** The lxapp that opened this one, when one did. */
    referrerInfo?: {
        /** Its id. */
        appId?: string;
        /** What it passed along. */
        extraData?: Record<string, unknown>;
    };
}"###;

        /// An inbound App Link, as `App.onOpen` receives it.
        type OpenLink = r###"{
    /** The link exactly as the OS delivered it. Untrusted. */
    url: string;
    /** The lxapp page path the link resolved to, if any. */
    path: string;
    /** The link's query parameters. Untrusted. */
    query: Record<string, string>;
}"###;

        type AppLifecycleEventArgs = r###"{
    /** `host` when the whole app moved; `lxapp` when only this lxapp did. */
    source: 'host' | 'lxapp';
    /** What moved it: the app going to the foreground or background, a screenshot, this lxapp opening or closing, or another lxapp taking or giving back the front. */
    reason: 'foreground' | 'background' | 'screenshot' | 'open' | 'close' | 'switch_back' | 'switch_away';
    /** The page a link or switch-back lands on. */
    path?: string;
    /** That page's query. */
    query?: Record<string, string>;
    /** `8003` = AppLink. */
    scene?: AppLaunchScene;
    /** Inbound link, present only with `scene: 8003`. */
    url?: string;
}"###;

        type AppScreenshotOptions = r###"{
    /**
     * Platform-specific window id to capture (desktop only). Omit to let the
     * platform pick: the key/main window on desktop, the sole window on mobile.
     */
    windowId?: string;
}"###;

        type AppScreenshotResult = r###"{
    /** `lx://` URI of the captured PNG in the lxapp temp directory. */
    uri: string;
    /** Image width in pixels, when the runtime could read it from the PNG. */
    width?: number;
    /** Image height in pixels, when the runtime could read it from the PNG. */
    height?: number;
}"###;

        /// A JSON value: what a navigation route parameter may hold. Not a way
        /// to smuggle a payload — every route declares its own parameter
        /// schema, and the framework caps size, count, and nesting.
        type JsonValue = r###"
    | null
    | boolean
    | number
    | string
    | JsonValue[]
    | { [key: string]: JsonValue }"###;

        /// Where a notification tap or a banner action goes: a page, tab or
        /// lxapp as `lx.nav.go` takes it, or `{ activate }`, `{ route }`,
        /// `{ appLink }`. One destination per target; a mix is a parameter error.
        type HostTarget = r###"PageTarget | TabTarget | Omit<AppTarget, 'channel' | 'version'> | ActivateTarget | RouteTarget | AppLinkTarget"###;

        type ActivateTarget = r###"{
    /** Bring the product forward, nothing else. */
    activate: true;
    page?: never;
    tab?: never;
    app?: never;
    route?: never;
    appLink?: never;
}"###;

        type RouteTarget = r###"{
    /** A location the host registered at startup that is not a page. */
    route: string;
    /** Values for the route's declared params. */
    params?: Record<string, JsonValue>;
    page?: never;
    tab?: never;
    app?: never;
    appLink?: never;
}"###;

        type AppLinkTarget = r###"{
    /** An `https://` URL that is also an inbound App Link (`scene === 8003`). */
    appLink: string;
    page?: never;
    tab?: never;
    app?: never;
    route?: never;
}"###;

        /// Local notifications as a resume affordance.
        ///
        /// Without `capabilities.notifications`, or where the platform has no
        /// local API, every call fails `E_NOT_SUPPORTED`; ask
        /// `lx.supports('app.notification')` first.
        /// Declaring the capability never prompts; permission runs on
        /// `requestPermission()` or the first `show()` that reaches the OS.
        ///
        /// Every lxapp may call it. The Control app speaks for the product; a
        /// guest lxapp speaks only for itself: its ids are its own (it cannot
        /// replace or cancel another app's), a tap can only open itself, it
        /// may hold 32 live notifications, and it may cause at most one
        /// permission prompt per launch, only while it is open on screen.
        type NotificationApi = r###"{
    /**
     * Read the current permission without prompting. `'default'` means the
     * user has not been asked yet.
     */
    getPermission(): Promise<'granted' | 'denied' | 'default'>;
    /**
     * Ask for notification permission. Prompts where the OS has a prompt and
     * the user has not answered; otherwise reports the current setting.
     * Rejects when the prompt is left unanswered.
     * @permission notifications
     */
    requestPermission(): Promise<'granted' | 'denied'>;
    /**
     * Post a local notification now or at `schedule`, replacing any under the same `id`.
     * An invalid target, unknown page or route, or an unconfigured host rejects here,
     * before anything is posted. A tap whose target is gone by then brings the
     * product forward and says it is unavailable.
     * @throws {E_PERMISSION_DENIED} A guest lxapp's `target` names anything but itself.
     */
    show(options: {
        /** Replace key: what is pending or delivered under it is replaced and its tap target retired. Generated when absent. */
        id?: string;
        /** First line. */
        title: string;
        /** Text below the title. */
        body?: string;
        /**
         * Where a tap goes; absent brings the product forward, nothing else.
         * A guest lxapp may only target itself: `{ page }`, `{ tab }` or `{ app }` with its own id.
         */
        target?: HostTarget;
        /** When to present; a time not in the future posts now. */
        schedule?: {
            /** Epoch milliseconds. */
            at: number;
        } | {
            /** Milliseconds from now. */
            delayMs: number;
        };
        /** No sound. The banner still appears. */
        silent?: boolean;
    }): Promise<{
        /** The replace key, generated when none was given. */
        id: string;
        /**
         * `posted`: the OS took it for display, not proof anyone saw it. `scheduled`:
         * queued for `schedule`, and shown even if the product is frontmost then.
         * `suppressed`: an immediate post while the product is frontmost; nothing
         * posted, no permission needed.
         */
        status: 'posted' | 'scheduled' | 'suppressed';
    }>;
    /**
     * Remove what is pending or delivered under `id`, and retire its tap
     * target. Unknown ids are fine.
     */
    cancel(id: string): Promise<void>;
    /**
     * Remove every local notification this API posted or scheduled. A guest
     * lxapp's call removes only its own.
     */
    cancelAll(): Promise<void>;
}"###;

        /// Product-drawn desktop banner, top-right. Not an OS notification and
        /// not bound to App Link. Desktop only, in every lxapp: elsewhere every
        /// call fails `E_NOT_SUPPORTED`, so ask `lx.supports('app.banner')` first.
        ///
        /// A guest lxapp's banner carries its name and icon, so
        /// it cannot pass as the product's. Its ids are its own, it may have
        /// three banners showing or waiting, each closes after 60s at most
        /// (`timeoutMs` is capped), and the product's banners go first.
        ///
        /// No buttons: an informational card that auto-dismisses (5s unless
        /// `timeoutMs` is set). With buttons: a gate that waits for a choice,
        /// dismiss, timeout, or replace. User outcomes resolve; presentation
        /// failures reject.
        type BannerApi = r###"{
    /** Show a banner, or replace the one with the same `id`; resolves with what the person did. */
    show(options: {
        /** Replace key; a banner under it is replaced and resolves `replaced`. */
        id?: string;
        /** First line. */
        title: string;
        /** Text below the title. */
        body?: string;
        /** Buttons; with any, the banner waits for a choice instead of closing by itself. */
        actions?: Array<{
            /** What `action` reports when this button is picked. */
            id: string;
            /** Button text. */
            label: string;
            /** Emphasis: `primary` for the expected choice, `destructive` for a dangerous one. */
            style?: 'default' | 'primary' | 'destructive';
            /**
             * Where picking this action goes, as a notification target, checked
             * before the banner shows; absent, picking it only resolves `show`.
             */
            target?: HostTarget;
        }>;
        /**
         * Close after this many milliseconds; a guest's banners close after 60000 at most.
         * @default 5000 without buttons; with buttons, it waits for a choice
         */
        timeoutMs?: number;
        /** Omit/`system` follows the OS. `light`/`dark` force chrome. `#RGB`/`#RRGGBB`/`#RRGGBBAA` is a solid fill. */
        background?: 'system' | 'light' | 'dark' | string;
    }): Promise<
        | {
            /** A button was picked. */
            status: 'ok';
            /** The banner's replace key. */
            id: string;
            /** The picked button's `id`. */
            action: string;
        }
        | {
            /** It closed without a choice. */
            status: 'canceled';
            /** The banner's replace key. */
            id: string;
            /** Closed by the person, by `timeoutMs`, or by a banner with the same `id`. */
            reason: 'dismissed' | 'timeout' | 'replaced';
        }
    >;
    /** Unknown ids are fine. */
    dismiss(id: string): Promise<void>;
}"###;

        /// Launch-at-startup control for the host app.
        ///
        /// Wherever the host cannot register a startup item every call fails
        /// `E_NOT_SUPPORTED`; ask first:
        ///
        /// ```ts
        /// if (lx.supports('app.autostart')) {
        ///   // render the "Launch at startup" toggle
        /// }
        /// ```
        ///
        /// Requires `capabilities.autostart: true` in `lingxia.yaml`; without it
        /// `lx.supports` says no on all platforms. Declaring the capability never enables
        /// autostart by itself — the SDK registers the app only when `enable()`
        /// is called, so the decision stays with the user (typically a settings-page
        /// toggle, default off).
        ///
        /// Host-app-level capability: like `checkUpdate` and `screenshot`, the methods
        /// are available only to the native-assigned Control app; other lxapps receive
        /// a permission error.
        type AutostartApi = r###"{
    /**
     * Whether the app is registered to launch at startup, read from the OS
     * each time; the user can change it outside the app, so re-read it when
     * the settings screen shows.
     * @platform macOS: login items. Windows: the `Run` registry key.
     */
    isEnabled(): Promise<boolean>;
    /**
     * Register the app as a startup item for the current user.
     * Idempotent. Call it only from the person's own action.
     * @platform macOS: the system may tell the user a login item was added.
     */
    enable(): Promise<void>;
    /** Unregister the app as a startup item for the current user. Idempotent. */
    disable(): Promise<void>;
}"###;

        /// The product-wide cache a settings screen reports and clears.
        ///
        /// App-scoped, not lxapp-scoped: the figure covers every lxapp the host
        /// has run. Control app only; a guest's call fails `E_PERMISSION_DENIED`.
        type AppCacheApi = r###"{
    /** Bytes `clear()` would free, estimated; open lxapps' caches and the WebView cache are not counted. */
    size(): Promise<number>;
    /**
     * Clear reclaimable host caches. Control app only. Live session usercache and
     * temp are preserved, including the caller's. Does not restart any lxapp.
     * Userdata, KV, Downloads, cookies, valid installs and host components survive.
     * Per-category failures are reported; setup/worker failures reject the call.
     */
    clear(): Promise<{
        /** Estimated file bytes successfully removed; excludes WebView cache. */
        freedBytes: number;
        /** Protected usercache/session paths skipped, not a count of apps. */
        skippedActivePaths: number;
        /** What happened to the WebView's own cache. */
        webview: 'cleared' | 'unsupported' | 'failed';
        /** Categories that could not be cleared. */
        failures: string[];
    }>;
}"###;
        type TerminalFontSettings = r###"{
    /** Ordered candidates; the first installed monospaced family wins. */
    family: string[];
    /**
     * Font size in points.
     * @default 13
     */
    size: number;
    /**
     * Line height as a multiple of the font size.
     * @default 1
     */
    lineHeight: number;
    /** Draw the font's ligatures. */
    ligatures: boolean;
}"###;

        /// A light scheme and a dark one. Which is in use follows the
        /// product's light/dark setting; the terminal has no switch of its own.
        type TerminalThemeSettings = r###"{
    /** Color scheme name used in light appearance. */
    light: string;
    /** Color scheme name used in dark appearance. */
    dark: string;
}"###;

        type TerminalSettingsValue = r###"{
    /** Font settings. */
    font: TerminalFontSettings;
    /** Color scheme per appearance. */
    theme: TerminalThemeSettings;
}"###;

        type TerminalSettingsPatch = r###"{
    /** Font fields to change; others keep their value. */
    font?: Partial<TerminalFontSettings>;
    /** Scheme names to change; others keep their value. */
    theme?: Partial<TerminalThemeSettings>;
}"###;

        type TerminalSettingsWarning = r###"{
    /** What is wrong: the user's settings file does not parse, or names a scheme that does not exist. */
    code: 'invalidUserFile' | 'missingColorScheme';
    /** A sentence for a person. */
    message: string;
}"###;

        type TerminalSettingsSnapshot = r###"{
    /** Monotonic process revision used by update/reset compare-and-swap. */
    revision: number;
    /** Framework defaults. */
    defaults: TerminalSettingsValue;
    /** User-authored fields only. */
    overrides: TerminalSettingsPatch;
    /** Resolved configuration after all valid layers. */
    value: TerminalSettingsValue;
    /** What the terminal draws with now. */
    effective: {
        /** The scheme the product is in, and therefore the terminal too. */
        appearance: 'light' | 'dark';
        /** The scheme in use, or `null` when its name resolves to none. */
        colorScheme: string | null;
        /** The font in use. */
        font: {
            /** The family drawn. */
            family: string;
            /** Requested families that are not installed. */
            missing: string[];
            /** None of the requested families is installed, so a system monospace font is drawn. */
            fellBack: boolean;
        };
    };
    /** Problems found while reading the settings; they never block it. */
    warnings: TerminalSettingsWarning[];
}"###;

        type TerminalColorScheme = r###"{
    /** Display name. */
    name?: string;
    /** Background color, `#rrggbb`. */
    background: string;
    /** Text color when no other is set. */
    foreground: string;
    /** Cursor color; the foreground when absent. */
    cursorColor?: string;
    /** Selection fill. */
    selectionBackground?: string;
    /** Text color inside a selection. */
    selectionForeground?: string;
    /** ANSI color 0. */
    black: string;
    /** ANSI color 1. */
    red: string;
    /** ANSI color 2. */
    green: string;
    /** ANSI color 3. */
    yellow: string;
    /** ANSI color 4. */
    blue: string;
    /** ANSI color 5. */
    purple: string;
    /** ANSI color 6. */
    cyan: string;
    /** ANSI color 7. */
    white: string;
    /** ANSI color 8. */
    brightBlack: string;
    /** ANSI color 9. */
    brightRed: string;
    /** ANSI color 10. */
    brightGreen: string;
    /** ANSI color 11. */
    brightYellow: string;
    /** ANSI color 12. */
    brightBlue: string;
    /** ANSI color 13. */
    brightPurple: string;
    /** ANSI color 14. */
    brightCyan: string;
    /** ANSI color 15. */
    brightWhite: string;
}"###;

        type TerminalColorSchemeDetails = r###"{
    /** The name settings refer to it by. */
    name: string;
    /** Shipped with the product, or imported by the user. */
    source: 'builtIn' | 'imported';
    /** Its colors. */
    scheme: TerminalColorScheme;
}"###;

        type InstalledTerminalFont = r###"{
    /** Family name, as `TerminalFontSettings.family` takes it. */
    family: string;
    /** Every glyph has the same width. */
    monospace: boolean;
    /** The font has programming ligatures. */
    ligatures: boolean;
    /** The font carries Nerd Font icons. */
    nerdIcons: boolean;
}"###;

        type TerminalPreviewController = r###"{
    /** Preview a stored name or an unpersisted scheme. Last request wins. */
    show(scheme: string | TerminalColorScheme): Promise<void>;
    /** Restore saved settings only when this controller owns the preview. */
    clear(): Promise<void>;
    /** Idempotently clear and retire this controller. */
    close(): Promise<void>;
}"###;

        type TerminalSettingsApi = r###"{
    /** The settings now: defaults, the user's overrides, and what is in effect. */
    get(): Promise<TerminalSettingsSnapshot>;
    /** Save the patched fields; rejects when the saved settings moved past `ifRevision`. */
    update(patch: TerminalSettingsPatch & {
        /** The `revision` the patch was made against. */
        ifRevision: number;
    }): Promise<TerminalSettingsSnapshot>;
    /** Drop the user's overrides; rejects when the saved settings moved past `ifRevision`. */
    reset(options: {
        /** The `revision` the reset was decided against. */
        ifRevision: number;
        /** Reset only the font or only the theme; both when absent. */
        scope?: 'font' | 'theme';
    }): Promise<TerminalSettingsSnapshot>;
    /** Fires after saved settings, effective appearance, or fonts change. */
    onChange(listener: (snapshot: TerminalSettingsSnapshot) => void, options?: SignalOptions): () => void;
}"###;

        type TerminalColorSchemesApi = r###"{
    /** Every scheme, shipped and imported. */
    list(): Promise<TerminalColorSchemeDetails[]>;
    /** Add a scheme from its text. */
    import(options: {
        /** A Windows Terminal JSON scheme, or Xresources or kitty `name: value` lines. */
        text: string;
        /** Name to save it under; the scheme's own name, else `imported`, when absent. */
        name?: string;
        /** Existing names are rejected unless overwrite is explicit. */
        overwrite?: boolean;
    }): Promise<TerminalColorSchemeDetails>;
    /** A controller that shows a scheme live without saving it. */
    createPreview(): TerminalPreviewController;
}"###;

        type TerminalFontsApi = r###"{
    /** The font families installed on this machine. */
    list(): Promise<InstalledTerminalFont[]>;
}"###;

        type WindowsTerminalInlineImageStatus = r###"{
    enabled: boolean;
    installed: boolean;
    package: {
        version: string;
        url: string;
        sha256: string;
        bytes: number;
    };
}"###;

        type WindowsTerminalApi = r###"{
    status(): Promise<WindowsTerminalInlineImageStatus>;
    /** Verify and install the fixed Microsoft ConPTY package from lxapp temp storage. */
    install(options: { path: string }): Promise<WindowsTerminalInlineImageStatus>;
    /** Select the installed runtime for new terminal sessions. */
    setEnabled(options: { enabled: boolean }): Promise<WindowsTerminalInlineImageStatus>;
}"###;

        type TerminalApi = r###"{
    /** Saved terminal settings, revision-checked on write. */
    readonly settings: TerminalSettingsApi;
    /** Installed color schemes, plus import and live preview. */
    readonly colorSchemes: TerminalColorSchemesApi;
    /** Terminal fonts installed on this machine. */
    readonly fonts: TerminalFontsApi;
    /** Windows-only optional inline-image compatibility runtime. */
    readonly windows?: WindowsTerminalApi;
}"###;

        type BinaryFileData = r###"ArrayBuffer | ArrayBufferView"###;

        /// What the product's light/dark scheme is set to. `'auto'` follows
        /// the system.
        type AppearancePreference = r###"'auto' | 'light' | 'dark'"###;

        /// `lx.host.appearance` — the scheme this lxapp renders in.
        type AppearanceApi = r###"{
    /**
     * The scheme this lxapp is rendering in. An lxapp that pinned one in its
     * `lxapp.json` reports that; every other lxapp reports the product's.
     */
    get(): ResolvedAppearance;
    /**
     * Follow it, starting with the current value. Returns an unsubscribe.
     *
     * The first callback runs synchronously, before `watch` returns, so the
     * unsubscribe is not yet bound inside it.
     */
    watch(callback: (resolved: ResolvedAppearance) => void, options?: SignalOptions): () => void;
    /** What the user chose for the whole product. Control app only. */
    getPreference(): AppearancePreference;
    /**
     * Pin the product to `'light'` or `'dark'`, or follow the system with
     * `'auto'`. An lxapp that pinned a scheme in its manifest keeps it.
     * Control app only.
     */
    setPreference(preference: AppearancePreference): Promise<void>;
    /**
     * Follow the choice, not what it resolves to: a system flip under `'auto'`
     * moves `lx.host.appearance.watch` and leaves this quiet. Starts with the
     * current value; that first callback runs synchronously, before
     * `watchPreference` returns. Control app only.
     */
    watchPreference(callback: (preference: AppearancePreference) => void, options?: SignalOptions): () => void;
}"###;
        /// What the product's language is set to: `'auto'` follows the system,
        /// or any canonical BCP-47 tag. `string & {}` keeps `'auto'` in
        /// autocomplete while still accepting a tag.
        type DisplayLanguagePreference = r###"'auto' | (string & {})"###;

        /// `lx.host.displayLanguage` — the language this lxapp renders in.
        type DisplayLanguageApi = r###"{
    /**
     * The language in effect right now, as a canonical BCP-47 tag. Map it to
     * the catalogs this lxapp actually ships and fall back where it has none;
     * that narrowing is yours, and is not a language setting of its own.
     */
    get(): string;
    /**
     * Follow the language, starting with the current value. Returns an
     * unsubscribe.
     *
     * Logic needs this because the strings it hands to native chrome —
     * navigation bar titles, tab bar labels, modal and action-sheet text — are
     * the app's own, and nothing re-renders them on its behalf.
     *
     * The first callback runs synchronously, before `watch` returns, so the
     * unsubscribe is not yet bound inside it.
     */
    watch(callback: (language: string) => void, options?: SignalOptions): () => void;
    /** What the user chose: `'auto'`, or a canonical BCP-47 tag. Control app only. */
    getPreference(): DisplayLanguagePreference;
    /**
     * Persist the product's language. Rejects a tag that is not valid BCP-47.
     * Control app only.
     */
    setPreference(preference: DisplayLanguagePreference): Promise<void>;
    /**
     * Follow the choice, not what it resolves to: a system locale change under
     * `'auto'` moves the language without moving the preference. Starts with
     * the current value and returns an unsubscribe; that first callback runs
     * synchronously, before `watchPreference` returns. Control app only.
     */
    watchPreference(callback: (preference: DisplayLanguagePreference) => void, options?: SignalOptions): () => void;
}"###;

        type ResolvedAppearance = r###"'light' | 'dark'"###;
        type VisibilityPreference = r###"'auto' | 'hidden'"###;
        type TabBarVisibilityPreference = r###"'auto' | 'visible' | 'hidden'"###;

        type NavigationBarStylePatch = r###"{
    backgroundColor?: string | null;
    foregroundColor?: string | null;
    dividerColor?: string | null;
}"###;

        type NavigationBarPatch = r###"{
    title?: string | null;
    homeButton?: VisibilityPreference;
    style?: NavigationBarStylePatch | null;
}"###;

        type TabBarItemPatch = r###"{
    /** Position of the item to change, from 0. */
    index: number;
    /** Label text. */
    text?: string | null;
    /**
     * Package-relative path, or an `lx://temp` / `lx://usercache` /
     * `lx://userdata` path from a LingXia file API. Network URLs are rejected —
     * download first (`lx.network.download`) and pass the returned path.
     *
     * SVG (and SF Symbols) are template-tinted with `foregroundColor` /
     * `selectedForegroundColor`. Raster files (PNG/JPEG/WebP) keep their own
     * colour so a brand mark is not flattened into a solid square.
     */
    iconPath?: string | null;
    /** Text in a badge on the item; `null` removes it. */
    badge?: string | null;
    /** Show a dot without text. */
    redDot?: boolean;
}"###;

        /// Patch for `lx.app.tabBar.update()`. Items, badges, red dots, and
        /// visibility only — a `style` field is rejected. Colors stay in
        /// static `lxapp.json` `tabBar.style`. `backgroundColor` is
        /// mobile-only; the desktop sidebar follows the host
        /// `lingxia.yaml` theme.
        type TabBarPatch = r###"{
    /** Show or hide the bar. */
    visibility?: TabBarVisibilityPreference;
    /** Per-item changes. */
    items?: readonly TabBarItemPatch[];
}"###;

        /// One app-declared action shown in the host-provided More affordance.
        /// Mobile hosts render these in the capsule sheet; desktop hosts render
        /// them in the lxapp context menu. The native menu is fully dismissed
        /// before `onClick` runs.
        type MoreAction = r###"{
    /** Bundled resource path or an app-accessible local `lx://` path. */
    icon: string;
    /** Visible action label. */
    label: string;
    /** Runs when the person picks it, after the menu has closed. */
    onClick: () => void | Promise<void>;
}"###;

        type PickDirectoryOptions = r###"{
    /**
     * The directory the dialog opens in; the platform's choice when absent.
     * @platform Android, HarmonyOS phones and tablets: only a directory in lxapp storage, which opens the in-app chooser.
     */
    defaultPath?: string;
}"###;

        /// One file the user picked. `path` is opaque: never parse it; pass it
        /// to LingXia APIs (`lx.fs`, `lx.openFile`, `lx.share`, uploads). It is
        /// read-only and valid for this session only; `lx.fs.copy` it to keep
        /// the file. Whether the host copied the file is host-specific: desktop
        /// grants the live original, iOS hands over a copy, Android and
        /// HarmonyOS a reference to the original.
        ///
        /// `name` is one path component and always has an extension; `type` is
        /// derived from that extension, so both are the same on every platform
        /// (`application/octet-stream` when unknown). A capture with no name of
        /// its own is named `IMG_YYYYMMDD_HHMMSS.jpg` / `VID_…` (UTC). `name` is
        /// display-only and not unique.
        type PickedFile = r###"{
    path: string;
    name: string;
    /** Bytes. */
    size: number;
    /** MIME type, e.g. `application/pdf`, `image/jpeg`. */
    type: string;
}"###;

        /// A picked directory. `path` is a read-only root: `lx.fs.exists`,
        /// `stat`, `readDir` and `file()` reads, and `lx.fs.copy` as a source,
        /// also take `<path>/<relative>` with each segment percent-encoded.
        /// Empty, `.` or `..` segments, `:`, a backslash, and any path that leaves the
        /// directory through a symlink do not resolve; writes are rejected.
        type PickedDirectory = r###"{
    /** Opaque read-only root; pass it to `lx.fs`. */
    path: string;
    /** The directory's own name, for display. */
    name: string;
}"###;

        /// Result of `lx.pick.directory`.
        type PickDirectoryResult = r###"{
    /** A directory was picked. */
    status: 'ok';
    /** The picked directory. */
    directory: PickedDirectory;
} | CanceledResult"###;

        type PickFileOptions = r###"{
    /** Optional file filters. Empty or omitted means all file types. */
    filters?: FileDialogFilter[];
    /**
     * Initial directory the dialog opens in.
     *
     * When this resolves to an app-local directory, LingXia may use its internal
     * file picker. When omitted, the platform system file picker is used.
     */
    defaultPath?: string;
}"###;

        /// Result of `lx.pick.file`: exactly one file.
        type PickFileResult = r###"{
    /** A file was picked. */
    status: 'ok';
    /** The picked file, alone in a list. */
    files: [PickedFile];
} | CanceledResult"###;

        /// Result of `lx.pick.files` and `lx.pick.media`: at least one file.
        /// From `lx.pick.media` on mobile, each file is a copy in the lxapp
        /// temp directory, counted against its quota.
        type PickFilesResult = r###"{
    /** At least one file was picked. */
    status: 'ok';
    /** The picked files, in the order chosen. */
    files: [PickedFile, ...PickedFile[]];
} | CanceledResult"###;

        type PickMediaOptions = r###"{
    /**
     * Most files to return. Where the system picker cannot enforce it
     * (desktop), files beyond it are dropped from the result.
     * @default 9
     */
    count?: number;
    /**
     * Kinds to offer. The camera captures one kind, so with `sourceType`
     * camera name a single entry (macOS excepted).
     * @default ['image', 'video']
     */
    mediaType?: ('image' | 'video')[];
    /**
     * Where media comes from; with both, the person chooses first.
     * @platform Windows: no camera; `camera` is dropped from a pair and a camera-only request rejects.
     * @default ['album']
     */
    sourceType?: ('album' | 'camera')[];
    /**
     * Camera facing.
     * @default 'back'
     */
    camera?: 'back' | 'front';
    /** Longest video the camera records, in seconds. */
    maxDurationSeconds?: number;
}"###;

        /// Representations the runtime can round-trip. Closed union.
        type ClipboardType = r###"'text' | 'image'"###;

        /// One clipboard write. Every host accepts both: `image` takes a PNG or
        /// JPEG file and re-encodes it as the platform's native image format.
        type ClipboardWriteItem = r###"{
    /** Write text. */
    type: 'text';
    /** Unicode text, at most 1 MiB. */
    text: string;
} | {
    /** Write an image. */
    type: 'image';
    /**
     * Managed `lx://` path, or a path `lx.pick` returned — the same file
     * rules as `lx.share`. Rejects
     * `E_INVALID_ARG` when the file is not a decodable image.
     */
    filePath: string;
}"###;

        type ClipboardReadOptions = r###"{
    /** Omit to receive every representation the host can surface. */
    type?: ClipboardType;
}"###;

        /// Result of `lx.clipboard.readText`. Branch on `status`.
        /// A copied empty string is `{ status: 'ok', text: '' }`;
        /// an image-only clipboard is `{ status: 'empty' }`.
        type ClipboardTextResult = r###"{
    /** The clipboard holds no text. */
    status: 'empty';
} | {
    /** Text was read. */
    status: 'ok';
    /** The text. */
    text: string;
} | CanceledResult"###;

        /// Result of `lx.clipboard.read`. Omit `type` to receive every
        /// representation this host can surface. A requested type that is
        /// absent is `{ status: 'empty' }`, not an error.
        type ClipboardReadResult = r###"{
    /** The clipboard holds nothing of the requested type. */
    status: 'empty';
} | {
    /** Something was read. */
    status: 'ok';
    /** One entry per representation. */
    items: ClipboardItem[];
} | CanceledResult"###;

        type ClipboardItem = r###"{
    /** Plain text. */
    type: 'text';
    /** The text. */
    text: string;
} | {
    /** An image. */
    type: 'image';
    /**
     * Temporary `lx://temp` PNG, session-scoped and auto-cleaned. Move or
     * copy it with `lx.fs` if you need to keep it.
     */
    filePath: string;
}"###;


        /// How `lx.media.compressImage` re-encodes the image.
        type CompressImageOptions = r###"{
    /**
     * JPEG quality, an integer from 0 to 100; values outside are clamped.
     * @default 80
     */
    quality?: number;
    /** Largest output width in pixels; the aspect ratio is kept. */
    maxWidth?: number;
    /** Largest output height in pixels; the aspect ratio is kept. */
    maxHeight?: number;
}"###;

        /// The image `lx.media.compressImage` wrote.
        type CompressImageResult = r###"{
    /** The compressed JPEG, a temporary `lx://` path. */
    path: string;
}"###;

        /// Where `lx.media.compressVideo` writes and how hard it compresses.
        type CompressVideoOptions = r###"{
    /** Aborting it stops the compression. */
    signal?: AbortSignal;
    /** Where to write it: an `lx://userdata`, `lx://usercache` or `lx://temp` path, or a path relative to `lx://userdata`. A temporary file when omitted. */
    to?: string;
} & (
    | {
    /**
     * Compression quality preset.
     * Mutually exclusive with `bitrate`, `fps`, and `resolution`.
     */
    quality: VideoCompressQuality;
    bitrate?: never;
    fps?: never;
    resolution?: never;
}
    | {
    quality?: never;
    /**
     * Preferred target video bitrate in bits per second, as `VideoInfo.bitrate`.
     * May be adjusted or ignored by platform codec/runtime limitations.
     */
    bitrate?: number;
    /**
     * Preferred target frame rate in fps.
     * Some platforms may ignore this option.
     */
    fps?: number;
    /**
     * Target resolution scale ratio relative to source size, in range `(0, 1]`.
     * May be approximated or ignored by platform transcoder capabilities.
     */
    resolution?: number;
}
)"###;

        type CompressVideoResult = r###"{
    /** The compressed video, an `lx://` path. */
    path: string;
    /** Width in pixels. */
    width: number;
    /** Height in pixels. */
    height: number;
    /** Duration in milliseconds. */
    durationMs: number;
    /**
     * Output file size in bytes.
     * Could be close to source size when platform falls back to source content.
     */
    size: number;
    /** MIME type, e.g. `video/mp4`. */
    type: string;
}"###;

        /// Handle returned by `lx.media.compressVideo`. Its progress counts the
        /// fraction done out of a `total` of 1.
        type CompressVideoTask = r###"CancelableTask<CompressVideoResult>"###;

        type ConnectWifiOptions = r###"{
    /** Network name. */
    ssid: string;
    /** Omit for an open network. */
    password?: string;
}"###;

        /// Where `lx.fs.copy` puts the copy.
        type FsCopyOptions = r###"{
    /** The destination file path. */
    to: ManagedPath;
    /**
     * Replace a file already at `to`.
     * @default false
     */
    overwrite?: boolean;
}"###;

        type DeviceOrientation = r###""portrait" | "landscape""###;

        type DeviceOrientationChangeEvent = r###"{
    value: DeviceOrientation;
}"###;

        type DownloadOptionsBase = r###"{
    /**
     * Optional request headers.
     * Restricted headers such as `Referer` are ignored by the runtime.
     */
    headers?: Record<string, string>;
    /** Request timeout in milliseconds. */
    timeoutMs?: number;
    /** Optional abort signal. */
    signal?: AbortSignal;
}"###;

        type DownloadResult = r###"AppDownloadResult | DownloadsDownloadResult"###;

        type DownloadsDownloadOptions = r###"DownloadOptionsBase & {
    /** The user's Downloads folder. Needs the host's downloads grant. */
    to: 'downloads';
    /** Filename hint; the host picks a free name next to it. */
    suggestedName?: string;
}"###;

        type DownloadsDownloadResult = r###"{
    /** Where the file landed in the user's Downloads folder. */
    uri: SystemDownloadsPath;
    /** The Downloads folder. */
    storage: 'downloads';
    /** MIME type from the response, when it named one. */
    mimeType?: string;
    /** File size in bytes. */
    sizeBytes: number;
}"###;

        /// Which frame `lx.media.videoThumbnail` takes and where it writes it.
        type VideoThumbnailOptions = r###"{
    /**
     * The frame's position from the start, in milliseconds.
     * @default 0
     */
    positionMs?: number;
    /** Where to write it: an `lx://userdata`, `lx://usercache` or `lx://temp` path, or a path relative to `lx://userdata`. A temporary file when omitted. */
    to?: string;
    /** Largest output width in pixels; the aspect ratio is kept, nothing is cropped. */
    maxWidth?: number;
    /** Largest output height in pixels; the aspect ratio is kept, nothing is cropped. */
    maxHeight?: number;
    /**
     * JPEG quality, an integer from 0 to 100; values outside are clamped.
     * @default 80
     */
    quality?: number;
}"###;

        /// The image `lx.media.videoThumbnail` wrote.
        type VideoThumbnailResult = r###"{
    /** The thumbnail: the `to` path when it was an `lx://` path, else the `lx://` path written. */
    path: string;
    /**
     * Output image width in pixels.
     */
    width: number;
    /**
     * Output image height in pixels.
     */
    height: number;
    /**
     * Output MIME type, usually `image/jpeg`.
     */
    type: string;
}"###;

        type FileDialogFilter = r###"{
    /** Optional label shown in the native dialog (desktop only). */
    name?: string;
    /** Allowed extensions without dots, e.g. ['pdf', 'txt']. */
    extensions: string[];
}"###;

        type GetLocationOptions = r###"{
    /**
     * Coordinate system of the result.
     * @default 'wgs84'
     */
    coordinateSystem?: 'wgs84' | 'gcj02';
    /** Include `altitude` in the result. */
    altitude?: boolean;
    /** Ask for a precise fix; slower and costs more power. */
    isHighAccuracy?: boolean;
    /**
     * How long to wait for a fix before failing, in milliseconds.
     * @default 10000
     */
    timeoutMs?: number;
}"###;

        /// Build-time deployment environment of the host app (`dev` | `prod`).
        ///
        /// Surfaced via {@link HostAppApi.env}. Taken from the `env` field in
        /// the generated `app.json`. Missing `env` is treated as `'prod'`.
        ///
        /// This is the immutable host build axis: package-id suffix, publish
        /// token, signed App Link entitlements, and self-update channel.
        /// In-app App Link hosts follow {@link HostAppApi.getServiceEnv} on
        /// the next launch. The mutable service environment is
        /// {@link HostAppApi.toggleServiceEnv}. It is **not** the
        /// lxapp publish channel (`LxAppReleaseType`:
        /// `'release' | 'draft'`). Default channel is `release` and can be
        /// overridden when opening an lxapp.
        ///
        type HostAppEnv = r###"'dev' | 'prod'"###;

        type HostAppUpdateApplyStage = r###"'download' | 'install'"###;

        type HostAppUpdateCheckResult = r###"{
    /** No newer version is published. */
    hasUpdate: false;
    update?: never;
} | {
    /** A newer version is published. */
    hasUpdate: true;
    /** The newer version. */
    update: HostAppUpdateInfo;
}"###;

        type HostAppUpdateInfo = r###"{
    version: string;
    size?: number;
    releaseNotes?: string[];
    /**
     * How this update is applied. `store` opens the platform marketplace;
     * `direct` downloads and self-installs. `lx.supports('app.selfUpdate')`
     * is true only for `direct`.
     */
    channel: 'direct' | 'store';
    /**
     * Apply this checked update.
     *
     * `apply()` is single-use for this update object. It also claims custom
     * host updates for the rest of this process, same as
     * {@link HostAppApi.claimCustomUpdate}.
     *
     * Await `task.result` when progress is not needed, or iterate the task
     * to render it.
     *
     * On `direct`, downloads and hands off install. On `store`, opens the
     * platform store listing (no package is downloaded) and resolves
     * `storeOpened`; whether the user then updates is not reported.
     */
    apply(): HostAppUpdateTask;
}"###;

        type HostAppUpdateResult = r###"{
    /** `storeOpened` on a `store` channel: the listing opened, nothing was installed. */
    state: 'installRequested' | 'storeOpened';
}"###;

        /// Handle returned by `apply()`. Its progress is the package download;
        /// `total` is present when the update named its size.
        type HostAppUpdateTask = r###"Task<HostAppUpdateResult>"###;

        type KeyEvent = r###"{
    /** Key value following W3C naming (e.g. "Enter", "ArrowLeft", "a") */
    key: string;
    /** Physical key code (e.g. "ENTER", "DPAD_LEFT") */
    code: string;
    altKey?: boolean;
    ctrlKey?: boolean;
    shiftKey?: boolean;
    metaKey?: boolean;
    repeat?: boolean;
}"###;

        type KeyEventCallback = r###"(event: KeyEvent) => void"###;

        type LxAppReleaseType = r###"'release' | 'draft'"###;

        type MediaObjectFit = r###"'cover' | 'contain' | 'fill' | 'fit'"###;

        type MediaRotation = r###"0 | 90 | 180 | 270"###;

        type FsMkdirOptions = r###"{
    /**
     * Create missing parents too; no error when it exists.
     * @default false
     */
    recursive?: boolean;
}"###;

        /// A tab root of this lxapp.
        type TabName = r###"keyof LxAppPages extends never ? string : keyof LxAppTabs & string"###;

        type TabTarget = r###"{
    /** A tab root of this lxapp. */
    tab: TabName;
    page?: never;
    app?: never;
    url?: never;
    params?: never;
}"###;

        /// Another lxapp, optionally at one of its pages.
        type AppTarget = r###"{
    /** The lxapp's id. */
    app: string;
    /** A page name of that lxapp; omit for its initial page. */
    page?: ExternalPageName;
    tab?: never;
    url?: never;
    /** The page's params. */
    params?: PageQuery;
    /** Lxapp publish channel. Defaults to `release`. */
    channel?: LxAppReleaseType;
    /** Install this version first if it is newer than the installed one; older versions are refused. */
    version?: string;
}"###;

        /// A web page, opened in the in-app browser. `lx.openExternal` hands a URL to the system.
        type UrlTarget = r###"{
    /** An `https` URL. */
    url: string;
    page?: never;
    tab?: never;
    app?: never;
    params?: never;
}"###;

        type NavTarget = r###"PageTarget | TabTarget | AppTarget | UrlTarget"###;


        type NetworkChangeCallback = r###"(info: NetworkInfo) => void"###;

        type NetworkInfo = r###"{
    /** Any network is usable. */
    isConnected: boolean;
    /** `'none'` whenever `isConnected` is false. */
    networkType: NetworkType;
    /** The primary IPv4 address, at most one; empty when not connected. */
    ipv4: string[];
    /** The primary IPv6 address, at most one; empty when not connected. */
    ipv6: string[];
}"###;

        type NetworkType = r###"'none' | 'unknown' | 'wifi' | '2g' | '3g' | '4g' | '5g' | 'ethernet'"###;

        /// How `lx.openFile` shows the file.
        type OpenFileOptions = r###"{
    /**
     * `auto` previews in the app and falls back to another app; `review` only
     * previews in the app and rejects when it cannot; `external` hands the file to another app.
     * @default 'auto'
     */
    mode?: 'auto' | 'review' | 'external';
}"###;

        /// Native interaction supplied by the host around page content.
        type SurfaceInteraction = r###"{
    /**
     * Show the standard circular close button.
     * @default false
     */
    closeButton?: boolean;
    /**
     * Whether a tap outside closes it.
     * @default 'tapOutside' for floats, 'manual' for windows
     */
    dismiss?: 'tapOutside' | 'manual';
    /**
     * Block interaction with content below.
     * @default false
     */
    modal?: boolean;
}"###;

        /// Built-in browser product page. Opening one requires
        /// `capabilities.browser` and is restricted to the native-assigned Control app.
        type BuiltinShellPage = r###"'downloads'"###;

        type OverlaySurfaceSize = r###"{
    /** Width hint. */
    width?: OverlaySurfaceSizeValue;
    /** Height hint. */
    height?: OverlaySurfaceSizeValue;
}"###;

        /// Size hint for an overlay surface (aside / float).
        ///
        /// - number: absolute px, must be > 0
        /// - `${number}%`: percentage of the container, 0 < N ≤ 100
        ///
        type OverlaySurfaceSizeValue = r###"number | `${number}%`"###;

        /// What `onLoad` receives: each param with the type it was passed with.
        /// Validate a query from outside (an app link, a URL): it is usually
        /// strings.
        type PageLoadOptions = r###"{
    [key: string]: JsonValue | undefined;
}"###;

        /// A pushed page's line to the page that opened it (`page.opener`).
        type PageMessagePort = r###"{
    /**
     * Delivers `message` to the other side's `onMessage` handlers, at once and
     * as the same value (both sides share this Logic context; nothing is
     * copied). Until the other side first subscribes, messages wait for it.
     */
    send(message: unknown): void;
    /** Receives what the other side sends. Returns a function that removes the handler. */
    onMessage(handler: (message: unknown) => void, options?: SignalOptions): () => void;
}"###;

        /// A pushed or surface page's outcome. `canceled` means it left without a
        /// result: `user` (Back, a dismiss, a close), `opener` (the opener went
        /// away, or a newer result promise replaced this one), `reclaimed` (a
        /// hidden surface page unloaded under memory pressure), `app_closed`.
        type PageResult = r###"
    | {
        /** The page gave a result. */
        status: 'ok';
        /** What it set by `setResult` or `close(value)`. */
        value: JsonValue;
    }
    | {
        /** It left without a result. */
        status: 'canceled';
        /** Who ended it: the person, the opener, memory pressure, or the app closing. */
        reason: 'user' | 'opener' | 'reclaimed' | 'app_closed';
    }"###;

        type PageQuery = r###"Record<string, PageQueryValue>"###;

        /// A page param: any JSON value. `null` and `undefined` mean absent. The
        /// whole set of non-string params is limited to 8 KB as JSON.
        type PageQueryValue = r###"JsonValue | undefined"###;

        type PreviewMediaAdvance = r###"'manual' | 'next' | 'loop'"###;

        /// One change-stream event / the `current` snapshot.
        type PreviewMediaChange = r###"{
    /** Position of the shown item. */
    index: number;
    /** The shown item. */
    source: PreviewMediaShownSource;
}"###;

        type PreviewMediaCloseReason = r###"'manual' | 'completed' | 'interrupted' | 'error'"###;

        /// A media session. `presented` distinguishes a rendered first frame from
        /// an early close, cancellation, or failure. `completed` reports closure;
        /// native playback errors may report reason `error`, while request failures reject.
        type PreviewMediaHandle = r###"{
    /** Settles once the first item is on screen, or with why it never was. */
    readonly presented: Promise<{
        /** The first item is on screen. */
        status: 'presented';
    } | {
        /** It never showed. */
        status: 'notPresented';
        /** Dismissed first, failed to load, or closed by `close()`. */
        reason: 'canceled' | 'failed' | 'closed';
    }>;
    /** The item on screen now. */
    readonly current: PreviewMediaChange;
    /** Fires when the shown item changes; `current` holds the latest. Returns a function that removes the listener. */
    onChange(listener: (change: PreviewMediaChange) => void, options?: SignalOptions): () => void;
    /** Settles when the preview closes. */
    readonly completed: Promise<PreviewMediaResult>;
}"###;

        type PreviewMediaOptions = r###"string | PreviewMediaSingleOptions | PreviewMediaSequenceOptions"###;

        type PreviewMediaResult = r###"{
    /**
     * Why the preview session finished.
     */
    reason: PreviewMediaCloseReason;
    /**
     * Index of the item on screen when the session closed.
     */
    index: number;
    /**
     * The item on screen when the session closed — "what the user just
     * viewed/played", without mapping `index` back yourself.
     */
    source: PreviewMediaShownSource;
}"###;

        type PreviewMediaSequenceOptions = r###"{
    /**
     * Preview list. Supports images, videos, or a mixed queue.
     */
    sources: PreviewMediaSource[];
    /**
     * Initial item index in `sources`.
     * Must be an integer.
     * Out-of-range values are clamped by runtime.
     * @default 0
     */
    startIndex?: number;
    /**
     * Auto behavior for the preview session.
     *
     * - `manual`: never auto-advance
     * - `next`: advance to the next item; if already on the last item, close the session
     * - `loop`: advance to the next item; if already on the last item, wrap to the first item
     *
     * Default: `manual`
     */
    advance?: PreviewMediaAdvance;
    /**
     * Optional cancellation signal for the preview request.
     *
     * Aborting rejects the returned promise with a cancellation error and requests the active
     * native preview session to close immediately.
     */
    signal?: AbortSignal;
    /**
     * Show the `current/total` indicator at the top.
     * @default true for more than one item
     */
    showIndexIndicator?: boolean;
}"###;

        /// The item the user is (or was) looking at, handed back as the caller
        /// described it — `path` is returned verbatim, so it can be matched against
        /// the caller's own data without re-indexing an array.
        ///
        type PreviewMediaShownSource = r###"{
    /** The path exactly as passed in the request. */
    path: string;
    /** Resolved media kind (after extension inference when `type` was omitted). */
    type: 'image' | 'video';
}"###;

        type PreviewMediaSingleOptions = r###"PreviewMediaSource & {
    /**
     * Auto behavior for the preview session.
     *
     * - `manual`: never auto-advance
     * - `next`: advance to the next item; if already on the last item, close the session
     * - `loop`: advance to the next item; if already on the last item, wrap to the first item
     *
     * Default: `manual`
     */
    advance?: PreviewMediaAdvance;
    /**
     * Optional cancellation signal for the preview request.
     *
     * Aborting rejects the returned promise with a cancellation error and requests the active
     * native preview session to close immediately.
     */
    signal?: AbortSignal;
    /**
     * Show the `current/total` indicator at the top.
     * @default true for more than one item
     */
    showIndexIndicator?: boolean;
}"###;

        type PreviewMediaSource = r###"{
    /**
     * Media source path.
     *
     * Accepts an `https://` (or `http://`) URL for a remote image or video —
     * the host loads it directly, so no prior `lx.network.download` is required,
     * and its domain must be permitted by the app's network policy — an
     * `lx://` path (for example `lx://usercache/...`), or a sandbox-local path
     * that can be resolved by runtime access rules.
    */
    path: string;
    /** Image or video; guessed from the path when absent. */
    type?: 'image' | 'video';
    /**
     * Clockwise rotation in degrees.
     * @default the rotation the media's metadata records
     */
    rotation?: MediaRotation;
    /**
     * How a video fits the screen.
     * @default 'contain'
     */
    objectFit?: MediaObjectFit;
    /**
     * Display duration in milliseconds.
     * Effective when preview `advance` is not `manual`.
     */
    durationMs?: number;
}"###;



        type FsRemoveOptions = r###"{
    /**
     * Remove a directory and everything in it.
     * @default false
     */
    recursive?: boolean;
}"###;

        /// Where `lx.fs.rename` moves the file or directory.
        type FsRenameOptions = r###"{
    /** The new path. */
    to: ManagedPath;
    /**
     * Replace a file already at `to`; a directory is never replaced.
     * @default false
     */
    overwrite?: boolean;
}"###;


        type PickCodeOptions = r###"{
    /**
     * Read only from the camera; `false` also lets the person pick a picture.
     * @default true
     */
    onlyFromCamera?: boolean;
    /** Symbologies to read; every one when absent. */
    formats?: ('barCode' | 'qrCode' | 'datamatrix' | 'pdf417')[];
}"###;

        /// Result of `lx.pick.code`. `format` is the symbology that was read.
        type PickCodeResult = r###"{
    /** A code was read. */
    status: 'ok';
    /** Its decoded text. */
    value: string;
    /** The symbology, e.g. `qrCode`. */
    format: string;
} | CanceledResult"###;

        /// Share images, PDFs, or other files.
        ///
        type ShareFilesOptions = r###"ShareTitleOptions & {
    /**
     * File paths returned by LingXia APIs to share. Images, PDFs, and other
     * documents are all represented as file paths.
     *
     * Use `lx.pick.files` for system files and `lx.pick.media` for picked media;
     * pass the returned path here without parsing it.
     * Some platforms or receivers may limit multi-file shares. Share files one
     * at a time when targeting those receivers.
     *
     * `files` and `page` are mutually exclusive.
     * `text` is intentionally not supported for file shares because system
     * receivers handle text+attachment inconsistently.
     */
    files: string[];
    page?: never;
    text?: never;
}"###;

        type ShareOptions = r###"ShareTextOptions | SharePageOptions | ShareFilesOptions"###;

        type SharePage = r###"/**
 * Share the current page.
 */
true
/**
 * Share the current page with query.
 */
 | {
    /**
     * Query appended to the current page. Query belongs to the page target and is
     * encoded into the AppLink URL.
     */
    query?: ShareQuery;
}"###;

        /// Share the current page as an AppLink.
        ///
        type SharePageOptions = r###"ShareTextBaseOptions & {
    /**
     * Share the current page. The runtime uses the current appId and page path
     * implicitly and shares it through the host AppLink configuration.
     *
     * Rejects when the host app has no `appLinks.hosts` configuration because
     * receivers would not be able to open the shared page.
     *
     * `title` and `text` are presentation hints. Platforms and receivers may
     * ignore them; on iOS the URL is shared by itself so receivers can render it
     * as a webpage card when they support that.
     *
     * `page` and `files` are mutually exclusive.
     */
    page: SharePage;
    files?: never;
}"###;

        type ShareQuery = r###"Record<string, string | number | boolean>"###;

        type ShareResult = r###"{
    /**
     * What the share sheet reported. Not part of the `canceled` family: some
     * platforms only observe that the system UI opened and closed, so the
     * unknown case is stated rather than hidden in a missing boolean that
     * every call site would read as "not shared".
     */
    outcome: 'completed' | 'dismissed' | 'unknown';
}"###;

        type ShareTextBaseOptions = r###"ShareTitleOptions & {
    /**
     * Share text body.
     */
    text?: string;
}"###;

        /// Share title/text only. Receiver support is platform/app dependent; some
        /// share extensions may reject text-only shares.
        ///
        type ShareTextOptions = r###"ShareTextBaseOptions & {
    page?: never;
    files?: never;
}"###;

        type ShareTitleOptions = r###"{
    /**
     * Share title.
     */
    title?: string;
}"###;

        type ActionSheetOptions = r###"{
    /** The choices, in order. */
    items: readonly {
        /** What the result's `id` reports when this one is picked. */
        id: string;
        /** Text shown. */
        label: string;
    }[];
    /** `'danger'` draws the items as destructive. */
    tone?: 'default' | 'danger';
}"###;

        /// An acknowledgement dialog has no cancel button.
        type AlertOptions = r###"{
    /** The question or statement. */
    title: string;
    /** Detail below the title. */
    message?: string;
    /** Label of the only button. */
    confirmText?: string;
}"###;
        /// A confirmation dialog always permits declining.
        type ConfirmOptions = r###"{
    /** The question. */
    title: string;
    /** Detail below the title. */
    message?: string;
    /** `'danger'` draws the confirm button as destructive. */
    tone?: 'default' | 'danger';
    /** Label of the accepting button. */
    confirmText?: string;
    /** Label of the declining button. */
    cancelText?: string;
}"###;
        /// The toast `lx.ui.toast` returned.
        type ToastHandle = r###"{
    /**
     * Dismiss this toast only; harmless after a newer toast replaces it.
     * Before it shows, this cancels it.
     */
    dismiss(): void;
}"###;

        /// How `lx.ui.toast` looks and how long it stays.
        type ToastOptions = r###"{
    /**
     * `success` and `danger` add a matching icon above the text.
     * @default 'default'
     */
    tone?: 'default' | 'success' | 'danger';
    /** Show a spinner above the text instead of a tone icon. */
    loading?: boolean;
    /**
     * How long it stays, in milliseconds.
     * @default 1500
     */
    durationMs?: number;
    /**
     * Where it appears.
     * @default 'center'
     */
    position?: 'top' | 'center' | 'bottom';
}"###;

        /// Asynchronous persistent key-value storage backed by the lxapp
        /// database. Use `lx.fs` for path-based data.
        ///
        /// `get<T>()` is an unchecked assertion at the call site. Pin every
        /// key's shape once with `lx.storage<Schema>()` — that returns a
        /// `TypedStorage<Schema>` instead of this untyped handle.
        ///
        type Storage = r###"{
    /**
     * Reads a stored value. `T` is an unchecked assertion about the stored
     * shape, exactly like a `JSON.parse` boundary; a missing key resolves
     * `undefined`, which a stored `null` never does.
     */
    get<T = unknown>(key: string, decode?: (value: unknown) => T): Promise<T | undefined>;
    set(key: string, value: unknown): Promise<void>;
    /**
     * Resolves whether an exact key exists, without reading its value. Prefer
     * it over comparing `get` against `undefined`: presence is a key lookup,
     * while `get` also reads and deserializes the stored value.
     */
    has(key: string): Promise<boolean>;
    delete(key: string): Promise<void>;
    clear(): Promise<void>;
    /** Resolves every key, optionally filtered by prefix. */
    list(prefix?: string): Promise<string[]>;
    info(): Promise<StorageInfo>;
}"###;

        /// Current persistent-storage usage and configured limits.
        ///
        type StorageInfo = r###"{
    currentSize: number;
    limitSize: number;
    keyCount: number;
}"###;

        type StreamSourceOptions = r###"{
    provider: string;
    isLive: boolean;
    durationSeconds?: number;
    params?: Record<string, unknown>;
}"###;

        type SurfaceClosedEvent = r###"{
    reason: SurfaceCloseReason;
}"###;

        /// Where the content came from. The discriminant on every surface
        /// handle, so `AnySurface` narrows without a runtime `typeof` check.
        ///
        type SurfaceKind = r###"'page' | 'declared' | 'app' | 'tab' | 'builtin'"###;

        /// What the host actually produced. Reported by `realized`, which is
        /// how a caller reads the outcome of an ordered placement preference.
        ///
        type SurfacePlacement = r###"'main' | 'aside' | 'float' | 'window'"###;

        /// Window decoration. `system` is the standard title bar. `full`
        /// extends the page to the window edge while keeping the system
        /// minimize, maximize, resize, and drag affordances — the runtime owns
        /// a native drag strip across the top and publishes its height as
        /// `topInset` on the page-chrome snapshot, so a page that does nothing
        /// to opt in still cannot trap the user.
        ///
        type WindowChrome = r###"'system' | 'full'"###;

        /// Why a surface operation was refused. Carried on `data.code` of every
        /// `SurfaceError`, so no caller has to match on message text.
        ///
        type SurfaceErrorCode = r###"/** The placement cannot be realized by this host build. */
'unsupported_placement'
/** A privileged operation was called by an lxapp other than the native-assigned Control app. */
 | 'denied'
/** No such declared surface, lxapp, or builtin page. */
 | 'not_declared'
/** The arguments are malformed or combine options that cannot apply together. */
 | 'invalid_arg'
/** The target is already open in a role this call cannot change. */
 | 'already_open_other_role'
/** The role forbids this: a main cannot be hidden, and the stable
 * root main cannot be closed or leave `main`. */
 | 'role_locked'
/** The surface has been closed; the handle is detached. */
 | 'closed'
/** The host lacks a capability the request needs, such as an instantiable
 * native provider for a keyed surface. */
 | 'capability_missing'
/** The operation reached the host and failed there. */
 | 'failed'"###;

        /// A surface rejection. `code` is the host code: `E_SURFACE_CONFLICT`,
        /// `E_SURFACE_ROLE_LOCKED`, `E_SURFACE_CLOSED`, `E_CAPABILITY_MISSING`,
        /// `E_NOT_SUPPORTED`, `E_NOT_FOUND`, `E_INVALID_ARG`,
        /// `E_PERMISSION_DENIED`, or `E_INTERNAL`. `data.code` carries the finer
        /// `SurfaceErrorCode`; read it with `surfaceErrorCode(error)` and never
        /// parse the message.
        ///
        /// ```ts
        /// import { surfaceErrorCode } from '@lingxia/types/error';
        ///
        /// try {
        ///   await lx.surface.openPage('detail', { as: 'window' });
        /// } catch (error) {
        ///   if (surfaceErrorCode(error) !== 'unsupported_placement') throw error;
        /// }
        /// ```
        ///
        type SurfaceError = r###"Error & {
    readonly data?: { readonly code?: SurfaceErrorCode };
}"###;

        /// What every surface handle carries, whatever opened it.
        ///
        type SurfaceBase = r###"{
    /** Which kind of surface this is; narrows the handle. */
    readonly kind: SurfaceKind;
    /** The caller-supplied identity, when this surface was opened with one. */
    readonly key?: string;
    /**
     * The placement the host produced, which an ordered preference may narrow.
     * Live rather than a snapshot: a later open that moves the surface, or the
     * user dragging it, updates the handle you already hold.
     */
    readonly realized: SurfacePlacement;
    /** True until `close()` fires; afterwards the page instance is torn down. */
    readonly alive: boolean;
    /**
     * Last-known visibility, kept in sync with the native side. Safe to bind
     * into declarative UI; for event-driven updates use `onShow` / `onHide`.
     */
    readonly visible: boolean;
    /**
     * Destroy the surface. The stable root main cannot be closed. Repeated
     * calls after a successful close are idempotent.
     */
    close(): Promise<void>;
    /** Fires once when the surface closes, whoever closed it. Returns a function that removes the handler. */
    onClose(handler: (event: SurfaceClosedEvent) => void, options?: SignalOptions): () => void;
}"###;

        /// Surfaces the host can hide and restore without losing page state.
        ///
        type SurfaceShowable = r###"{
    /**
     * Restore a hidden surface. The page instance survived, so scroll
     * position, form input, and JS state come back with it. Idempotent.
     */
    show(): Promise<void>;
    /**
     * Hide without destroying. Main surfaces cannot be hidden and reject.
     * Idempotent.
     */
    hide(): Promise<void>;
    /** Fires on a real transition to visible, whichever side drove it. */
    onShow(handler: (event: SurfaceVisibilityEvent) => void, options?: SignalOptions): () => void;
    /** Fires on a real transition to hidden, whichever side drove it. */
    onHide(handler: (event: SurfaceVisibilityEvent) => void, options?: SignalOptions): () => void;
}"###;

        /// Two-way messaging, available when both sides are lxapp pages.
        ///
        type SurfaceMessaging = r###"{
    /**
     * Sends to the other side: from the opener to the opened page, from the
     * page (`page.surface`) to its opener. Delivered at once and uncopied to
     * the other side's `onMessage` handlers; until it first subscribes,
     * messages wait for it.
     */
    send(message: unknown): void;
    /** Receives what the other side sends. Returns a function that removes the handler. */
    onMessage(handler: (message: unknown) => void, options?: SignalOptions): () => void;
}"###;


        /// A surface declared by the host in `lingxia.yaml`. A declaration that
        /// shows an lxapp is that app's `AppSurface` instead.
        ///
        type DeclaredSurface = r###"SurfaceBase & SurfaceShowable & {
    /** A surface `lingxia.yaml` declares. */
    readonly kind: 'declared';
}"###;

        /// Another lxapp composed into a shell slot.
        ///
        type AppSurface = r###"SurfaceBase & SurfaceShowable & {
    /** Another lxapp. */
    readonly kind: 'app';
    /** Where it is shown. */
    readonly realized: 'main' | 'aside';
}"###;

        /// A host builtin page such as downloads. The shell owns
        /// its lifetime and its visibility, so this handle reports identity:
        /// there is no `show`, `hide`, `close`, or `onClose`.
        ///
        type BuiltinSurface = r###"Omit<SurfaceBase, 'close' | 'onClose'> & {
    /** A host builtin page. */
    readonly kind: 'builtin';
}"###;

        /// External content in the in-app browser.
        ///
        type TabSurface = r###"(SurfaceBase & {
    /** An in-app browser tab. */
    readonly kind: 'tab';
    /** Where the browser is shown. */
    readonly realized: 'main' | 'aside';
    /** The handle is one tab, which it can activate and close. */
    readonly scope: 'tab';
    /** Brings this tab to the front. Rejects with `closed` once the tab is gone. */
    activate(): Promise<void>;
}) | (Omit<SurfaceBase, 'close' | 'onClose'> & {
    /** In-app browser tabs. */
    readonly kind: 'tab';
    /** Where the browser is shown. */
    readonly realized: 'main' | 'aside';
    /** The handle is the browser's tab group; the shell owns its lifetime. */
    readonly scope: 'group';
})"###;

        /// Every surface handle, narrowable by `kind`.
        ///
        type AnySurface = r###"PageSurface | DeclaredSurface | AppSurface | TabSurface | BuiltinSurface"###;

        type OpenPageShared = r###"{
    /**
     * A float accepts a percentage; a window is in logical pixels and ignores
     * one. Both live here rather than in two option types, because `as` may be
     * an ordered preference and the realized placement is not known up front.
     */
    size?: OverlaySurfaceSize;
    /** Whether the content behind a float still takes input. */
    interaction?: SurfaceInteraction;
    /** Caller-owned identity, for `lx.surface.get(key)` later. */
    key?: string;
}"###;

        /// `as` picks the shape. A float anchors and carries no decoration; a
        /// window is decorated and does not anchor. The runtime rejects the
        /// wrong pairing either way, so the type says it first — except with an
        /// ordered preference, where the realized placement is not known up
        /// front and both stay open.
        type OpenPageOptions = r###"(OpenPageShared & {
    /** The default. Rejects when the host cannot float. */
    as?: 'float';
    /** Where the float anchors. */
    position?: SurfaceFloatPosition;
    chrome?: never;
}) | (OpenPageShared & {
    /** A separate desktop window. Rejects when the host cannot make one. */
    as: 'window';
    /** Window decoration. */
    chrome?: WindowChrome;
    position?: never;
}) | (OpenPageShared & {
    /**
     * An ordered preference: the first placement the host can realize wins,
     * and `realized` reports which.
     */
    as: readonly ('float' | 'window')[];
    chrome?: WindowChrome;
    position?: SurfaceFloatPosition;
})"###;

        type OpenUrlOptions = r###"{
    /**
     * `main` opens a browser tab in the main area; `aside` docks the browser
     * beside the main. Defaults to `'main'`.
     */
    as?: 'main' | 'aside' | readonly ('main' | 'aside')[];
    /** Preferred docking side when the realized placement is an aside. */
    edge?: SurfaceEdge;
    /** Aside width or float size. */
    size?: OverlaySurfaceSize;
    /** Stable identity for `lx.surface.get(key)`. */
    key?: string;
}"###;

        /// Which instance `lx.shell.getDeclared` looks up.
        type GetDeclaredOptions = r###"{
    /** The instance key it was opened with; the unkeyed instance when absent. */
    key?: string;
}"###;

        /// Options of `lx.surface.openDeclared`. Each one changes shared shell
        /// composition, so only the Control app may pass any; other lxapps call
        /// it without options and get the declaration as the host authored it.
        ///
        type OpenDeclaredOptions = r###"{
    /**
     * Which instance of the declaration: a declaration can be opened more than
     * once, and a new key creates one. Find it again with
     * `lx.shell.getDeclared(id, { key })`. 1 to 128 UTF-8 bytes. Declarations
     * without instantiable native providers reject it with `capability_missing`.
     */
    key?: string;
    /**
     * Open with a role other than the declaration's, or move the live instance
     * to it. Must be realizable by the declared provider; a stable root rejects
     * anything but `main`.
     */
    as?: 'main' | 'aside' | 'float';
    /** Preferred docking side when the effective role is `aside`. */
    edge?: SurfaceEdge;
}"###;

        /// What `lx.shell.setAppDisplay` changes. Each key it carries is set;
        /// `undefined`, `null` or `''` restores the default, and a key left out
        /// keeps its value.
        type AppDisplayPatch = r###"{
    /** One line, 1 to 64 characters after trimming; blank restores the default. */
    name?: string | null;
    /**
     * A bundled path or `lx://` file: PNG, JPEG, WebP or SVG of at most 1 MiB,
     * raster images at most 4096 px on a side; SVG content is not validated.
     * Copied, so the source may go.
     */
    icon?: string | null;
}"###;

        /// Options of `lx.surface.openApp`. Control app only.
        type OpenAppOptions = r###"{
    /**
     * `main` occupies the primary content area; `aside` a companion region.
     * Omitted, a live app keeps its role and a closed one opens in main. A live
     * app asked for the other role moves there without restarting.
     */
    as?: 'main' | 'aside';
    /** Preferred docking side. Only meaningful with `as: 'aside'`. */
    edge?: SurfaceEdge;
    /**
     * Configured page name from the target lxapp's `lxapp.json`. Omit it to
     * open that app's initial page. Full page routes are not supported.
     */
    page?: ExternalPageName;
    /**
     * The page's params. When the lxapp is already open, the surface is
     * focused (and moved, if `as` names the other role), not reopened: `page`
     * and `params` reach its Logic as App `onShow` with
     * `reason: 'switch_back'`, `path` and `query`, and its page stack is kept.
     */
    params?: PageQuery;
    /**
     * Lxapp publish channel. Defaults to `release` in every service environment.
     */
    channel?: LxAppReleaseType;
    /** Install this version first if it is newer than the installed one; older versions are refused. */
    targetVersion?: string;
}"###;

        /// Surfaces beside or above the page stack, each with a handle.
        /// `openApp` and the options of `openDeclared` are the Control app's;
        /// other lxapps get a typed refusal.
        ///
        type SurfaceApi = r###"{
    /**
     * Open one of this lxapp's pages as a float or a window. Returns at once;
     * await `.opened` to wait for the page.
     * @throws {E_INVALID_ARG} Malformed options.
     * @throws {E_NOT_SUPPORTED} `as: 'window'` where the host has no windows.
     * @throws {E_NOT_FOUND} A page the app does not declare.
     */
    openPage<N extends ConfiguredPageName>(page: N, ...options: OpenPageArgs<N>): PageSurface<NavResult<N>, NavMessages<N>>;
    /** Open external content in the in-app browser. */
    openUrl(url: string, options?: OpenUrlOptions): Promise<TabSurface>;
    /**
     * Open another lxapp in main or a companion slot, focus it if it is open,
     * or move it to the role `as` names. Control app only.
     */
    openApp(appId: string, options?: OpenAppOptions): Promise<AppSurface>;
    /**
     * Open a surface the host declared in `lingxia.yaml`, with the placement
     * the declaration chose. `key`, `as` and `edge` (Control app only) open a
     * keyed instance or override the placement; `as` and `edge` also move a
     * live instance. A declaration that shows an lxapp resolves to that app's
     * `AppSurface`.
     */
    openDeclared(id: string, options?: OpenDeclaredOptions): Promise<DeclaredSurface | AppSurface>;
    /**
     * Find one of this lxapp's own live pages or browser tabs by the key passed
     * to `openPage` / `openUrl`. Declared surfaces and other lxapps are found
     * with `lx.shell.getDeclared` and `lx.shell.getApp`.
     */
    get(key: string): PageSurface | TabSurface | undefined;
}"###;

        type SurfaceCloseReason = r###"'user' | 'programmatic' | 'owner_closed' | 'app_closed' | 'failed'
/**
 * The host unloaded a hidden surface's page under memory pressure.
 * Treat as a normal close: the page instance is gone; further send /
 * show / hide calls will fail. The opener may immediately reopen if needed.
 */
 | 'reclaimed' | 'unknown'"###;

        /// `page.env`: the size class Logic acts on. Pixels and insets stay in
        /// the View's `env`.
        ///
        type PageEnvContext = r###"{
    /**
     * Whether the window currently has a dock region, so `as: 'aside'` docks
     * beside the main instead of being reprojected as an overlay. A capability
     * of the window, not where this lxapp is presented.
     */
    readonly canDock: boolean;
    /** compact (<600) / regular (≥600). Shell medium/expanded are not distinct here. */
    readonly sizeClass: 'compact' | 'regular';
}"###;

        /// Preferred docking side for an aside when the Host has room for a docked
        /// layout. `aside` selects the companion region; `edge` selects a side within
        /// it. Compact Hosts may reproject the same aside as a full-screen overlay.
        type SurfaceEdge = r###"'left' | 'right' | 'top' | 'bottom'"###;

        /// Where a float popup anchors (default `center`).
        type SurfaceFloatPosition = r###"'center' | 'top' | 'bottom' | 'left' | 'right'"###;

        /// Detail payload for `onShow` / `onHide` events. `source` identifies which
        /// Surface object initiated the visibility change so observers can
        /// distinguish self-driven transitions from peer-driven ones (e.g. an opener
        /// UI that wants to update its own button state only when the page side
        /// toggled visibility). `shell` identifies a host-driven main switch.
        ///
        type SurfaceVisibilityEvent = r###"{
    source: 'opener' | 'page' | 'shell';
}"###;


        /// Native system Downloads path. Do not pass this to `lx.fs`.
        type SystemDownloadsPath = r###"string & {
    readonly [systemDownloadsPathBrand]: 'system-downloads-path';
}"###;

        type SetBadgeOptions = r###"{
    /**
     * Where to paint: `auto` is every product-owned surface; `appIcon` or `tray` names one.
     * @default 'auto'
     */
    surface?: 'auto' | 'appIcon' | 'tray';
}"###;

        type TrayMenuItem = r###"{
    /** Text shown. */
    label: string;
    /** Invoked when this item is clicked. */
    onClick?: () => void;
    /**
     * False greys it out.
     * @default true
     */
    enabled?: boolean;
    /** Draw a check mark. */
    checked?: boolean;
}"###;

        type TrayMenuSeparator = r###"{
    /** A line between items. */
    separator: true;
}"###;

        type UpdateFailedInfo = r###"UpdateReadyInfo & {
    error?: string;
}"###;

        type UpdateReadyInfo = r###"{
    version?: string;
    channel?: "release" | "draft" | string;
}"###;

        /// Upload options. The file streams from disk, so the size ceiling is
        /// the remote's, not memory.
        ///
        /// `bodyMode` picks the body shape, and with it which of the other
        /// fields apply:
        ///
        /// - `multipart` (default) wraps the file in a `multipart/form-data`
        ///   envelope beside the `formData` text fields — what an ordinary form
        ///   endpoint parses. `name`, `fileName`, and `formData` describe that
        ///   envelope. `formData`, when present, must contain at least one field.
        /// - `raw` sends the file bytes as the entire body. Presigned
        ///   object-storage URLs (S3, OSS, Azure Blob) need this: a multipart
        ///   envelope would be stored verbatim as the object's contents,
        ///   boundary lines and all. `name`, `formData`, and `fileName` are
        ///   then rejected rather than silently dropped.
        ///
        /// @example
        /// ```ts
        /// // A presigned URL is signed for one method and one Content-Type,
        /// // so both have to match whatever the signer used.
        /// const task = lx.network.upload(presignedUrl, {
        ///   file: 'lx://media/clip.mp4',
        ///   method: 'PUT',
        ///   bodyMode: 'raw',
        ///   mimeType: 'video/mp4',
        /// });
        /// for await (const { fraction } of task) render(fraction);
        /// const { ok, status } = await task.result;
        /// ```
        ///
        type UploadOptions = r###"{
    /** The file to send: an `lx://` path from a LingXia API, or a path relative to `lx://userdata`. */
    file: string;
    /**
     * HTTP method.
     * A presigned URL is signed for exactly one method, usually `PUT`.
     * @default 'POST'
     */
    method?: 'POST' | 'PUT' | 'PATCH';
    /**
     * Optional request headers.
     * Restricted headers such as `Referer` are ignored by the runtime.
     * `Content-Type` is yours to set only under `bodyMode: 'raw'`, where it
     * wins over `mimeType`; a multipart body owns the header, because it
     * carries the part boundary.
     */
    headers?: Record<string, string>;
    /** Request timeout in milliseconds. */
    timeoutMs?: number;
    /**
     * The file's MIME type: the file part's type under `multipart`, the
     * request's `Content-Type` under `raw`.
     * @default 'application/octet-stream' under `raw`
     */
    mimeType?: string;
    /** Optional abort signal. */
    signal?: AbortSignal;
} & (
    | {
    /**
     * How the file bytes are framed.
     * @default 'multipart'
     */
    bodyMode?: 'multipart';
    /**
     * Name of the multipart part carrying the file.
     * @default 'file'
     */
    name?: string;
    /** Text fields sent alongside the file. Must be non-empty when set. */
    formData?: Record<string, string>;
    /** Filename announced for the file part. Defaults to the file's own name. */
    fileName?: string;
}
    | {
    /** Send the file bytes as the whole body. Multipart fields are rejected. */
    bodyMode: 'raw';
    name?: never;
    formData?: never;
    fileName?: never;
}
)"###;

        /// The server's answer to `lx.network.upload`; any HTTP status resolves.
        type UploadResult = r###"{
    /** HTTP status code. */
    readonly status: number;
    /** Whether `status` is 2xx. */
    readonly ok: boolean;
    /** Response headers, names lowercased. */
    readonly headers: Readonly<Record<string, string>>;
    /** Response body decoded as UTF-8. */
    readonly text: string;
    /** Parse `text` as JSON. */
    json<T = unknown>(): T;
}"###;

        /// Handle returned by `lx.network.upload`; progress counts request body bytes.
        type UploadTask = r###"CancelableTask<UploadResult>"###;

        type VideoCompressQuality = r###"'low' | 'medium' | 'high'"###;

        type VideoContext = r###"{
    play(): Promise<void>;
    pause(): Promise<void>;
    stop(): Promise<void>;
    seek(positionSeconds: number): Promise<void>;
    requestFullScreen(): Promise<void>;
    exitFullScreen(): Promise<void>;
    setStreamSource(options: StreamSourceOptions): Promise<void>;
    /** A GPU page player's quality: `"auto"` or one of its source labels. */
    setQuality(quality: string): Promise<void>;
    /** A GPU page player's playback rate. */
    setPlaybackRate(rate: number): Promise<void>;
    /** Mutes or unmutes a GPU page player. */
    setMuted(muted: boolean): Promise<void>;
    /** A GPU page player's volume. */
    setVolume(volume: number): Promise<void>;
}"###;

        /// Local video metadata for client-side upload preflight and presentation.
        ///
        /// Track-level codec and audio fields are best-effort. The receiving service
        /// must still validate the uploaded bytes; this result does not indicate
        /// whether the file already exists in cloud storage.
        type VideoInfo = r###"{
    /**
     * Encoded display width in pixels.
     */
    width: number;
    /**
     * Encoded display height in pixels.
     */
    height: number;
    /**
     * Video duration in milliseconds.
     */
    durationMs: number;
    /**
     * Exact local file size in bytes.
     */
    size: number;
    /**
     * Clockwise rotation in degrees (usually `0 | 90 | 180 | 270`).
     */
    rotation?: number;
    /**
     * Average bitrate in bits per second (bps).
     */
    bitrate?: number;
    /**
     * Frame rate in frames per second (fps).
     */
    fps?: number;
    /**
     * Best-effort container MIME type, e.g. `video/mp4`.
     * It may be inferred from the file extension when the platform does not expose it.
     */
    type?: string;
    /**
     * Normalized video-track codec MIME type. Known values include `video/avc`,
     * `video/hevc`, `video/x-vnd.on2.vp8`, `video/x-vnd.on2.vp9`, `video/av01`,
     * `video/mp4v-es`, `video/mpeg2`, and `video/mjpeg`. Other valid `video/*`
     * values may be returned for codecs added by the platform. Omitted when the
     * platform cannot determine it.
     */
    videoCodec?: string;
    /**
     * Whether an audio track was detected. Omitted when the platform cannot determine it.
     */
    hasAudio?: boolean;
    /**
     * Best-effort audio-track codec MIME type, e.g. `audio/mp4a-latm` or `audio/opus`.
     * Omitted when there is no audio track or the platform cannot determine it.
     */
    audioCodec?: string;
    /**
     * Resolved path used by runtime (typically `lx://...`).
     */
    path: string;
}"###;

        type WifiConnectedCallback = r###"(info: WifiConnectedInfo) => void"###;

        type WifiConnectedInfo = r###"WifiInfo & {
    connected: boolean;
    state: string;
}"###;

        /// What `lx.fs.write` writes. Text takes an `encoding`; bytes take none.
        type FsWriteOptions = r###"{
    /** The text to write. */
    data: string;
    /**
     * How `data` is read; `base64` decodes it to bytes before writing.
     * @default 'utf8'
     */
    encoding?: 'utf8' | 'base64';
    /**
     * Replace a file already at the path; `false` rejects instead.
     * @default true
     */
    overwrite?: boolean;
} | {
    /** The bytes to write. */
    data: BinaryFileData;
    /** Bytes are written as they are. */
    encoding?: never;
    /**
     * Replace a file already at the path; `false` rejects instead.
     * @default true
     */
    overwrite?: boolean;
}"###;


        // Runtime namespaces are emitted as global interfaces. Re-export their
        // public module types without maintaining a second declaration.
        type ClipboardApi = "globalThis.ClipboardApi";
        type FileSystemApi = "globalThis.FileSystemApi";
        type HostAppApi = "globalThis.HostAppApi";
        type LxAppApi = "globalThis.LxAppApi";
        type NavApi = "globalThis.NavApi";
        type DeviceApi = "globalThis.DeviceApi";
        type NetworkApi = "globalThis.NetworkApi";
        type OrientationApi = "globalThis.OrientationApi";
        type WifiApi = "globalThis.WifiApi";
        type MediaApi = "globalThis.MediaApi";
        type PickApi = "globalThis.PickApi";
        type TabBarApi = "globalThis.TabBarApi";
        /// The menu-bar (macOS) / system-tray (Windows) item declared in
        /// `lingxia.yaml` (`tray:`). Desktop only: on mobile every method is a
        /// no-op and never throws. Control app only; a guest gets a permission error.
        type TrayApi = "globalThis.TrayApi";
        type UiApi = "globalThis.UiApi";

        /// The product's desktop shell. Changes are Control app only; other
        /// lxapps get a permission error.
        type ShellApi = "globalThis.ShellApi";
        /// Runtime actions in the desktop sidebar's header or footer. The shell
        /// lays them out and reports activation; the callbacks do the rest.
        type ShellSidebarActionsApi = "globalThis.ShellSidebarActionsApi";

        /// Where an action lives in the desktop sidebar.
        ///
        /// - `header`: the caption row beside the window controls. At most two
        ///   icon-only actions; declaring a third rejects the whole `replace`.
        /// - `footer`: always reachable. Icon buttons, plus a `value` drawn
        ///   beside the icon when the action carries one. Scrolls after five
        ///   rows.
        ///
        /// In the collapsed rail every action is an icon button. Apps cannot
        /// configure cell size, row, weight, color, or selected state.
        type ShellSidebarActionPlacement = r###"'header' | 'footer'"###;

        /// One app-declared shell sidebar action. It is a stateless command, not a
        /// selectable navigation item: the shell invokes `onActivate` once and
        /// does not infer a target or active state.
        ///
        /// `value` is footer-only, so a header action with one does not type-check.
        type ShellSidebarAction = r###"ShellSidebarActionBase & (
    | {
        /** Caption row: at most two, icon-only. */
        placement: 'header';
        value?: never;
      }
    | {
        /** Always reachable; scrolls after five rows. */
        placement: 'footer';
        /**
         * The action's current value: a fact that changes with the scene,
         * such as the working brand, store, or account. Drawn beside the icon,
         * tail-truncated, on the rest of its row. Absent or empty, the action
         * is an icon button. At most two footer actions may carry one.
         */
        value?: string;
      }
)"###;

        /// Fields shared by both sidebar action placements.
        type ShellSidebarActionBase = r###"{
    /** Stable, non-empty id; unique across both header and footer actions. */
    id: string;
    /**
     * Local lxapp-accessible icon. Use a bundled relative path such as
     * `public/settings.svg`, or an `lx://temp`, `lx://usercache`, or
     * `lx://userdata` path returned by LingXia file APIs. Native absolute paths,
     * parent traversal, `file:` URLs, and network URLs are rejected; download a
     * remote icon before registration.
     *
     * SVG is a template glyph tinted by the host. Raster PNG/JPEG/WebP retains
     * its colour and is center-cropped to the square icon slot, which is suitable
     * for a brand logo; provide square artwork when the crop matters. A path in
     * `lx://temp` is temporary, so download and register it again after the next
     * Logic launch rather than persisting it.
     *
     * Icon-only actions are told apart by their icon alone: give each one a
     * distinct icon.
     */
    icon: string;
    /**
     * Stable name, such as "Brand". The tooltip and accessibility name; it is
     * never drawn in the sidebar. Put a changing fact in `value` instead.
     */
    label: string;
    /** Visible but non-activatable when true. Defaults to false. */
    disabled?: boolean;
    /**
     * Called once for each enabled mouse, keyboard, accessibility, shortcut, or
     * automation activation. Explicitly open or navigate to the desired content.
     */
    onActivate: () => void;
}"###;

        /// Mutable presentation fields for an existing sidebar action. The patch
        /// must contain at least one field. A key that is present is set; a key
        /// that is absent is left unchanged. Use `replace` to change `placement`
        /// or `onActivate`.
        type ShellSidebarActionUpdate = r###"{
    /** Replacement local icon, with the same path rules as registration. */
    icon?: string;
    /** Replacement non-empty tooltip/accessibility name. */
    label?: string;
    /**
     * Replacement footer value. `undefined`, `null`, or `''` clears it, so
     * `{ value: brand?.name }` clears the value when there is no brand.
     */
    value?: string | null;
    /** Whether the action remains visible but rejects activation. */
    disabled?: boolean;
}"###;

    }
}
