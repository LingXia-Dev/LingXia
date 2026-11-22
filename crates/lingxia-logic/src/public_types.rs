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

        /// What `App({...})` takes: the lxapp's lifecycle hooks.
        type AppConfig = r###"{
    /** Runs once when the lxapp starts, before its first page. */
    onLaunch?: () => void | Promise<void>;
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
    onScreenshot?: () => void | Promise<void>;
}"###;

        /// Runtime-managed app download path, usually under `lx://userdata`.
        type AppDownloadFilePath = r###"string & {
    readonly [appDownloadPathBrand]: 'app-download-file-path';
}"###;

        /// Options for a download into this lxapp's own storage.
        type AppDownloadOptions = r###"DownloadOptionsBase & {
    /**
     * Durable output path under `lx://userdata`; relative paths resolve there.
     * The file must not exist yet. Omit for a temporary file. `'downloads'`
     * is the user's Downloads folder instead (`DownloadsDownloadOptions`).
     */
    to?: string;
    suggestedName?: never;
}"###;

        /// Where a download into this lxapp's storage landed: a temporary file, or the `to` path under `lx://userdata`.
        type AppDownloadResult = r###"{
    /** The temporary file. */
    path: AppDownloadFilePath;
    /** A temporary file this lxapp owns. */
    storage: 'temp';
    /** MIME type from the response, when it named one. */
    type?: string;
    /** Bytes written. */
    size: number;
} | {
    /** The `to` path. */
    path: AppDownloadFilePath;
    /** A durable file under `lx://userdata`. */
    storage: 'userdata';
    /** MIME type from the response, when it named one. */
    type?: string;
    /** Bytes written. */
    size: number;
}"###;

        /// Canonical platform-family label shared by `lx.host.info.os`
        /// and `lx.device.info.osName`. `"unknown"` is a non-product build.
        type HostOs = r###"'ios' | 'macos' | 'android' | 'windows' | 'harmony' | 'unknown'"###;

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

        /// An inbound App Link, as `App.onOpen` receives it.
        type OpenLink = r###"{
    /** The link exactly as the OS delivered it. Untrusted. */
    url: string;
    /** The lxapp page path the link resolved to, if any. */
    path: string;
    /** The link's query parameters. Untrusted. */
    query: Record<string, string>;
}"###;

        /// What `App` `onShow` and `onHide` receive: who moved and why.
        type AppLifecycleEventArgs = r###"{
    /** `host` when the whole app moved; `lxapp` when only this lxapp did. */
    source: 'host' | 'lxapp';
    /** What moved it: the app going to the foreground or background, a screenshot, this lxapp opening or closing, or another lxapp taking or giving back the front. */
    reason: 'foreground' | 'background' | 'screenshot' | 'open' | 'close' | 'switch_back' | 'switch_away';
    /** The page a link or switch-back lands on. */
    path?: string;
    /** That page's query. */
    query?: Record<string, string>;
}"###;

        /// Options for `lx.host.screenshot`.
        type AppScreenshotOptions = r###"{
    /**
     * Platform-specific window id to capture (desktop only). Omit to let the
     * platform pick: the key/main window on desktop, the sole window on mobile.
     */
    windowId?: string;
}"###;

        /// The PNG `lx.host.screenshot` wrote.
        type AppScreenshotResult = r###"{
    /** `lx://` path of the captured PNG in the lxapp temp directory. */
    path: string;
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
        /// An lxapp already open, such as docked as an aside, comes back where it
        /// is, and `page` and `params` reach its App `onShow` (`reason: 'switch_back'`).
        type HostTarget = r###"PageTarget | TabTarget | Omit<AppTarget, 'channel' | 'version'> | ActivateTarget | RouteTarget | AppLinkTarget"###;

        /// A target that only brings the product forward.
        type ActivateTarget = r###"{
    /** Bring the product forward, nothing else. */
    activate: true;
    page?: never;
    tab?: never;
    app?: never;
    route?: never;
    appLink?: never;
}"###;

        /// A target that opens a location the host registered, not a page.
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

        /// A target that opens an inbound App Link URL.
        type AppLinkTarget = r###"{
    /** An `https://` URL that is also an inbound App Link. */
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
        /// `lx.supports('host.notification')` first.
        /// Declaring the capability never prompts; permission runs on
        /// `lx.device.requestPermission('notifications')` or the first `show()`
        /// that reaches the OS.
        ///
        /// Every lxapp may call it. The Control app speaks for the product; a
        /// guest lxapp speaks only for itself: its ids are its own (it cannot
        /// replace or cancel another app's), a tap can only open itself, it
        /// may hold 32 live notifications, and it may cause at most one
        /// permission prompt per launch, only while it is open on screen.
        type NotificationApi = r###"{
    /**
     * Post a local notification now or at `schedule`, replacing any under the same `id`.
     * An invalid target, unknown page or route, or an unconfigured host rejects here,
     * before anything is posted. A tap whose target is gone by then brings the
     * product forward and says it is unavailable.
     * @example
     * ```ts
     * await lx.host.notification.show({ title: 'Upload finished', target: { page: 'files' } });
     * ```
     * @throws {E_PERMISSION_DENIED} A guest lxapp's `target` names anything but itself.
     */
    show(options: {
        /** Replace key: what is pending or delivered under it is replaced and its tap target retired. Generated when absent. */
        id?: string;
        /** First line. */
        title: string;
        /** Text below the title. */
        message?: string;
        /**
         * Where a tap goes; absent brings the product forward, nothing else.
         * A guest lxapp may only target itself: `{ page }`, `{ tab }` or `{ app }` with its own id.
         */
        target?: HostTarget;
        /**
         * When to present; a time not in the future posts now.
         * @platform Android: may fire late, and a reboot drops it.
         * @platform HarmonyOS: needs the agent-reminder privilege.
         */
        schedule?: {
            /** Epoch milliseconds. */
            at: number;
        } | {
            /** Milliseconds from now. */
            delayMs: number;
        };
        /**
         * No sound. The banner still appears.
         * @platform HarmonyOS: no effect.
         */
        silent?: boolean;
        /**
         * An immediate post while the product is frontmost: `banner` shows it as
         * `lx.host.banner` would, with an Open button that follows `target`;
         * `system` posts it anyway; `suppress` shows nothing. Where there is no
         * banner (phones), `banner` posts it as `system` does.
         * @platform macOS, iOS, HarmonyOS: `banner` and `system` still suppress when no banner is shown.
         * @default 'banner'
         */
        whenFrontmost?: 'banner' | 'system' | 'suppress';
    }): Promise<{
        /** The replace key, generated when none was given. */
        id: string;
        /**
         * `posted`: the OS took it for display, not proof anyone saw it. `scheduled`:
         * queued for `schedule`, and shown even if the product is frontmost then.
         * `presented`: the product was frontmost, so it shows as a banner instead.
         * `suppressed`: frontmost and nothing shown. Neither needs permission.
         */
        delivery: 'posted' | 'scheduled' | 'presented' | 'suppressed';
    }>;
    /**
     * Remove what is pending or delivered under `id`, and retire its tap
     * target. Unknown ids are fine.
     * @throws {E_INVALID_ARG} An empty `id`.
     * @supports host.notification
     * @throws {E_NOT_SUPPORTED} The host does not declare notifications.
     */
    cancel(id: string): Promise<void>;
    /**
     * Remove every local notification this API posted or scheduled. A guest
     * lxapp's call removes only its own.
     * @supports host.notification
     * @throws {E_NOT_SUPPORTED} The host does not declare notifications.
     */
    cancelAll(): Promise<void>;
}"###;

        /// Product-drawn desktop banner, top-right. Not an OS notification and
        /// not bound to App Link. Desktop only, in every lxapp: elsewhere every
        /// call fails `E_NOT_SUPPORTED`, so ask `lx.supports('host.banner')` first.
        /// Needs no `capabilities` entry in `lingxia.yaml`. The card is 328 pt wide.
        ///
        /// A guest lxapp's banner carries its name and icon, so
        /// it cannot pass as the product's. Its ids are its own, it may have
        /// three banners showing or waiting, each closes after 60s at most
        /// (`durationMs` is capped), and the product's banners go first.
        ///
        /// No buttons: an informational card that auto-dismisses (5s unless
        /// `durationMs` is set). With buttons: a gate that waits for a choice,
        /// dismiss, timeout, or replace. User outcomes resolve; presentation
        /// failures reject.
        type BannerApi = r###"{
    /**
     * Show a banner, or replace the one with the same `id`; resolves with what the person did.
     * @example
     * ```ts
     * const picked = await lx.host.banner.show({
     *   title: 'Run finished',
     *   actions: [{ id: 'open', label: 'Open', target: { page: 'run', params: { id: '42' } } }],
     * });
     * ```
     * @throws {E_INVALID_ARG} No `title`, more than two actions, a bad `background`, or an invalid action `target`.
     * @supports host.banner
     * @throws {E_NOT_SUPPORTED} Off the desktop.
     */
    show(options: {
        /**
         * Replace key, 1 to 64 characters: showing the same `id` again replaces that banner, which
         * resolves `replaced`. Different ids queue, one on screen at a time.
         */
        id?: string;
        /** First line. */
        title: string;
        /** Text below the title. */
        message?: string;
        /** Buttons, at most two; with any, the banner waits for a choice instead of closing by itself. */
        actions?: ReadonlyArray<{
            /** What `action` reports when this button is picked. */
            id: string;
            /** Button text. */
            label: string;
            /** Emphasis: `primary` for the expected choice, `danger` for a destructive one. */
            tone?: 'default' | 'primary' | 'danger';
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
        durationMs?: number;
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
            /** Closed by the person, by `durationMs`, or by a banner with the same `id`. */
            reason: 'dismissed' | 'timeout' | 'replaced';
        }
    >;
    /**
     * Close the banner under `id`; it resolves `dismissed`. Unknown ids are fine.
     * @throws {E_INVALID_ARG} An empty `id`.
     * @supports host.banner
     * @throws {E_NOT_SUPPORTED} Off the desktop.
     */
    dismiss(id: string): Promise<void>;
}"###;

        /// Launch-at-startup control for the host app.
        ///
        /// Wherever the host cannot register a startup item every call fails
        /// `E_NOT_SUPPORTED`; ask first:
        ///
        /// ```ts
        /// if (lx.supports('host.autostart')) {
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
        /// Host-app-level capability: like `update.check` and `screenshot`, the methods
        /// are available only to the native-assigned Control app; other lxapps receive
        /// a permission error.
        type AutostartApi = r###"{
    /**
     * Whether the app is registered to launch at startup, read from the OS
     * each time; the user can change it outside the app, so re-read it when
     * the settings screen shows.
     * @platform macOS: login items.
     * @platform Windows: the `Run` registry key.
     * @throws {E_PERMISSION_DENIED} From an lxapp other than the Control app.
     * @supports host.autostart
     * @throws {E_NOT_SUPPORTED} The host cannot register a startup item.
     */
    isEnabled(): Promise<boolean>;
    /**
     * Register the app as a startup item for the current user.
     * Idempotent. Call it only from the person's own action.
     * @platform macOS: the system may tell the user a login item was added.
     * @throws {E_PERMISSION_DENIED} From an lxapp other than the Control app.
     * @supports host.autostart
     * @throws {E_NOT_SUPPORTED} The host cannot register a startup item.
     */
    enable(): Promise<void>;
    /**
     * Unregister the app as a startup item for the current user. Idempotent.
     * @throws {E_PERMISSION_DENIED} From an lxapp other than the Control app.
     * @supports host.autostart
     * @throws {E_NOT_SUPPORTED} The host cannot register a startup item.
     */
    disable(): Promise<void>;
}"###;

        /// The product-wide cache a settings screen reports and clears.
        ///
        /// App-scoped, not lxapp-scoped: the figure covers every lxapp the host
        /// has run. Control app only; a guest's call fails `E_PERMISSION_DENIED`.
        type AppCacheApi = r###"{
    /**
     * Bytes `clear()` would free, estimated; open lxapps' caches and the WebView cache are not counted.
     * @throws {E_PERMISSION_DENIED} From an lxapp other than the Control app.
     */
    size(): Promise<number>;
    /**
     * Clear reclaimable host caches. Control app only. Live session usercache and
     * temp are preserved, including the caller's. Does not restart any lxapp.
     * Userdata, KV, Downloads, cookies, valid installs and host components survive.
     * Per-category failures are reported in `failures`.
     * @throws {E_PERMISSION_DENIED} From an lxapp other than the Control app.
     * @throws {E_INTERNAL} The clear could not start.
     */
    clear(): Promise<{
        /** Estimated file bytes successfully removed; excludes WebView cache. */
        freedBytes: number;
        /** Protected usercache/session paths skipped, not a count of apps. */
        skippedActivePaths: number;
        /** What happened to the WebView's own cache. */
        webview: 'cleared' | 'unsupported' | 'failed';
        /** Categories that could not be cleared. */
        failures: readonly string[];
    }>;
}"###;
        /// The terminal's font: family candidates, size, line height and ligatures.
        type TerminalFontSettings = r###"{
    /** Ordered candidates; the first installed monospaced family wins. */
    family: readonly string[];
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

        /// A complete set of terminal settings: font and theme.
        type TerminalSettingsValue = r###"{
    /** Font settings. */
    font: TerminalFontSettings;
    /** Color scheme per appearance. */
    theme: TerminalThemeSettings;
}"###;

        /// The terminal settings to change; fields left out keep their value.
        type TerminalSettingsPatch = r###"{
    /** Font fields to change; others keep their value. */
    font?: Partial<TerminalFontSettings>;
    /** Scheme names to change; others keep their value. */
    theme?: Partial<TerminalThemeSettings>;
}"###;

        /// A problem with the user's saved terminal settings, reported instead of thrown.
        type TerminalSettingsWarning = r###"{
    /** What is wrong: the user's settings file does not parse, or names a scheme that does not exist. */
    code: 'invalid_user_file' | 'missing_color_scheme';
    /** A sentence for a person. */
    message: string;
}"###;

        /// The terminal settings at one revision: defaults, the user's overrides and what is in effect.
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
            missing: readonly string[];
            /** None of the requested families is installed, so a system monospace font is drawn. */
            fellBack: boolean;
        };
    };
    /** Problems found while reading the settings; they never block it. */
    warnings: readonly TerminalSettingsWarning[];
}"###;

        /// A terminal color scheme: background, foreground, cursor, selection and the 16 ANSI colors.
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

        /// A stored terminal color scheme with its name and where it came from.
        type TerminalColorSchemeDetails = r###"{
    /** The name settings refer to it by. */
    name: string;
    /** Shipped with the product, or imported by the user. */
    source: 'built_in' | 'imported';
    /** Its colors. */
    scheme: TerminalColorScheme;
}"###;

        /// A font family installed on this machine.
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

        /// Shows a color scheme live in open terminals without saving it.
        type TerminalPreviewController = r###"{
    /** Preview a stored name or an unpersisted scheme. Last request wins. */
    show(scheme: string | TerminalColorScheme): Promise<void>;
    /** Restore saved settings only when this controller owns the preview. */
    clear(): Promise<void>;
    /** Idempotently clear and retire this controller. */
    close(): Promise<void>;
}"###;

        /// Read and change the saved terminal settings, checked against a revision.
        type TerminalSettingsApi = r###"{
    /**
     * The settings now: defaults, the user's overrides, and what is in effect.
     * @throws {E_PERMISSION_DENIED} Outside the host's own control surface.
     * @throws {E_IO} The saved settings could not be read.
     */
    get(): Promise<TerminalSettingsSnapshot>;
    /**
     * Save the patched fields.
     * @throws {E_INVALID_STATE} The saved settings moved past `ifRevision`; `data.reason` is `'revision_conflict'`.
     */
    update(patch: TerminalSettingsPatch & {
        /** The `revision` the patch was made against. */
        ifRevision: number;
    }): Promise<TerminalSettingsSnapshot>;
    /**
     * Drop the user's overrides.
     * @throws {E_INVALID_STATE} The saved settings moved past `ifRevision`; `data.reason` is `'revision_conflict'`.
     */
    reset(options: {
        /** The `revision` the reset was decided against. */
        ifRevision: number;
        /** Reset only the font or only the theme; both when absent. */
        scope?: 'font' | 'theme';
    }): Promise<TerminalSettingsSnapshot>;
    /** Fires after saved settings, effective appearance, or fonts change. */
    onChange(listener: (snapshot: TerminalSettingsSnapshot) => void, options?: SignalOptions): () => void;
}"###;

        /// The terminal color schemes: list, import and preview.
        type TerminalColorSchemesApi = r###"{
    /**
     * Every scheme, shipped and imported.
     * @throws {E_PERMISSION_DENIED} Outside the host's own control surface.
     */
    list(): Promise<TerminalColorSchemeDetails[]>;
    /**
     * Add a scheme from its text.
     * @throws {E_INVALID_DATA} The text is not a scheme in any of the three formats.
     * @throws {E_ALREADY_EXISTS} A scheme has that name and `overwrite` is not set.
     * @throws {E_IO} It could not be saved.
     * @throws {E_PERMISSION_DENIED} Outside the host's own control surface.
     */
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

        /// The font families a terminal can use.
        type TerminalFontsApi = r###"{
    /**
     * The font families installed on this machine.
     * @throws {E_PERMISSION_DENIED} Outside the host's own control surface.
     */
    list(): Promise<InstalledTerminalFont[]>;
}"###;

        /// Whether the Microsoft ConPTY runtime that passes inline images through is installed and selected.
        type WindowsTerminalInlineImageStatus = r###"{
    /** New terminal sessions use the installed runtime. */
    enabled: boolean;
    /** The runtime is installed. */
    installed: boolean;
    /** The one package version this host installs. */
    package: {
        /** Its version. */
        version: string;
        /** Where to download it. */
        url: string | URL;
        /** The SHA-256 digest `install` checks. */
        sha256: string;
        /** Its size in bytes. */
        bytes: number;
    };
}"###;

        /// Install and select the ConPTY runtime that passes inline images through, on Windows.
        type WindowsTerminalApi = r###"{
    /**
     * Whether the runtime is installed and selected.
     * @supports host.terminal.windows
     * @throws {E_NOT_SUPPORTED} Off Windows.
     * @throws {E_PERMISSION_DENIED} Outside the host's own control surface.
     */
    status(): Promise<WindowsTerminalInlineImageStatus>;
    /**
     * Verify and install the fixed Microsoft ConPTY package from lxapp temp storage.
     * @supports host.terminal.windows
     * @throws {E_NOT_SUPPORTED} Off Windows.
     * @throws {E_PERMISSION_DENIED} Outside the host's own control surface, or `path` is not this lxapp's temp file.
     * @throws {E_INVALID_DATA} The package fails verification.
     */
    install(options: {
        /** The downloaded package, an `lx://temp/` path. */
        path: string;
    }): Promise<WindowsTerminalInlineImageStatus>;
    /**
     * Select the installed runtime for new terminal sessions.
     * @supports host.terminal.windows
     * @throws {E_NOT_SUPPORTED} Off Windows.
     * @throws {E_PERMISSION_DENIED} Outside the host's own control surface.
     * @throws {E_INVALID_STATE} Enabling before the runtime is installed.
     */
    setEnabled(options: {
        /** Use the runtime for new sessions. */
        enabled: boolean;
    }): Promise<WindowsTerminalInlineImageStatus>;
}"###;

        /// The host terminal's settings, color schemes, fonts and Windows runtime.
        type TerminalApi = r###"{
    /** Saved terminal settings, revision-checked on write. */
    readonly settings: TerminalSettingsApi;
    /** Installed color schemes, plus import and live preview. */
    readonly colorSchemes: TerminalColorSchemesApi;
    /** Terminal fonts installed on this machine. */
    readonly fonts: TerminalFontsApi;
    /** The optional Windows runtime that passes inline images through. */
    readonly windows: WindowsTerminalApi;
}"###;

        /// Bytes `lx.fs.write` and the clipboard accept.
        type BinaryFileData = r###"ArrayBuffer | ArrayBufferView"###;

        /// What the product's light/dark scheme is set to. `'auto'` follows
        /// the system.
        type AppearancePreference = r###"'auto' | 'light' | 'dark'"###;

        /// `lx.app.appearance` — the scheme this lxapp renders in: the one its
        /// `lxapp.json` pins, else the product's.
        type AppAppearanceApi = r###"Live<ResolvedAppearance>"###;

        /// `lx.host.appearance` — the product's scheme preference; only the Control
        /// app gets past it. A system flip under `'auto'` moves `lx.app.appearance`
        /// and leaves this quiet.
        type HostAppearanceApi = r###"Live<AppearancePreference> & {
    /**
     * Pin the product to `'light'` or `'dark'`, or follow the system with
     * `'auto'`. An lxapp that pinned a scheme in its manifest keeps it. Resolves
     * once this lxapp's own `lx.app.appearance` follows.
     * @throws {E_INVALID_ARG} Any other value.
     * @throws {E_PERMISSION_DENIED} From an lxapp other than the Control app.
     */
    set(preference: AppearancePreference): Promise<void>;
}"###;

        /// What the product's language is set to: `'auto'` follows the system,
        /// or any canonical BCP-47 tag. `string & {}` keeps `'auto'` in
        /// autocomplete while still accepting a tag.
        type DisplayLanguagePreference = r###"'auto' | (string & {})"###;

        /// `lx.app.language` — the language this lxapp renders in, as a BCP-47 tag:
        /// the product's language narrowed to the `languages` its lxapp.json lists
        /// (the exact tag, else the same language, else the first). Pages read it
        /// as `env.language`; `watch` fires only when the narrowed value changes.
        /// Logic follows it for text it hands to native chrome, which nothing
        /// re-renders on its behalf.
        type AppLanguageApi = r###"Live<string>"###;

        /// `lx.host.displayLanguage` — the product's language preference: `'auto'`
        /// or a canonical BCP-47 tag; only the Control app gets past it. A system
        /// locale change under `'auto'` moves `lx.app.language` and leaves this quiet.
        type HostDisplayLanguageApi = r###"Live<DisplayLanguagePreference> & {
    /**
     * Persist the product's language.
     * @throws {E_INVALID_ARG} A tag that is not valid BCP-47.
     * @throws {E_PERMISSION_DENIED} From an lxapp other than the Control app.
     */
    set(preference: DisplayLanguagePreference): Promise<void>;
}"###;

        /// The scheme in effect: light or dark.
        type ResolvedAppearance = r###"'light' | 'dark'"###;
        /// Whether a piece of page chrome shows: `auto` leaves it to the runtime, `hidden` hides it.
        type VisibilityPreference = r###"'auto' | 'hidden'"###;
        /// Whether the tab bar shows: `auto` leaves it to the runtime, or always `visible` or `hidden`.
        type TabBarVisibilityPreference = r###"'auto' | 'visible' | 'hidden'"###;

        /// Navigation bar colors to change; `null` restores the declared one.
        type NavigationBarStylePatch = r###"{
    /** Bar color. */
    backgroundColor?: string | null;
    /** Title and button color. */
    foregroundColor?: string | null;
    /** The line under the bar. */
    dividerColor?: string | null;
}"###;

        /// Navigation bar fields to change; fields left out keep their value.
        type NavigationBarPatch = r###"{
    /** Title text; `null` restores the declared one. */
    title?: string | null;
    /** Whether the home button may show. */
    homeButton?: VisibilityPreference;
    /** Colors to change; `null` restores every declared color. */
    style?: NavigationBarStylePatch | null;
}"###;

        /// One tab's label, icons and badge to change.
        type TabBarItemPatch = r###"{
    /** Label text; `null` restores the declared one. */
    label?: string | null;
    /**
     * Package-relative path, or an `lx://temp` / `lx://usercache` /
     * `lx://userdata` path from a LingXia file API. Network URLs are rejected —
     * download first (`lx.network.download`) and pass the returned path.
     *
     * SVG (and SF Symbols) are template-tinted with `foregroundColor` /
     * `selectedForegroundColor`. Raster files (PNG/JPEG/WebP) keep their own
     * colour so a brand mark is not flattened into a solid square.
     */
    icon?: string | null;
    /** Text in a badge, `true` for a dot without text, `null` to remove either. */
    badge?: string | true | null;
}"###;

        /// Patch for `lx.app.tabBar.update()`. Items, badges, and
        /// visibility only — a `style` field is rejected. Colors stay in
        /// static `lxapp.json` `tabBar.style`. `backgroundColor` is
        /// mobile-only; the desktop sidebar follows the host
        /// `lingxia.yaml` theme.
        ///
        /// @example
        /// ```ts
        /// await lx.app.tabBar.update({ items: { home: { badge: '3' } } });
        /// ```
        type TabBarPatch = r###"{
    /** Show or hide the bar. */
    visibility?: TabBarVisibilityPreference;
    /** Changes per tab, keyed by the tab's page name. */
    items?: { readonly [K in TabName]?: TabBarItemPatch };
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
    onSelect: () => void | Promise<void>;
}"###;

        /// Options for `lx.pick.directory`.
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
    /** An `lx://temp` path this lxapp can read. */
    path: string;
    /** The file's name as the person saw it. */
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

        /// Options for `lx.pick.file` and `lx.pick.files`.
        type PickFileOptions = r###"{
    /** Optional file filters. Empty or omitted means all file types. */
    filters?: readonly FileDialogFilter[];
    /**
     * Initial directory the dialog opens in.
     *
     * When this resolves to an app-local directory, LingXia may use its internal
     * file picker. When omitted, the platform system file picker is used.
     */
    defaultPath?: string;
}"###;

        /// Result of `lx.pick.file` and `lx.pick.capture`: one file.
        type PickFileResult = r###"{
    /** A file was picked. */
    status: 'ok';
    /** The picked file. */
    file: PickedFile;
} | CanceledResult"###;

        /// Result of `lx.pick.files` and `lx.pick.media`: at least one file.
        /// From `lx.pick.media` on mobile, each file is a copy in the lxapp
        /// temp directory, counted against its quota.
        type PickFilesResult = r###"{
    /** At least one file was picked. */
    status: 'ok';
    /** The picked files, in the order chosen. */
    files: readonly [PickedFile, ...PickedFile[]];
} | CanceledResult"###;

        /// Options for `lx.pick.media`: how many and which kinds, from the library.
        type PickMediaOptions = r###"{
    /**
     * Most files to return. Where the system picker cannot enforce it
     * (desktop), files beyond it are dropped from the result.
     * @default 9
     */
    limit?: number;
    /**
     * Kinds to offer.
     * @default ['image', 'video']
     */
    types?: readonly ('image' | 'video')[];
}"###;

        /// Options for `lx.pick.capture`: what the camera records.
        type CaptureOptions = r###"{
    /** A photo or a video. */
    type: 'image' | 'video';
    /**
     * Which camera.
     * @default 'back'
     */
    facing?: 'back' | 'front';
    /** Longest video to record, in milliseconds; rounded up to whole seconds. */
    maxDurationMs?: number;
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
    path: string;
}"###;

        /// Options for `lx.clipboard.read`.
        type ClipboardReadOptions = r###"{
    /** Omit to receive every representation the host can surface. */
    type?: ClipboardType;
}"###;

        /// Result of `lx.clipboard.readText`. Branch on `status`.
        /// A copied empty string is `{ status: 'ok', text: '' }`;
        /// an image-only clipboard has no `text`.
        type ClipboardTextResult = r###"{
    /** The clipboard was read. */
    status: 'ok';
    /** The text; absent when the clipboard holds none. */
    text?: string;
} | CanceledResult"###;

        /// Result of `lx.clipboard.read`. Omit `type` to receive every
        /// representation this host can surface. A requested type that is
        /// absent leaves `items` empty, not an error.
        type ClipboardReadResult = r###"{
    /** The clipboard was read. */
    status: 'ok';
    /** One entry per representation; empty when nothing matched. */
    items: readonly ClipboardItem[];
} | CanceledResult"###;

        /// One clipboard representation: text, or an image file.
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
    path: string;
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

        /// The smaller video `lx.media.compressVideo` wrote.
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

        /// The network `lx.network.wifi.connect` joins.
        type ConnectWifiOptions = r###"{
    /** Network name. */
    ssid: string;
    /** Omit for an open network. */
    password?: string;
    /** Aborting it stops waiting for the system, and the call rejects. */
    signal?: AbortSignal;
}"###;

        /// What a managed path is: a regular file, a directory or a symbolic link.
        type FsEntryKind = r###"'file' | 'directory' | 'symlink'"###;

        /// Why an `lx` call failed, where `code` alone does not say: `e.reason === 'wifiPasswordWrong'`.
        ///
        /// - `initializationFailed` with `E_INTERNAL`
        /// - `userCancelled` with `E_ABORT`
        /// - `userDeclined` with `E_ABORT`
        /// - `cameraPermissionDenied` with `E_PERMISSION_DENIED`
        /// - `locationPermissionDenied` with `E_PERMISSION_DENIED`
        /// - `microphonePermissionDenied` with `E_PERMISSION_DENIED`
        /// - `photosPermissionDenied` with `E_PERMISSION_DENIED`
        /// - `storagePermissionDenied` with `E_PERMISSION_DENIED`
        /// - `notificationsPermissionDenied` with `E_PERMISSION_DENIED`
        /// - `bluetoothPermissionDenied` with `E_PERMISSION_DENIED`
        /// - `clipboardPermissionDenied` with `E_PERMISSION_DENIED`
        /// - `serviceUnavailable` with `E_INVALID_STATE`
        /// - `locationServicesDisabled` with `E_INVALID_STATE`
        /// - `bluetoothDisabled` with `E_INVALID_STATE`
        /// - `nfcDisabled` with `E_INVALID_STATE`
        /// - `featureDisabled` with `E_INVALID_STATE`
        /// - `networkUnavailable` with `E_NETWORK`
        /// - `serverFailed` with `E_NETWORK`
        /// - `connectionFailed` with `E_NETWORK`
        /// - `deviceUnsupported` with `E_NOT_SUPPORTED`
        /// - `hostUpgradeRequired` with `E_NOT_SUPPORTED`
        /// - `wifiUninitialized` with `E_INVALID_STATE`
        /// - `wifiFailed` with `E_NETWORK`
        /// - `wifiPasswordWrong` with `E_NETWORK`
        /// - `wifiTimedOut` with `E_TIMEOUT`
        /// - `wifiRequestDuplicated` with `E_NETWORK`
        /// - `wifiUnsupported` with `E_NOT_SUPPORTED`
        /// - `wifiPermissionDenied` with `E_NETWORK`
        /// - `wifiPromptUnanswered` with `E_NETWORK`
        /// - `wifiConnectionDeclined` with `E_NETWORK`
        /// - `wifiDisabled` with `E_INVALID_STATE`
        /// - `wifiNetworkNotFound` with `E_NOT_FOUND`
        type LxErrorReason = r###"'initializationFailed' | 'userCancelled' | 'userDeclined' | 'cameraPermissionDenied' | 'locationPermissionDenied' | 'microphonePermissionDenied' | 'photosPermissionDenied' | 'storagePermissionDenied' | 'notificationsPermissionDenied' | 'bluetoothPermissionDenied' | 'clipboardPermissionDenied' | 'serviceUnavailable' | 'locationServicesDisabled' | 'bluetoothDisabled' | 'nfcDisabled' | 'featureDisabled' | 'networkUnavailable' | 'serverFailed' | 'connectionFailed' | 'deviceUnsupported' | 'hostUpgradeRequired' | 'wifiUninitialized' | 'wifiFailed' | 'wifiPasswordWrong' | 'wifiTimedOut' | 'wifiRequestDuplicated' | 'wifiUnsupported' | 'wifiPermissionDenied' | 'wifiPromptUnanswered' | 'wifiConnectionDeclined' | 'wifiDisabled' | 'wifiNetworkNotFound'"###;

        /// A permission `lx.device.permission` reads and `lx.device.requestPermission` asks for.
        type PermissionName = r###"'location' | 'camera' | 'photos' | 'notifications'"###;

        /// A permission's state: `prompt` means the person has not been asked yet.
        type PermissionState = r###"'granted' | 'denied' | 'prompt'"###;

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

        /// The screen's orientation.
        type DeviceOrientation = r###""portrait" | "landscape""###;

        /// Options every download takes: headers and an abort signal.
        type DownloadOptionsBase = r###"{
    /**
     * Optional request headers.
     * Restricted headers such as `Referer` are ignored by the runtime.
     */
    headers?: Record<string, string>;
    /** Aborting it cancels the download, and the task's `result` rejects. */
    signal?: AbortSignal;
}"###;

        /// Where any download landed.
        type DownloadResult = r###"AppDownloadResult | DownloadsDownloadResult"###;

        /// Options for a download into the user's Downloads folder.
        type DownloadsDownloadOptions = r###"DownloadOptionsBase & {
    /** The user's Downloads folder. Needs the host's downloads grant. */
    to: 'downloads';
    /** Filename hint; the host picks a free name next to it. */
    suggestedName?: string;
}"###;

        /// Where a download into the user's Downloads folder landed.
        type DownloadsDownloadResult = r###"{
    /** Where the file landed in the user's Downloads folder. */
    path: SystemDownloadsPath;
    /** The Downloads folder. */
    storage: 'downloads';
    /** MIME type from the response, when it named one. */
    type?: string;
    /** File size in bytes. */
    size: number;
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

        /// One file type a file picker offers.
        type FileDialogFilter = r###"{
    /** Optional label shown in the native dialog (desktop only). */
    name?: string;
    /** Allowed extensions without dots, e.g. ['pdf', 'txt']. */
    extensions: readonly string[];
}"###;

        /// Options for `lx.device.location`.
        type GetLocationOptions = r###"{
    /**
     * Coordinate system of the result.
     * @default 'wgs84'
     */
    coordinates?: 'wgs84' | 'gcj02';
    /**
     * `high` asks for a precise fix; slower and costs more power.
     * @default 'balanced'
     */
    accuracy?: 'balanced' | 'high';
    /** Aborting it stops waiting for a fix, and the call rejects. */
    signal?: AbortSignal;
}"###;

        /// Build-time deployment environment of the host app (`dev` | `prod`).
        ///
        /// Surfaced via {@link HostAppApi.env}. Taken from the `env` field in
        /// the generated `app.json`. Missing `env` is treated as `'prod'`.
        ///
        /// This is the immutable host build axis: package-id suffix, publish
        /// token, signed App Link entitlements, and self-update channel.
        /// In-app App Link hosts follow {@link HostAppApi.serviceEnv} on
        /// the next launch. The mutable service environment is
        /// {@link HostAppApi.setServiceEnv}. It is **not** the
        /// lxapp publish channel (`LxAppReleaseType`:
        /// `'release' | 'draft'`). Default channel is `release` and can be
        /// overridden when opening an lxapp.
        ///
        type HostAppEnv = r###"'dev' | 'prod'"###;

        /// A step of applying a host app update: downloading or installing.
        type HostAppUpdateApplyStage = r###"'download' | 'install'"###;

        /// A newer published host app version.
        type HostAppUpdateInfo = r###"{
    /** The newer version. */
    version: string;
    /** Package size in bytes, when the feed names it. */
    size?: number;
    /** Release notes, one entry per line. */
    releaseNotes?: readonly string[];
    /**
     * How this update is applied. `store` opens the platform marketplace;
     * `direct` downloads and self-installs, the only case where
     * self-update is supported.
     * @supports host.selfUpdate
     */
    via: 'direct' | 'store';
    /**
     * Apply this checked update.
     *
     * `apply()` is single-use for this update object. It also claims custom
     * host updates for the rest of this process, same as
     * {@link HostUpdateApi.claim}.
     *
     * Await `task.result` when progress is not needed, or iterate the task
     * to render it.
     *
     * On `direct`, downloads and hands off install. On `store`, opens the
     * platform store listing (no package is downloaded) and resolves
     * `store_opened`; whether the user then updates is not reported.
     */
    apply(): HostAppUpdateTask;
}"###;

        /// What applying a host app update did.
        type HostAppUpdateResult = r###"{
    /** `store_opened` via `store`: the listing opened, nothing was installed. */
    state: 'install_requested' | 'store_opened';
}"###;

        /// Handle returned by `apply()`. Its progress is the package download;
        /// `total` is present when the update named its size.
        type HostAppUpdateTask = r###"Task<HostAppUpdateResult>"###;

        /// A hardware key press or release.
        type KeyEvent = r###"{
    /** Key value following W3C naming (e.g. "Enter", "ArrowLeft", "a") */
    key: string;
    /** Physical key code (e.g. "ENTER", "DPAD_LEFT") */
    code: string;
    /** Alt (Option) is held. */
    altKey?: boolean;
    /** Control is held. */
    ctrlKey?: boolean;
    /** Shift is held. */
    shiftKey?: boolean;
    /** Command (macOS) or the Windows key is held. */
    metaKey?: boolean;
    /** The key is held down and this event is a repeat. */
    repeat?: boolean;
}"###;

        /// Receives each hardware key event.
        type KeyEventCallback = r###"(event: KeyEvent) => void"###;

        /// Which channel an lxapp build came from: a release or a draft.
        type LxAppReleaseType = r###"'release' | 'draft'"###;

        /// How media fills its box: crop, letterbox, stretch, or fit.
        type MediaObjectFit = r###"'cover' | 'contain' | 'fill' | 'fit'"###;

        /// A clockwise rotation in degrees.
        type MediaRotation = r###"0 | 90 | 180 | 270"###;

        /// Options for `lx.fs.mkdir`.
        type FsMkdirOptions = r###"{
    /**
     * Create missing parents too; no error when it exists.
     * @default false
     */
    recursive?: boolean;
}"###;

        /// A tab root of this lxapp.
        type TabName = r###"keyof LxAppPages extends never ? string : keyof LxAppTabs & string"###;

        /// A navigation target that switches to one of this lxapp's tabs.
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
    url: string | URL;
    page?: never;
    tab?: never;
    app?: never;
    params?: never;
}"###;

        /// Anywhere `lx.nav.go` can go: a page, a tab, another lxapp, or a URL.
        type NavTarget = r###"PageTarget | TabTarget | AppTarget | UrlTarget"###;



        /// Connectivity right now: whether it is up, its type and addresses.
        type NetworkInfo = r###"{
    /** Any network is usable. */
    isConnected: boolean;
    /**
     * `'none'` when offline. `'unknown'` with `isConnected: false` means the
     * platform has not reported yet.
     */
    networkType: NetworkType;
    /** The primary IPv4 address, at most one; empty when not connected. */
    ipv4: readonly string[];
    /** The primary IPv6 address, at most one; empty when not connected. */
    ipv6: readonly string[];
}"###;

        /// The kind of network the device is on.
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
     * @default 'tap_outside' for floats, 'manual' for windows
     */
    dismiss?: 'tap_outside' | 'manual';
    /**
     * Block interaction with content below.
     * @default false
     */
    modal?: boolean;
}"###;

        /// Built-in browser product page. Opening one requires
        /// `capabilities.browser` and is restricted to the native-assigned Control app.
        type BuiltinShellPage = r###"'downloads'"###;

        /// A float's size hint.
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

        /// A page's params, keyed by name.
        type PageQuery = r###"Record<string, PageQueryValue>"###;

        /// A page param: any JSON value. `null` and `undefined` mean absent. The
        /// whole set of non-string params is limited to 8 KB as JSON.
        type PageQueryValue = r###"JsonValue | undefined"###;

        /// What a preview does when an item ends: wait, go to the next, or loop.
        type PreviewMediaAdvance = r###"'manual' | 'next' | 'loop'"###;

        /// One change-stream event / the `current` snapshot.
        type PreviewMediaChange = r###"{
    /** Position of the shown item. */
    index: number;
    /** The shown item. */
    source: PreviewMediaShownSource;
}"###;

        /// Why a media preview closed.
        type PreviewMediaCloseReason = r###"'manual' | 'completed' | 'interrupted' | 'error'"###;

        /// A media session. `presented` tells a rendered first frame from a
        /// session that ended before one. `completed` reports closure;
        /// native playback errors may report reason `error`, while request failures reject.
        type PreviewMediaHandle = r###"{
    /** `true` once the first item is on screen; `false` when the session ended first. */
    readonly presented: Promise<boolean>;
    /** The item on screen now. */
    readonly current: PreviewMediaChange;
    /** Fires when the shown item changes; `current` holds the latest. Returns a function that removes the listener. */
    onChange(listener: (change: PreviewMediaChange) => void, options?: SignalOptions): () => void;
    /** Settles when the preview closes. */
    readonly completed: Promise<PreviewMediaResult>;
}"###;

        /// What `lx.media.preview` shows: one path, one source, or a list.
        type PreviewMediaOptions = r###"string | PreviewMediaSingleOptions | PreviewMediaSequenceOptions"###;

        /// How a media preview ended.
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

        /// A media preview of several items, starting at one of them.
        type PreviewMediaSequenceOptions = r###"{
    /**
     * Preview list. Supports images, videos, or a mixed queue.
     */
    sources: readonly PreviewMediaSource[];
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

        /// A media preview of one item.
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

        /// One image or video in a media preview.
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



        /// Options for `lx.fs.remove`.
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


        /// Options for `lx.pick.code`.
        type PickCodeOptions = r###"{
    /**
     * Read only from the camera; `false` also lets the person pick a picture.
     * @default true
     */
    onlyFromCamera?: boolean;
    /** Symbologies to read; every one when absent. */
    formats?: readonly ('bar_code' | 'qr_code' | 'datamatrix' | 'pdf417')[];
}"###;

        /// Result of `lx.pick.code`. `format` is the symbology that was read.
        type PickCodeResult = r###"{
    /** A code was read. */
    status: 'ok';
    /** Its decoded text. */
    value: string;
    /** The symbology, in snake_case, e.g. `qr_code`. */
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

        /// What `lx.share` hands to the share sheet: text, the current page, or files.
        type ShareOptions = r###"ShareTextOptions | SharePageOptions | ShareFilesOptions"###;

        /// The current page as a share target, with or without a query.
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

        /// Query values appended to a shared page's link.
        type ShareQuery = r###"Record<string, string | number | boolean>"###;

        /// What the share sheet reported.
        type ShareResult = r###"{
    /**
     * What the share sheet reported. Not part of the `canceled` family: some
     * platforms only observe that the system UI opened and closed, so the
     * unknown case is stated rather than hidden in a missing boolean that
     * every call site would read as "not shared".
     */
    outcome: 'completed' | 'dismissed' | 'unknown';
}"###;

        /// The title and text every share may carry.
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

        /// The title every share may carry.
        type ShareTitleOptions = r###"{
    /**
     * Share title.
     */
    title?: string;
}"###;

        /// The choices `lx.ui.actionSheet` shows.
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
    /** Aborting it closes the sheet, and the call rejects. */
    signal?: AbortSignal;
}"###;

        /// An acknowledgement dialog has no cancel button.
        type AlertOptions = r###"{
    /** The question or statement. */
    title: string;
    /** Detail below the title. */
    message?: string;
    /** Label of the only button. */
    confirmText?: string;
    /** Aborting it closes the dialog, and the call rejects. */
    signal?: AbortSignal;
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
    /** Aborting it closes the dialog, and the call rejects. */
    signal?: AbortSignal;
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
    /** Aborting it dismisses the toast. */
    signal?: AbortSignal;
}"###;

        /// Current persistent-storage usage and configured limits.
        ///
        type StorageInfo = r###"{
    /** Stored data, in bytes. */
    usedBytes: number;
    /** The configured ceiling, in bytes. */
    limitBytes: number;
    /** Number of stored keys; `list()` names them. */
    keys: number;
}"###;

        /// A stream a page player plays from a provider the host registered.
        type StreamSourceOptions = r###"{
    /** The name the host registered the provider under. */
    provider: string;
    /** A live stream, with no fixed length to seek in. */
    isLive: boolean;
    /** Length in milliseconds; ignored when `isLive`. */
    durationMs?: number;
    /** Passed to the provider as it starts. */
    params?: Record<string, unknown>;
}"###;

        /// What a surface's `onClosed` receives.
        type SurfaceClosedEvent = r###"{
    /** Why it closed. */
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
    /** The surface failure in detail. */
    readonly data?: {
        /** Which surface rule refused the call. */
        readonly code?: SurfaceErrorCode;
    };
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

        /// Options every `lx.surface.openPage` placement takes.
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

        /// Options for `lx.surface.openUrl`: where the browser opens.
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

        /// Which instance `lx.surface.declared` looks up.
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
     * `lx.surface.declared(id, { key })`. 1 to 128 UTF-8 bytes. Declarations
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
    version?: string;
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
     * @supports surface.window
     * @throws {E_NOT_SUPPORTED} `as: 'window'` where the host has no windows.
     * @throws {E_NOT_FOUND} A page the app does not declare.
     */
    openPage<N extends ConfiguredPageName>(page: N, ...options: OpenPageArgs<N>): PageSurface<NavResult<N>, NavMessages<N>>;
    /** Open external content in the in-app browser. */
    openUrl(url: string | URL, options?: OpenUrlOptions): Promise<TabSurface>;
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
     * with `lx.surface.declared` and `lx.surface.app`.
     */
    get(key: string): PageSurface | TabSurface | undefined;
    /**
     * The handle of an lxapp live as a surface of its own, declared or not;
     * `undefined` when it is not. Control app only.
     */
    app(appId: string): AppSurface | undefined;
    /**
     * The handle of a live declared surface, or of its keyed instance, the same
     * object on every call; `undefined` when it is not live. Control app only.
     */
    declared(id: string, options?: GetDeclaredOptions): DeclaredSurface | AppSurface | undefined;
}"###;

        /// Why a surface closed.
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
    /** Who changed it: the opener's handle, the page itself, or the shell. */
    source: 'opener' | 'page' | 'shell';
}"###;


        /// Native system Downloads path. Do not pass this to `lx.fs`.
        type SystemDownloadsPath = r###"string & {
    readonly [systemDownloadsPathBrand]: 'system-downloads-path';
}"###;

        /// Where `lx.host.badge.set` and `clear` paint.
        type BadgeOptions = r###"{
    /**
     * Where to paint: `auto` is every product-owned surface; `app_icon` or `tray` names one.
     * @default 'auto'
     */
    surface?: 'auto' | 'app_icon' | 'tray';
}"###;

        /// What a badge call changed.
        type BadgeResult = r###"{
    /** Whether any surface took the change; `false` when one exists but declined, such as a hidden tray item. */
    painted: boolean;
}"###;

        /// A mark on the product's own system chrome, such as an unread count. A badge never
        /// prompts, and a notification never sets one.
        type BadgeApi = r###"{
    /**
     * Show `value` on the badge, such as an unread count.
     * @supports host.badge
     * @platform macOS: the dock and the menu-bar item; not painted while the tray item is hidden or notifications are denied.
     * @platform Windows: the taskbar and the notification-area item.
     * @platform iOS: the home-screen icon, numbers only; needs notification permission.
     * @platform HarmonyOS: the home-screen icon.
     * @throws {E_NOT_SUPPORTED} The platform has no badge surface of the requested kind, such as on Android.
     * @throws {E_INVALID_ARG} An empty value, or text where the platform paints numbers only.
     * @throws {E_PERMISSION_DENIED} From an lxapp other than the Control app.
     */
    set(value: string | number, options?: BadgeOptions): Promise<BadgeResult>;
    /**
     * Remove the badge.
     * @supports host.badge
     * @throws {E_NOT_SUPPORTED} The platform has no badge surface of the requested kind, such as on Android.
     * @throws {E_PERMISSION_DENIED} From an lxapp other than the Control app.
     */
    clear(options?: BadgeOptions): Promise<BadgeResult>;
}"###;

        /// One row of the tray item's menu.
        type TrayMenuItem = r###"{
    /** Text shown. */
    label: string;
    /** Runs when the person picks this item. */
    onSelect?: () => void;
    /** Greys it out. */
    disabled?: boolean;
    /** Draw a check mark. */
    checked?: boolean;
}"###;

        /// What `lx.host.tray.update` changes; an absent key is left as it is.
        type TrayPatch = r###"{
    /** A bundled resource path. */
    icon?: string;
    /** Text beside the icon (macOS); `null` or `''` clears it. */
    title?: string | null;
    /** Show or hide the item. */
    visible?: boolean;
}"###;

        /// A line between rows of the tray item's menu.
        type TrayMenuSeparator = r###"{
    /** A line between items. */
    separator: true;
}"###;

        /// What `lx.app.update.onFailed` receives.
        type UpdateFailedInfo = r###"UpdateReadyInfo & {
    /** What went wrong, for a log line. */
    error?: string;
}"###;

        /// What `lx.app.update.onReady` receives: the downloaded bundle.
        type UpdateReadyInfo = r###"{
    /** The downloaded bundle's version. */
    version?: string;
    /** The channel it came from. */
    channel?: "release" | "draft" | string;
}"###;

        /// Upload options. The file streams from disk, so the size ceiling is
        /// the remote's, not memory.
        ///
        /// `bodyMode` picks the body shape, and with it which of the other
        /// fields apply:
        ///
        /// - `multipart` (default) wraps the file in a `multipart/form-data`
        ///   envelope beside the `fields` text fields — what an ordinary form
        ///   endpoint parses. `field`, `fileName`, and `fields` describe that
        ///   envelope. `fields`, when present, must contain at least one entry.
        /// - `raw` sends the file bytes as the entire body. Presigned
        ///   object-storage URLs (S3, OSS, Azure Blob) need this: a multipart
        ///   envelope would be stored verbatim as the object's contents,
        ///   boundary lines and all. `field`, `fields`, and `fileName` are
        ///   then rejected rather than silently dropped.
        ///
        /// @example
        /// ```ts
        /// // A presigned URL is signed for one method and one Content-Type,
        /// // so both have to match whatever the signer used.
        /// const task = lx.network.upload(presignedUrl, {
        ///   path: 'lx://media/clip.mp4',
        ///   method: 'PUT',
        ///   bodyMode: 'raw',
        ///   type: 'video/mp4',
        /// });
        /// for await (const { fraction } of task) render(fraction);
        /// const { ok, status } = await task.result;
        /// ```
        ///
        type UploadOptions = r###"{
    /** The file to send: an `lx://` path from a LingXia API, or a path relative to `lx://userdata`. */
    path: string;
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
     * wins over `type`; a multipart body owns the header, because it
     * carries the part boundary.
     */
    headers?: Record<string, string>;
    /**
     * The file's MIME type: the file part's type under `multipart`, the
     * request's `Content-Type` under `raw`.
     * @default 'application/octet-stream' under `raw`
     */
    type?: string;
    /** Aborting it cancels the upload, and the task's `result` rejects. */
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
    field?: string;
    /** Text fields sent alongside the file. Must be non-empty when set. */
    fields?: Record<string, string>;
    /** Filename announced for the file part. Defaults to the file's own name. */
    fileName?: string;
}
    | {
    /** Send the file bytes as the whole body. Multipart fields are rejected. */
    bodyMode: 'raw';
    field?: never;
    fields?: never;
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

        /// A preset size for `lx.media.compressVideo`.
        type VideoCompressQuality = r###"'low' | 'medium' | 'high'"###;

        /// A page `<video>`'s controller, from `page.video(name)`.
        type PagePlayer = r###"{
    /** Start or resume playback. */
    play(): Promise<void>;
    /** Pause at the current position. */
    pause(): Promise<void>;
    /** Pauses and returns to the start. */
    stop(): Promise<void>;
    /** Jump to a position, in milliseconds from the start. */
    seek(positionMs: number): Promise<void>;
    /** Show the player full screen. */
    requestFullscreen(): Promise<void>;
    /** Leave full screen. */
    exitFullscreen(): Promise<void>;
    /** Play from a stream provider the host registered instead of a URL. */
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

        /// Receives each Wi-Fi connection change.
        type WifiConnectedCallback = r###"(info: WifiConnectedInfo) => void"###;

        /// A Wi-Fi network and whether the device is joined to it.
        type WifiConnectedInfo = r###"WifiInfo & {
    /** The device is joined to this network now. */
    connected: boolean;
    /** The platform's own word for the connection state, such as `connected`. */
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
        /// This lxapp's own updates: a newer bundle downloads in the background.
        type LxAppUpdateApi = "globalThis.LxAppUpdateApi";
        /// The actions this lxapp adds to the host's More menu.
        type MoreActionsApi = "globalThis.MoreActionsApi";
        type NavApi = "globalThis.NavApi";
        type DeviceApi = "globalThis.DeviceApi";
        type NetworkApi = "globalThis.NetworkApi";
        type OrientationApi = "globalThis.OrientationApi";
        type WifiApi = "globalThis.WifiApi";
        type MediaApi = "globalThis.MediaApi";
        type PickApi = "globalThis.PickApi";
        type TabBarApi = "globalThis.TabBarApi";
        type TrayApi = "globalThis.TrayApi";
        /// The tray item's dropdown menu.
        type TrayMenuApi = "globalThis.TrayMenuApi";
        /// Check for a newer host app and take over the update flow. Control app only.
        type HostUpdateApi = "globalThis.HostUpdateApi";
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
        /// selectable navigation item: the shell invokes `onSelect` once and
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
    onSelect: () => void;
}"###;

        /// Mutable presentation fields for an existing sidebar action. The patch
        /// must contain at least one field. A key that is present is set; a key
        /// that is absent is left unchanged. Use `replace` to change `placement`
        /// or `onSelect`.
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
