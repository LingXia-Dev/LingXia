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

        /// The user dismissed the operation. Never an error.
        ///
        type CanceledResult = r###"{
    canceled: true;
}"###;

        /// Result of `lx.showActionSheet`. Branch on `canceled` before reading
        /// the selected item index.
        type ActionSheetResult = r###"{
    canceled: false;
    /** Index of the tapped item in `itemList`. */
    index: number;
} | CanceledResult"###;

        type AppConfig = r###"{
    globalData?: Record<string, unknown>;
    onLaunch?: (options?: AppLaunchOptions) => void | Promise<void>;
    onShow?: (args?: AppLifecycleEventArgs) => void | Promise<void>;
    onHide?: (args?: AppLifecycleEventArgs) => void | Promise<void>;
    onUserCaptureScreen?: () => void | Promise<void>;
    [key: string]: unknown;
}"###;

        /// Runtime-managed app download path, usually under `lx://userdata`.
        type AppDownloadFilePath = r###"string & {
    readonly [appDownloadPathBrand]: 'app-download-file-path';
}"###;

        type AppDownloadOptions = r###"DownloadOptionsBase & {
    /**
     * Optional app-owned durable output path.
     *
     * Omit `filePath` to receive a temporary result in `tempFilePath`. Relative
     * paths resolve under user data. `lx://` paths must target `lx://userdata`;
     * `lx://usercache` is not accepted here.
     */
    filePath?: string;
    /**
     * App-owned output. Omit to use a temporary output unless `filePath` is set.
     */
    destination?: 'app';
}"###;

        type AppDownloadResult = r###"{
    /**
     * Temporary result.
     *
     * Not durable; move or copy it to `lx://userdata` if you need to keep it.
     *
     * When `filePath` is omitted, the runtime must be able to infer a file
     * type from the URL or the server's `Content-Type` header.
     */
    tempFilePath: string;
    filePath?: never;
    mimeType?: string;
    size: number;
} | {
    /** Durable destination under `lx://userdata`. */
    filePath: AppDownloadFilePath;
    tempFilePath?: never;
    mimeType?: string;
    size: number;
}"###;

        type AppInstance = r###"AppConfig & {
    globalData: Record<string, unknown>;
}"###;

        type AppLaunchOptions = r###"{
    path?: string;
    query?: Record<string, string>;
    scene?: number;
    referrerInfo?: {
        appId?: string;
        extraData?: Record<string, unknown>;
    };
}"###;

        type AppLifecycleEventArgs = r###"{
    source: 'host' | 'lxapp';
    reason: 'foreground' | 'background' | 'screenshot' | 'open' | 'close' | 'switch_back' | 'switch_away';
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
    tempFilePath: string;
    /** Image width in pixels, when the runtime could read it from the PNG. */
    width?: number;
    /** Image height in pixels, when the runtime could read it from the PNG. */
    height?: number;
}"###;

        /// Launch-at-startup control for the host app.
        ///
        /// Absent (`undefined`) wherever the host cannot register a startup item.
        /// `lx.supports({ capability: 'autostart' })` and the member's presence always
        /// agree, so either gate works:
        ///
        /// ```ts
        /// if (lx.supports({ capability: 'autostart' })) {
        ///   // render the "Launch at startup" toggle
        /// }
        /// ```
        ///
        /// Requires `capabilities.autostart: true` in `lingxia.yaml`; without it the
        /// member is absent on all platforms. Declaring the capability never enables
        /// autostart by itself — the SDK registers the app only when `setEnabled(true)`
        /// is called, so the decision stays with the user (typically a settings-page
        /// toggle, default off).
        ///
        /// Host-app-level capability: like `checkUpdate` and `screenshot`, the methods
        /// are available only to the home lxapp; other lxapps receive a permission
        /// error.
        ///
        type AutostartApi = r###"{
    /**
     * Whether the app is currently registered to launch at startup, read from
     * the OS (macOS login items / Windows `Run` registry key) — never a cached
     * preference. The user can flip this outside the app (System Settings on
     * macOS, Task Manager's Startup page on Windows), so re-read it whenever the
     * settings UI is shown.
     */
    isEnabled(): Promise<boolean>;
    /**
     * Register or unregister the app as a startup item for the current user.
     * Idempotent. On macOS the system may notify the user that a login item was
     * added — only call this from an explicit user action.
     */
    setEnabled(on: boolean): Promise<void>;
}"###;

        type TerminalThemeMode = r###"'system' | 'light' | 'dark'"###;

        type TerminalFontSettings = r###"{
    /** Ordered candidates; the first installed monospaced family wins. */
    family: string[];
    size: number;
    lineHeight: number;
    ligatures: boolean;
}"###;

        type TerminalThemeSettings = r###"{
    mode: TerminalThemeMode;
    light: string;
    dark: string;
}"###;

        type TerminalSettingsValue = r###"{
    font: TerminalFontSettings;
    theme: TerminalThemeSettings;
}"###;

        type TerminalSettingsPatch = r###"{
    font?: Partial<TerminalFontSettings>;
    theme?: Partial<TerminalThemeSettings>;
}"###;

        type TerminalSettingsWarning = r###"{
    code: 'invalidUserFile' | 'missingColorScheme';
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
    effective: {
        /** Host appearance before applying terminal.theme.mode. */
        systemAppearance: 'light' | 'dark';
        appearance: 'light' | 'dark';
        colorScheme: string | null;
        font: {
            family: string;
            missing: string[];
            fellBack: boolean;
        };
    };
    warnings: TerminalSettingsWarning[];
}"###;

        type TerminalColorScheme = r###"{
    name?: string;
    background: string;
    foreground: string;
    cursorColor?: string;
    selectionBackground?: string;
    selectionForeground?: string;
    black: string;
    red: string;
    green: string;
    yellow: string;
    blue: string;
    purple: string;
    cyan: string;
    white: string;
    brightBlack: string;
    brightRed: string;
    brightGreen: string;
    brightYellow: string;
    brightBlue: string;
    brightPurple: string;
    brightCyan: string;
    brightWhite: string;
}"###;

        type TerminalColorSchemeDetails = r###"{
    name: string;
    source: 'builtIn' | 'imported';
    scheme: TerminalColorScheme;
}"###;

        type InstalledTerminalFont = r###"{
    family: string;
    monospace: boolean;
    ligatures: boolean;
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
    get(): Promise<TerminalSettingsSnapshot>;
    update(
        patch: TerminalSettingsPatch,
        options: { ifRevision: number },
    ): Promise<TerminalSettingsSnapshot>;
    reset(options: {
        ifRevision: number;
        scope?: 'font' | 'theme';
    }): Promise<TerminalSettingsSnapshot>;
    /** Fires after saved settings, effective appearance, or fonts change. */
    onChange(listener: (snapshot: TerminalSettingsSnapshot) => void): () => void;
}"###;

        type TerminalColorSchemesApi = r###"{
    list(): Promise<TerminalColorSchemeDetails[]>;
    import(options: {
        text: string;
        name?: string;
        /** Existing names are rejected unless overwrite is explicit. */
        overwrite?: boolean;
    }): Promise<TerminalColorSchemeDetails>;
    createPreview(): TerminalPreviewController;
}"###;

        type TerminalFontsApi = r###"{
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
    readonly settings: TerminalSettingsApi;
    readonly colorSchemes: TerminalColorSchemesApi;
    readonly fonts: TerminalFontsApi;
    /** Windows-only optional inline-image compatibility runtime. */
    readonly windows?: WindowsTerminalApi;
}"###;

        type BinaryFileData = r###"ArrayBuffer | ArrayBufferView"###;

        type AppearancePreference = r###"'auto' | 'light' | 'dark'"###;
        type ResolvedAppearance = r###"'light' | 'dark'"###;
        type VisibilityPreference = r###"'auto' | 'hidden'"###;

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

        type TabBarStylePatch = r###"{
    foregroundColor?: string | null;
    selectedForegroundColor?: string | null;
}"###;

        type TabBarItemPatch = r###"{
    index: number;
    text?: string | null;
    iconPath?: string | null;
    selectedIconPath?: string | null;
    badge?: string | null;
    redDot?: boolean;
}"###;

        type TabBarPatch = r###"{
    visibility?: VisibilityPreference;
    style?: TabBarStylePatch | null;
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
    onClick: () => void | Promise<void>;
}"###;

        type ChooseDirectoryOptions = r###"{
    /** Initial directory the dialog opens in. Platform default if omitted. */
    defaultPath?: string;
}"###;

        /// Result of `lx.chooseDirectory`. Branch on `canceled` before reading
        /// the selected directory.
        type ChooseDirectoryResult = r###"{
    canceled: false;
    /** Native-consumable directory reference (path or URI). */
    path: string;
} | CanceledResult"###;

        type ChooseFileOptions = r###"{
    /** Allow selecting multiple files. Default: false */
    multiple?: boolean;
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

        /// Result of `lx.chooseFile`. Branch on `canceled` before reading the
        /// selected paths.
        type ChooseFileResult = r###"{
    canceled: false;
    /**
     * File paths returned by LingXia; always at least one. Values may be
     * app-local paths, `lx://...` paths, or platform system-picker references.
     * Treat them as opaque strings and pass them back to LingXia APIs such as
     * `lx.share`.
     */
    paths: [string, ...string[]];
} | CanceledResult"###;

        type ChooseMediaOptions = r###"{
    count?: number;
    mediaType?: ('image' | 'video')[];
    sourceType?: ('album' | 'camera')[];
    camera?: 'back' | 'front';
    maxDuration?: number;
}"###;

        /// Result of `lx.chooseMedia`. Branch on `canceled` before reading the
        /// selected entries.
        type ChooseMediaResult = r###"{
    canceled: false;
    /** Picked media; always at least one entry. */
    entries: [ChosenMediaEntry, ...ChosenMediaEntry[]];
} | CanceledResult"###;

        type ChosenMediaEntry = r###"{
    tempFilePath: string;
    fileType: 'image' | 'video';
    isOriginal: boolean;
}"###;

        type CompressImageOptions = r###"{
    path: string;
    quality?: number;
    compressedWidth?: number;
    compressedHeight?: number;
}"###;

        type CompressImageResult = r###"{
    tempFilePath: string;
}"###;

        type CompressVideoIteratorResult = r###"{
    done: boolean;
    value?: CompressVideoProgressEvent;
}"###;

        type CompressVideoOptions = r###"{
    /**
     * Source video path or `lx://` URI.
     */
    path: string;
    /**
     * Cross-platform note: video compression parameters are best-effort and may map to
     * native presets instead of exact encoder settings.
     *
     * Compression quality preset.
     * When provided, `bitrate`, `fps`, and `resolution` are ignored.
     */
    quality?: VideoCompressQuality;
    /**
     * Preferred target video bitrate in kbps.
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
    /**
     * Optional output path for compressed file.
     */
    outputPath?: string;
}"###;

        type CompressVideoProgressEvent = r###"{
    /** Transcode progress in percent, `0`-`100`. */
    progress: number;
}"###;

        type CompressVideoResult = r###"{
    tempFilePath: string;
    width: number;
    height: number;
    durationMs: number;
    /**
     * Output file size in bytes.
     * Could be close to source size when platform falls back to source content.
     */
    size: number;
    type: string;
}"###;

        /// Handle returned by `lx.compressVideo`.
        ///
        /// Awaiting the task resolves with the final {@link CompressVideoResult}.
        /// Iterating it with `for await` yields {@link CompressVideoProgressEvent}s
        /// while the transcode runs.
        ///
        type CompressVideoTask = r###"PromiseLike<CompressVideoResult> & AsyncIterable<CompressVideoProgressEvent> & {
    next(): Promise<CompressVideoIteratorResult>;
    /** Stops iteration only. Does not cancel the compression. */
    return(): Promise<CompressVideoIteratorResult>;
    catch<TResult = never>(onrejected?: ((reason: unknown) => TResult | PromiseLike<TResult>) | null): Promise<CompressVideoResult | TResult>;
    finally(onfinally?: (() => void) | null): Promise<CompressVideoResult>;
    /**
     * Cancels the transcode and deletes any partial output.
     * The task promise rejects with an `AbortError` (`code: 'E_ABORT'`).
     */
    cancel(): void;
    wait(): Promise<CompressVideoResult>;
}"###;

        type ConnectWifiOptions = r###"{
    SSID: string;
    password?: string;
}"###;

        type FsCopyOptions = r###"{
    /** Defaults to false. */
    overwrite?: boolean;
}"###;

        /// Display and orientation APIs.
        ///
        type DeviceOrientation = r###""portrait" | "landscape""###;

        type DeviceOrientationChangeEvent = r###"{
    value: DeviceOrientation;
}"###;

        type DownloadDestination = r###"'app' | 'downloads'"###;

        type DownloadOptionsBase = r###"{
    /** HTTP(S) source URL. */
    url: string;
    /**
     * Optional request headers.
     * Restricted headers such as `Referer` are ignored by the runtime.
     */
    headers?: Record<string, string>;
    /** Request timeout in milliseconds. */
    timeout?: number;
    /** Optional abort signal. */
    signal?: AbortSignal;
}"###;

        type DownloadResult = r###"AppDownloadResult | DownloadsDownloadResult"###;

        type DownloadsDownloadOptions = r###"DownloadOptionsBase & {
    /**
     * Optional filename hint for the system Downloads destination.
     * This is not an app-owned `lx.fs` path.
     */
    filePath?: string;
    /** Save into the user's system Downloads directory. */
    destination: 'downloads';
}"###;

        type DownloadsDownloadResult = r###"{
    /** Native system Downloads path. Do not pass this to `lx.fs`. */
    filePath: SystemDownloadsPath;
    tempFilePath?: never;
    mimeType?: string;
    size: number;
}"###;

        type ExtractVideoThumbnailOptions = r###"{
    /**
     * Source video path or `lx://` URI.
     */
    path: string;
    /**
     * Optional output image path. If omitted, runtime chooses a temporary path.
     */
    outputPath?: string;
    /**
     * Max output width in pixels.
     * Optional; when set with/without `maxHeight`, output keeps aspect ratio (no cropping).
     */
    maxWidth?: number;
    /**
     * Max output height in pixels.
     * Optional; when set with/without `maxWidth`, output keeps aspect ratio (no cropping).
     */
    maxHeight?: number;
    /**
     * Target frame time in milliseconds from video start.
     * `0` means first frame.
     */
    timeMs?: number;
    /**
     * JPEG quality in range `0-100`.
     */
    quality?: number;
}"###;

        type ExtractVideoThumbnailResult = r###"{
    /**
     * Generated thumbnail file path.
     */
    tempFilePath: string;
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
    /** Optional label shown in the native dialog. */
    name?: string;
    /** Allowed extensions without dots, e.g. ['pdf', 'txt']. */
    extensions: string[];
}"###;

        /// Media picker, preview, scan, and file processing APIs.
        ///
        type GetImageInfoOptions = r###"{
    path: string;
}"###;

        /// Location APIs.
        ///
        type GetLocationOptions = r###"{
    type?: 'wgs84' | 'gcj02';
    altitude?: boolean;
    isHighAccuracy?: boolean;
    highAccuracyExpireTime?: number;
}"###;

        type GetVideoInfoOptions = r###"{
    /**
     * Video file path or `lx://` URI.
     */
    path: string;
}"###;

        /// Build-time environment version of the host app.
        ///
        /// Surfaced via {@link HostAppApi.envVersion}. Mirrors the
        /// `crates/lingxia-update::ReleaseType` enum and the `envVersion` field in the
        /// generated `app.json`. Pre-envVersion app artifacts are treated as `'release'`.
        ///
        /// Note: this is *separate* from `LxAppEnvVersion` in the navigator module,
        /// which encodes lxapp release channels (`'develop' | 'preview' | 'release'`)
        /// for cross-app navigation URLs and uses the truncated `develop` form.
        ///
        type HostAppEnvVersion = r###"'developer' | 'preview' | 'release'"###;

        type HostAppUpdateApplyStage = r###"'download' | 'install'"###;

        type HostAppUpdateCheckResult = r###"{
    hasUpdate: false;
    update?: never;
} | {
    hasUpdate: true;
    update: HostAppUpdateInfo;
}"###;

        type HostAppUpdateEvent = r###"{
    state: 'downloading';
    downloadedBytes?: number;
    progress?: number;
} | {
    state: 'downloaded' | 'installRequested';
} | {
    state: 'failed';
    stage: HostAppUpdateApplyStage;
    error: string;
}"###;

        type HostAppUpdateInfo = r###"{
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
     * Requires `lx.supports({ capability: 'selfUpdate' })`. Where the host cannot
     * install its own update it rejects with an unsupported-operation error;
     * use `version` and `releaseNotes` to guide users to the app marketplace.
     */
    apply(): HostAppUpdateTask;
}"###;

        type HostAppUpdateIteratorResult = r###"{
    done: boolean;
    value?: HostAppUpdateEvent;
}"###;

        type HostAppUpdateResult = r###"{
    state: 'installRequested';
}"###;

        type HostAppUpdateTask = r###"PromiseLike<HostAppUpdateResult> & AsyncIterable<HostAppUpdateEvent> & {
    next(): Promise<HostAppUpdateIteratorResult>;
    /** Stops iteration only. It does not cancel an app update already handed to the platform. */
    return(): Promise<HostAppUpdateIteratorResult>;
    catch<TResult = never>(onrejected?: ((reason: unknown) => TResult | PromiseLike<TResult>) | null): Promise<HostAppUpdateResult | TResult>;
    finally(onfinally?: (() => void) | null): Promise<HostAppUpdateResult>;
    wait(): Promise<HostAppUpdateResult>;
}"###;

        /// Input event APIs.
        ///
        /// Platform support: Android only
        ///
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

        type LxAppEnvVersion = r###"'release' | 'preview' | 'develop'"###;

        /// LxApp metadata APIs.
        ///
        type LxAppReleaseType = r###"'release' | 'preview' | 'developer'"###;

        /// Device action APIs.
        ///
        type MakePhoneCallOptions = r###"{
    phoneNumber: string;
}"###;

        type MediaObjectFit = r###"'cover' | 'contain' | 'fill' | 'fit'"###;

        type MediaRotation = r###"0 | 90 | 180 | 270"###;

        type FsMkdirOptions = r###"{
    recursive?: boolean;
}"###;

        /// Result of `lx.showModal`. `canceled: false` means the user confirmed;
        /// there is no third resolved outcome. Presentation failures reject.
        ///
        type ModalResult = r###"{
    canceled: false;
} | CanceledResult"###;

        /// Options for `lx.navigateBack()`. Omit the object or `delta` to pop
        /// one page.
        type NavigateBackOptions = r###"{
    /** Number of pages to pop. Defaults to 1. */
    delta?: number;
}"###;

        /// Navigate to another lxapp inside the current App Surface. JavaScript
        /// callers address pages by their configured name; page routes are an
        /// internal runtime detail and are not accepted as input.
        type NavigateToAppOptions = r###"{
    appId: string;
    /**
     * Configured page name from the target lxapp's `lxapp.json`. Omit it to
     * open the target app's initial page. Full routes such as
     * `/pages/home/index` are not supported.
     */
    page?: string;
    query?: PageQuery;
    envVersion?: LxAppEnvVersion;
    targetVersion?: string;
}"###;

        type NetworkChangeCallback = r###"(info: NetworkInfo) => void"###;

        type NetworkInfo = r###"{
    isConnected: boolean;
    networkType: NetworkType;
    ipv4: string[];
    ipv6: string[];
}"###;

        /// Open a surface declaration from the host's `lingxia.yaml`.
        /// `key` identifies an additional reusable instance when supported;
        /// `as` changes the live instance's role without changing its identity.
        type OpenDeclaredSurfaceSpec = r###"{
    surface: string;
    /**
     * Stable caller-owned identity for an additional native declaration
     * instance. Leading/trailing whitespace is ignored; use 1 to 128 UTF-8
     * bytes. Declarations without instantiable native providers reject it.
     */
    key?: string;
    /**
     * Omit to use the declaration's role. Overrides must be realizable by the
     * declared provider. `float` does not synthesize a popover contract: the
     * declaration must already provide the required float/tray presentation.
     * A `main` occupies the primary content area, appears in the main/sidebar
     * switcher, and has no content-area tab strip. An `aside` occupies a
     * companion region around the main and uses that region's tab strip; it
     * never enters the main switcher. A stable root rejects non-main overrides.
     */
    as?: 'main' | 'aside' | 'float';
    interaction?: never;
    page?: never;
    url?: never;
    appId?: never;
    envVersion?: never;
    targetVersion?: never;
    position?: never;
    size?: never;
    query?: never;
} & ({
    edge?: never;
} | {
    /**
     * Preferred docking side when the effective role is `aside`. `aside`
     * selects the companion region; `edge` selects a side within that region.
     * Omit it to keep the declaration's edge. Compact hosts may reproject it.
     */
    as?: 'aside';
    edge: SurfaceEdge;
})"###;

        /// Network status APIs.
        ///
        type NetworkType = r###"'none' | 'unknown' | 'wifi' | '2g' | '3g' | '4g' | '5g' | 'ethernet'"###;

        /// Compose a dynamic business lxapp as its own shell Surface (home lxapp
        /// only). Unlike `navigateToApp`, this creates a parallel shell item and
        /// lifecycle handle. No YAML declaration is required. Reopening its
        /// current role focuses the live instance; changing a live main/aside
        /// role requires closing it first.
        ///
        /// Dynamic App Surfaces intentionally do not support `float`: a shell
        /// float needs a host-declared tray anchor, dismissal policy, and
        /// presentation contract that `{ appId }` cannot supply. Declare that
        /// lxapp as a float in `lingxia.yaml` and open it with `{ surface }`.
        type OpenAppSurfaceSpec = r###"{
    appId: string;
    /**
     * Configured page name from the target lxapp's `lxapp.json`. Omit it to
     * open the target app's initial page. Full page routes are not supported.
     */
    page?: string;
    query?: PageQuery;
    /** Defaults to 'release'. */
    envVersion?: LxAppEnvVersion;
    targetVersion?: string;
    interaction?: never;
    url?: never;
    surface?: never;
    key?: never;
    position?: never;
    size?: never;
} & ({
    /**
     * Occupies the primary content area and appears in the main/sidebar
     * switcher. Main content has no tab strip of its own. Dynamic floats are
     * not supported.
     */
    as: 'main';
    edge?: never;
} | {
    /**
     * Occupies a companion region around the current main, with switching in
     * that region's tab strip. It does not appear in the main/sidebar switcher.
     */
    as: 'aside';
    /**
     * Preferred docking side. `aside` selects the companion region; `edge`
     * selects where that region docks on layouts with room. Omit it for the
     * default (`right` for dynamic lxapps); compact hosts may reproject it.
     */
    edge?: SurfaceEdge;
})"###;

        /// File system APIs.
        ///
        type OpenFileOptions = r###"{
    /** Local file path or runtime-managed temp path. */
    filePath: string;
    /** Optional coarse file type hint such as `pdf`, `docx`, or `xlsx`. */
    fileType?: string;
    /**
     * `auto`: prefer native review, then fall back to external open.
     * `review`: require native review UI and reject when unsupported.
     * `external`: hand off directly to the system / external app.
     */
    mode?: 'auto' | 'review' | 'external';
    /** Hint for whether the native review UI should expose its action menu when supported. */
    showMenu?: boolean;
}"###;

        /// Spec for {@link OpenSurfaceSpec}. A discriminated union keyed by source so a
        /// page name and a declared surface id never collide (each is its own string
        /// space, separately type-checkable).
        ///
        /// - `{ page }` — one of this lxapp's own pages, by name, arranged as `as`
        ///   (`float` is a popup; `window` is a bare desktop window, which rejects on
        ///   mobile). `position` applies to `float`, and `size` is a Host-clamped hint.
        ///   They are fixed at open (re-open to change). Your own pages **cannot** be
        ///   docked as an `aside` — an aside is external content only (see `{ url }`).
        ///   For a side panel of your own, use a declared `surface`, an in-page split
        ///   layout, or `role: main` for a switchable destination.
        ///
        ///   `float` is a popup layered above the main at `position` (like a dialog); it
        ///   takes no layout space. `interaction` controls the native close button,
        ///   outside-click dismissal, and modality. Defaults are no button,
        ///   `tapOutside`, and non-modal.
        /// - `{ surface }` — a surface declared in `lingxia.yaml` `surfaces:`, by id
        ///   (e.g. `'terminal'`, `'ai-assistant'`). Form, position, and startup data come
        ///   from the declaration.
        /// - `{ url }` — external content in the in-app browser. Without `as` it opens as
        ///   a main browser tab (the **self** browser: full chrome **with an editable
        ///   address bar**, no handle). With `as: 'aside'` it opens in the **browser
        ///   aside** — a docked (large screen) / full-screen (phone) **multi-tab** browser
        ///   for external content only (`https://` or `file://`).
        ///
        ///   The aside is **API-only** and never permits address editing or a manual
        ///   "new tab" action. Desktop may show the current address read-only; compact
        ///   phone/Runner chrome omits the address row entirely.
        ///   Tabs are **deduped by URL** — reopening a URL focuses the existing tab and
        ///   preserves its current navigation. On `medium` / `expanded`, the returned
        ///   handle is **tab-scoped**: `close()` closes that tab. Compact browser chrome
        ///   owns the group and returns `null`. Closing the last tab closes the aside;
        ///   dismissing it only hides the group. The tab UI shows page **titles** (never
        ///   the URL), plus per-tab close, back/forward, refresh, and dismissal.
        ///
        ///   Presentation is the only large/small difference: on `medium` / `expanded`
        ///   the aside **docks** and splits beside the main at `edge` (default `'right'`)
        ///   with a horizontal title tab strip; on `compact` (phone / runner) it presents
        ///   **full-screen** with a single-row **bottom** browser toolbar (tabs reached
        ///   via an aside-only switcher). System/edge Back and the toolbar dismiss action
        ///   exit the whole aside even when page history exists; the explicit browser
        ///   Back button navigates history. `size` is a host-clamped preferred size
        ///   (large screen only).
        ///
        type OpenPageSurfaceSpec = r###"{
    /** Configured page name from this lxapp's `lxapp.json`. */
    page: string;
    /** A popup above the main. */
    as: 'float';
    position?: SurfaceFloatPosition;
    size?: OverlaySurfaceSize;
    interaction?: SurfaceInteraction;
    query?: Record<string, unknown>;
    edge?: never;
    surface?: never;
    url?: never;
    appId?: never;
    key?: never;
    envVersion?: never;
    targetVersion?: never;
} | {
    /** Configured page name from this lxapp's `lxapp.json`. */
    page: string;
    as: 'window';
    size?: WindowSurfaceSize;
    /** Windows use manual dismissal; `tapOutside` is invalid. */
    interaction?: SurfaceInteraction;
    query?: Record<string, unknown>;
    edge?: never;
    position?: never;
    surface?: never;
    url?: never;
    appId?: never;
    key?: never;
    envVersion?: never;
    targetVersion?: never;
}"###;

        /// Native interaction supplied by the host around page content.
        type SurfaceInteraction = r###"{
    /** Show the standard circular close button. Default `false`. */
    closeButton?: boolean;
    /** Default `tapOutside` for floats and `manual` for windows. */
    dismiss?: 'tapOutside' | 'manual';
    /** Block interaction with content below. Default `false`. */
    modal?: boolean;
}"###;

        type OpenSurfaceSpec = r###"OpenPageSurfaceSpec | OpenDeclaredSurfaceSpec | OpenAppSurfaceSpec | OpenBuiltinBrowserSurfaceSpec | OpenUrlTabSpec | OpenUrlAsideSpec"###;

        /// Built-in browser product page. Opening one requires
        /// `capabilities.browser` and is restricted to the home lxapp.
        type BuiltinBrowserSurfaceUrl = r###"'lingxia://settings' | 'lingxia://downloads'"###;

        type OpenBuiltinBrowserSurfaceSpec = r###"{
    url: BuiltinBrowserSurfaceUrl;
    as?: never;
    edge?: never;
    size?: never;
    position?: never;
    interaction?: never;
    page?: never;
    appId?: never;
    envVersion?: never;
    targetVersion?: never;
    key?: never;
    surface?: never;
    query?: never;
}"###;

        /// Open `url` in the multi-tab browser aside. `url` must be `https://` or
        /// `file://` (external content only). Repeated calls add/focus tabs (deduped by
        /// URL) in the single aside per window. Medium/expanded returns a tab-scoped
        /// handle; compact returns `null` because browser chrome owns the group. See
        /// {@link OpenSurfaceSpec} for the full aside contract.
        ///
        type OpenUrlAsideSpec = r###"{
    url: string;
    as: 'aside';
    edge?: SurfaceEdge;
    size?: OverlaySurfaceSize;
    interaction?: never;
    page?: never;
    surface?: never;
    appId?: never;
    envVersion?: never;
    targetVersion?: never;
    key?: never;
    position?: never;
    query?: never;
}"###;

        type OpenUrlTabSpec = r###"{
    url: string;
    as?: never;
    interaction?: never;
    page?: never;
    surface?: never;
    appId?: never;
    envVersion?: never;
    targetVersion?: never;
    key?: never;
    edge?: never;
    position?: never;
    size?: never;
    query?: never;
}"###;

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

        type PageLoadOptions = r###"{
    [key: string]: string | undefined;
}"###;

        type PageMessagePort = r###"{
    postMessage(message: unknown): void;
    onMessage(handler: (message: unknown) => void): () => void;
}"###;

        type PageQuery = r###"Record<string, PageQueryValue>"###;

        type PageQueryValue = r###"string | number | boolean | null | undefined"###;

        type PreviewMediaAdvance = r###"'manual' | 'next' | 'loop'"###;

        /// One change-stream event / the `current` snapshot.
        type PreviewMediaChange = r###"{
    index: number;
    source: PreviewMediaShownSource;
}"###;

        type PreviewMediaCloseReason = r###"'manual' | 'completed' | 'interrupted' | 'error'"###;

        /// Handle returned synchronously from `lx.previewMedia(...)` — synchronous so
        /// listeners can be attached before the first event fires:
        ///
        /// - `presented` resolves once the first pixel of the underlying media has
        ///   been composited to screen. Use this to time the hide of an overlay
        ///   surface above the preview so the swap is seamless. Never rejects;
        ///   resolves with no value when the first frame is up. Safe to ignore.
        /// - `current` is a live `{ index, source }` snapshot of the item on screen,
        ///   updated as the user swipes and as the session auto-advances.
        /// - `onChange(listener)` fires for every item change. Returns an
        ///   unsubscribe function.
        /// - `completed` resolves `{ reason, index, source }` when the preview
        ///   session ends (manual / auto / interrupted / error), or rejects on abort.
        ///
        /// If the call was aborted before any frame was presented, `presented` still
        /// resolves (with no value) once the abort takes effect — it never rejects,
        /// to keep fire-and-forget usage safe.
        ///
        /// @example
        /// const preview = lx.previewMedia({ sources, startIndex: 2 });
        /// preview.onChange(({ source }) => markAsViewed(source.path));
        /// const { reason, source } = await preview.completed;
        ///
        type PreviewMediaHandle = r###"{
    readonly presented: Promise<void>;
    readonly current: PreviewMediaChange;
    onChange(listener: (change: PreviewMediaChange) => void): () => void;
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
     * Default: `0`.
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
     * Whether to show the top `current/total` indicator.
     *
     * Default: `true` when previewing multiple items, otherwise `false`.
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
     * Whether to show the top `current/total` indicator.
     *
     * Default: `true` when previewing multiple items, otherwise `false`.
     */
    showIndexIndicator?: boolean;
}"###;

        type PreviewMediaSource = r###"{
    /**
     * Media source path.
     * Recommended: `lx://` path (for example `lx://usercache/...`) or a sandbox-local path
     * that can be resolved by runtime access rules.
    */
    path: string;
    type?: 'image' | 'video';
    /**
     * Optional clockwise rotation in degrees (`0 | 90 | 180 | 270`).
     * Default: when omitted, runtime resolves orientation from media metadata.
     */
    rotate?: MediaRotation;
    /**
     * Optional display fit mode for video preview.
     * Default: `contain`.
     */
    objectFit?: MediaObjectFit;
    /**
     * Display duration in milliseconds.
     * Effective when preview `advance` is not `manual`.
     */
    durationMs?: number;
}"###;

        type RedirectToOptions = r###"PageTargetOptions"###;

        type ReLaunchOptions = r###"PageTargetOptions"###;

        type FsRemoveOptions = r###"{
    recursive?: boolean;
}"###;

        type FsRenameOptions = r###"{
    /** Defaults to false. */
    overwrite?: boolean;
}"###;

        type SaveMediaOptions = r###"{
    filePath: string;
}"###;

        type ScanCodeOptions = r###"{
    onlyFromCamera?: boolean;
    scanType?: ('barCode' | 'qrCode' | 'datamatrix' | 'pdf417')[];
}"###;

        /// Result of `lx.scanCode`. Branch on `canceled` before reading the scan
        /// payload.
        type ScanCodeResult = r###"{
    canceled: false;
    scanResult: string;
    scanType: string;
} | CanceledResult"###;

        /// Share images, PDFs, or other files.
        ///
        type ShareFilesOptions = r###"ShareTitleOptions & {
    /**
     * File paths returned by LingXia APIs to share. Images, PDFs, and other
     * documents are all represented as file paths.
     *
     * Use `lx.chooseFile` for system files and `lx.chooseMedia` for picked media;
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

        /// Share APIs.
        ///
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

        type ShowActionSheetOptions = r###"{
    itemList: string[];
    itemColor?: string;
}"###;

        type ShowModalOptions = r###"{
    title?: string;
    content?: string;
    showCancel?: boolean;
    cancelText?: string;
    cancelColor?: string;
    confirmText?: string;
    confirmColor?: string;
}"###;

        /// UI feedback, navigation, and surface control APIs.
        ///
        type ShowToastOptions = r###"{
    title: string;
    icon?: 'success' | 'error' | 'loading' | 'none';
    image?: string;
    duration?: number;
    mask?: boolean;
    position?: 'top' | 'center' | 'bottom';
}"###;

        /// Asynchronous persistent key-value storage backed by the lxapp
        /// database. Use `lx.fs` for path-based data.
        ///
        type Storage = r###"{
    /**
     * Reads a stored value. `T` is an unchecked assertion about the stored
     * shape, exactly like a `JSON.parse` boundary; a missing key resolves
     * `undefined`, which a stored `null` never does.
     */
    get<T = unknown>(key: string): Promise<T | undefined>;
    set(key: string, value: unknown): Promise<void>;
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
    duration?: number;
    params?: Record<string, unknown>;
}"###;

        type Surface = r###"SurfaceHandle & {
    /**
     * Last-known visibility, kept in sync with the native side via show/hide
     * events. False once the surface has been closed. Safe to bind into
     * declarative UI; for event-driven updates subscribe via `onShow`/`onHide`.
     */
    readonly visible: boolean;
    /**
     * True until `close()` fires. After close the surface is detached and the
     * page instance is being torn down; further `show()` / `hide()` calls will
     * reject.
     */
    readonly alive: boolean;
    /**
     * Sends a message to the other side of a page surface.
     *
     * For the opener this targets the opened page. For the opened page this
     * targets the opener. URL surfaces have no page-side receiver.
    */
    postMessage(message: unknown): void;
    onMessage(handler: (message: unknown) => void): () => void;
    onClose(handler: (event: SurfaceClosedEvent) => void): () => void;
    /**
     * Fires when the surface transitions to visible, regardless of whether
     * `show()` was called on this side or on the peer. Returns an unsubscribe
     * function. Only fires on real state changes — calling `show()` on an
     * already-visible surface is a no-op for listeners.
     */
    onShow(handler: (event: SurfaceVisibilityEvent) => void): () => void;
    /**
     * Fires when the surface transitions to hidden, regardless of which side
     * triggered it. Returns an unsubscribe function. Only fires on real state
     * changes.
     */
    onHide(handler: (event: SurfaceVisibilityEvent) => void): () => void;
    close(): Promise<void>;
    /**
     * Toggle the surface to visible without tearing it down. The page instance
     * and its state survive a hide / show round-trip — only close() actually
     * destroys the surface and fires the onClose listener. Idempotent: calling
     * on an already-visible surface resolves without firing `onShow`.
     */
    show(): Promise<void>;
    /**
     * Hide the surface without destroying it. The page instance stays mounted,
     * so a subsequent show() restores the same scroll position, form input,
     * and JS state. Hidden surfaces still receive postMessage but are not
     * visible to the user. Idempotent.
     */
    hide(): Promise<void>;
}"###;

        type SurfaceClosedEvent = r###"{
    id: string;
    reason: SurfaceCloseReason;
}"###;

        /// Surfaces (docked asides, floats, windows, browser tabs, declared surfaces)
        /// and the desktop tray — the types behind `lx.openSurface`, `lx.onSurfaceContext`,
        /// and `lx.tray`.
        ///
        type SurfaceCloseReason = r###"'user' | 'programmatic' | 'owner_closed' | 'app_closed' | 'failed'
/**
 * The SDK reclaimed a long-hidden overlay surface for resource reasons.
 * Treat as a normal close: the page instance is gone; further postMessage /
 * show / hide calls will fail. The opener may immediately reopen if needed.
 */
 | 'reclaimed' | 'unknown'"###;

        /// The current surface viewport context, delivered to `lx.onSurfaceContext()`
        /// so an lxapp can self-adapt (e.g. switch column count by `sizeClass`).
        ///
        type SurfaceContext = r###"{
    /** compact (<600) / medium (600–840) / expanded (>840), with hysteresis. */
    sizeClass: 'compact' | 'medium' | 'expanded';
    /** Actual surface viewport width in logical pixels. */
    width: number;
    /** Actual surface viewport height in logical pixels. */
    height: number;
}"###;

        /// Preferred docking side for an aside when the Host has room for a docked
        /// layout. `aside` selects the companion region; `edge` selects a side within
        /// it. Compact Hosts may reproject the same aside as a full-screen overlay.
        type SurfaceEdge = r###"'left' | 'right' | 'top' | 'bottom'"###;

        /// Where a float popup anchors (default `center`).
        type SurfaceFloatPosition = r###"'center' | 'top' | 'bottom' | 'left' | 'right'"###;

        type SurfaceRole = r###"'main' | 'aside' | 'float'"###;

        type SurfacePresentation = r###"'main' | 'dock' | 'overlay' | 'popover' | 'sheet' | 'window'"###;

        type SurfaceHandle = r###"{
    readonly id: string;
    /** Standalone windows have no role in the primary shell graph. */
    readonly role?: SurfaceRole;
    readonly presentation: SurfacePresentation;
    readonly visible: boolean;
    readonly alive: boolean;
    /**
     * Show a host-managed surface. Resolves only after native presentation
     * succeeds and the handle's visibility has been updated.
     */
    show(): Promise<void>;
    /**
     * Hide without destroying user-visible state when the platform supports it.
     * Main surfaces cannot be hidden and reject this operation.
     */
    hide(): Promise<void>;
    /**
     * Destroy the live surface. The stable root main cannot be closed and
     * rejects this operation. Repeated calls after a successful close are
     * idempotent.
     */
    close(): Promise<void>;
    onShow(handler: (event: SurfaceVisibilityEvent) => void): () => void;
    onHide(handler: (event: SurfaceVisibilityEvent) => void): () => void;
    onClose(handler: (event: SurfaceClosedEvent) => void): () => void;
}"###;

        /// Detail payload for `onShow` / `onHide` events. `source` identifies which
        /// Surface object initiated the visibility change so observers can
        /// distinguish self-driven transitions from peer-driven ones (e.g. an opener
        /// UI that wants to update its own button state only when the page side
        /// toggled visibility). `shell` identifies a host-driven main switch.
        ///
        type SurfaceVisibilityEvent = r###"{
    id: string;
    source: 'opener' | 'page' | 'shell';
}"###;

        type SwitchTabOptions = r###"PageTargetOptions"###;

        /// Native system Downloads path. Do not pass this to `lx.fs`.
        type SystemDownloadsPath = r###"string & {
    readonly [systemDownloadsPathBrand]: 'system-downloads-path';
}"###;

        /// Runtime control of the menu-bar (macOS) / system-tray (Windows) status item.
        /// The tray is declared in `lingxia.yaml` (`tray:`); these update its dynamic
        /// content at runtime.
        ///
        /// **Desktop only.** Mobile platforms have no tray, so every method here is a
        /// no-op there (it never throws) — safe to call from portable code. For an
        /// app-icon badge that *is* cross-platform (including mobile), use
        /// `lx.app.setBadge`.
        ///
        type TrayMenuItem = r###"{
    label: string;
    /** Invoked when this item is clicked. */
    onClick?: () => void;
    enabled?: boolean;
    checked?: boolean;
}"###;

        type TrayMenuSeparator = r###"{
    separator: true;
}"###;

        type UpdateFailedInfo = r###"UpdateReadyInfo & {
    error?: string;
}"###;

        /// Callback-based updates for this lxapp's bundle. Available to every
        /// lxapp. To update the native host app, the home lxapp uses the
        /// task-based `lx.app.checkUpdate()` API instead.
        ///
        type UpdateManager = r###"{
    applyUpdate(): void;
    /** Subscribes to a ready update and returns the unsubscribe fn. */
    onUpdateReady(callback: (info: UpdateReadyInfo) => void): () => void;
    /** Subscribes to a failed update and returns the unsubscribe fn. */
    onUpdateFailed(callback: (info: UpdateFailedInfo) => void): () => void;
}"###;

        type UpdateReadyInfo = r###"{
    version?: string;
    isForceUpdate?: boolean;
    channel?: "release" | "preview" | "developer" | string;
}"###;

        type UploadIteratorResult = r###"{
    done: boolean;
    value?: UploadProgressEvent;
}"###;

        type UploadOptions = r###"{
    /** HTTP(S) destination URL. */
    url: string;
    /** Local file path or runtime-managed URI to upload. */
    filePath: string;
    /** Multipart field name. Default: `file`. */
    name?: string;
    /**
     * Optional request headers.
     * Restricted headers such as `Referer` are ignored by the runtime.
     */
    headers?: Record<string, string>;
    /** Optional extra `multipart/form-data` text fields. */
    formData?: Record<string, string>;
    /** Request timeout in milliseconds. */
    timeout?: number;
    /** Override multipart filename. */
    fileName?: string;
    /** Override file MIME type. */
    mimeType?: string;
    /** Optional abort signal. */
    signal?: AbortSignal;
}"###;

        type UploadProgressEvent = r###"{
    kind: 'progress' | 'canceled' | 'completed';
    uploadedBytes?: number;
    totalBytes?: number;
    progress?: number;
    result?: UploadResult;
}"###;

        type UploadResult = r###"{
    /** HTTP status code returned by the server. */
    statusCode: number;
    /** Response body decoded as UTF-8 text. */
    data: string;
}"###;

        type UploadTask = r###"PromiseLike<UploadResult> & AsyncIterable<UploadProgressEvent> & {
    next(): Promise<UploadIteratorResult>;
    /** Stops iteration only. Does not cancel the underlying upload task. */
    return(): Promise<UploadIteratorResult>;
    catch<TResult = never>(onrejected?: ((reason: unknown) => TResult | PromiseLike<TResult>) | null): Promise<UploadResult | TResult>;
    finally(onfinally?: (() => void) | null): Promise<UploadResult>;
    cancel(): Promise<void>;
    wait(): Promise<UploadResult>;
}"###;

        type VideoCompressQuality = r###"'low' | 'medium' | 'high'"###;

        type VideoContext = r###"{
    play(): void;
    pause(): void;
    stop(): void;
    seek(position: number): void;
    requestFullScreen(): void;
    exitFullScreen(): void;
    setStreamSource(options: StreamSourceOptions): void;
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

        type WindowSurfaceSize = r###"{
    /** Initial window width in logical pixels. */
    width?: number;
    /** Initial window height in logical pixels. */
    height?: number;
}"###;

        type FsWriteOptions = r###"{
    /**
     * How string input is interpreted. Strings are UTF-8 by default; `base64`
     * decodes the input into raw bytes before writing.
     */
    encoding?: 'utf8' | 'base64';
    /** Defaults to false. */
    overwrite?: boolean;
}"###;

        /// Target page for `navigateTo`, `redirectTo`, `switchTab`, and `reLaunch`.
        /// JavaScript navigation accepts only the configured page name; full routes
        /// are internal runtime details. Discover names with `lxdev lxapp pages`.
        type PageTargetOptions = r###"{
    /** Configured page name from `lingxia.yaml` / `lxapp.json`. */
    page: string;
    query?: PageQuery;
}"###;

        type NavigateToOptions = "PageTargetOptions";

        // Runtime namespaces are emitted as global interfaces. Re-export their
        // public module types without maintaining a second declaration.
        type AppearanceApi = "globalThis.AppearanceApi";
        type FileSystemApi = "globalThis.FileSystemApi";
        type HostAppApi = "globalThis.HostAppApi";
        type LxEnv = "globalThis.LxEnv";
        type NavigationBarApi = "globalThis.NavigationBarApi";
        type TabBarApi = "globalThis.TabBarApi";
        type TrayApi = "globalThis.TrayApi";

        /// App-owned host-shell chrome. Mutations are available only to the home
        /// lxapp's Logic context; other lxapps receive a permission error.
        type ShellApi = r###"{
    /**
     * Declares runtime actions in the desktop shell's sidebar header or footer.
     * The shell controls layout and only dispatches activation; callbacks own
     * navigation and all other behavior.
     */
    sidebarActions: ShellSidebarActionsApi;
}"###;

        /// Where the host renders a sidebar action on desktop.
        ///
        /// - `header`: icon-only, at most two actions; `label` supplies tooltip
        ///   and accessibility text. Hidden in the compact/collapsed shell.
        /// - `footer`: icon and label in the expanded sidebar, icon-only in the
        ///   compact rail. The host wraps cells and scrolls after five visible
        ///   rows.
        ///
        /// Apps cannot configure cell size, row, weight, color, or selected state.
        /// Where an action lives in the sidebar.
        ///
        /// `header` is the caption row beside the window controls: at most two
        /// actions, for the ones a person reaches for constantly. Declaring a
        /// third rejects the whole `replace` call rather than hiding one.
        ///
        /// `footer` is unbounded and scrolls, and every entry stays visible at
        /// any window size. Anything that must be findable belongs here.
        type ShellSidebarActionPlacement = r###"'header' | 'footer'"###;

        /// One app-declared shell sidebar action. It is a stateless command, not a
        /// selectable navigation item: the shell invokes `onActivate` once and
        /// does not infer a target or active state.
        type ShellSidebarAction = r###"{
    /** Stable, non-empty id; unique across both header and footer actions. */
    id: string;
    /**
     * Initial host-owned region. Use `replace` to move an action. The header
     * takes at most two; everything else belongs in the footer.
     */
    placement: ShellSidebarActionPlacement;
    /**
     * Local lxapp-accessible icon. Use a bundled relative path such as
     * `public/settings.svg`, or an `lx://temp`, `lx://usercache`, or
     * `lx://userdata` path returned by LingXia file APIs. Native absolute paths,
     * parent traversal, `file:` URLs, and network URLs are rejected; download a
     * remote icon before registration. For portable rendering, prefer a square,
     * transparent, monochrome SVG or PNG designed for a 16-point visual.
     */
    icon: string;
    /**
     * Visible footer title and the tooltip/accessibility text for every
     * placement. Long footer labels are kept on one line and tail-truncated.
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
        /// must contain at least one field. Use `replace` to change `placement` or
        /// `onActivate`.
        type ShellSidebarActionUpdate = r###"{
    /** Replacement local icon, with the same path rules as registration. */
    icon?: string;
    /** Replacement non-empty visible/accessibility label. */
    label?: string;
    /** Whether the action remains visible but rejects activation. */
    disabled?: boolean;
}"###;

    }
}
