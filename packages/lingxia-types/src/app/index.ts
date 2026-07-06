/**
 * Host app, app lifecycle, and page instance APIs.
 */

import type { Surface } from '../surface';

export interface AppBaseInfo {
  language: string;
  productName: string;
  version: string;
  SDKVersion: string;
}

export interface HostAppUpdateInfo {
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
   * Direct package handoff is currently supported on Android and macOS. Other
   * platforms reject with an unsupported-operation error; use `version` and
   * `releaseNotes` to guide users to the appropriate app marketplace.
   */
  apply(): HostAppUpdateTask;
}

export type HostAppUpdateApplyStage = 'download' | 'install';

export type HostAppUpdateCheckResult =
  | {
      hasUpdate: false;
      update?: never;
    }
  | {
      hasUpdate: true;
      update: HostAppUpdateInfo;
    };

export type HostAppUpdateEvent =
  | {
      state: 'downloading';
      downloadedBytes?: number;
      progress?: number;
    }
  | {
      state: 'downloaded' | 'installRequested';
    }
  | {
      state: 'failed';
      stage: HostAppUpdateApplyStage;
      error: string;
    };

export interface HostAppUpdateResult {
  state: 'installRequested';
}

export interface HostAppUpdateIteratorResult {
  done: boolean;
  value?: HostAppUpdateEvent;
}

export interface HostAppUpdateTask
  extends PromiseLike<HostAppUpdateResult>,
    AsyncIterable<HostAppUpdateEvent> {
  next(): Promise<HostAppUpdateIteratorResult>;
  /** Stops iteration only. It does not cancel an app update already handed to the platform. */
  return(): Promise<HostAppUpdateIteratorResult>;
  catch<TResult = never>(
    onrejected?: ((reason: unknown) => TResult | PromiseLike<TResult>) | null,
  ): Promise<HostAppUpdateResult | TResult>;
  finally(onfinally?: (() => void) | null): Promise<HostAppUpdateResult>;
  wait(): Promise<HostAppUpdateResult>;
}

/**
 * Build-time environment version of the host app.
 *
 * Surfaced via {@link HostAppApi.envVersion}. Mirrors the
 * `crates/lingxia-update::ReleaseType` enum and the `envVersion` field in the
 * generated `app.json`. Pre-envVersion app artifacts are treated as `'release'`.
 *
 * Note: this is *separate* from `LxAppEnvVersion` in the navigator module,
 * which encodes lxapp release channels (`'develop' | 'preview' | 'release'`)
 * for cross-app navigation URLs and uses the truncated `develop` form.
 */
export type HostAppEnvVersion = 'developer' | 'preview' | 'release';

export interface AppScreenshotOptions {
  /**
   * Platform-specific window id to capture (desktop only). Omit to let the
   * platform pick: the key/main window on desktop, the sole window on mobile.
   */
  windowId?: string;
}

export interface AppScreenshotResult {
  /** `lx://` URI of the captured PNG in the lxapp temp directory. */
  tempFilePath: string;
  /** Image width in pixels, when the runtime could read it from the PNG. */
  width?: number;
  /** Image height in pixels, when the runtime could read it from the PNG. */
  height?: number;
}

/**
 * Launch-at-startup control for the host app.
 *
 * **macOS 13+ / Windows only.** Everywhere else — other platforms, or a
 * macOS shell older than 13 — `lx.app.autostart` is absent (`undefined`);
 * presence is the support check, so portable code gates on the member itself:
 *
 * ```ts
 * if (lx.app.autostart) {
 *   // render the "Launch at startup" toggle
 * }
 * ```
 *
 * Requires `capabilities.autostart: true` in `lingxia.yaml`; without it the
 * member is absent on all platforms. Declaring the capability never enables
 * autostart by itself — the SDK registers the app only when `setEnabled(true)`
 * is called, so the decision stays with the user (typically a settings-page
 * toggle, default off).
 *
 * Host-app-level capability: like `checkUpdate` and `screenshot`, the methods
 * are available only to the home lxapp; other lxapps receive a permission
 * error.
 */
export interface AutostartApi {
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
}

export interface HostAppApi {
  /**
   * The environment this build was produced for, mirroring `app.json::envVersion`.
   * Always present; defaults to `'release'` for builds produced before the field
   * existed in the artifact.
   *
   * Use this to branch on environment-specific behavior (e.g. verbose logging in
   * developer builds, distinct theme tinting, distinct error-reporter tags).
   * Synchronous; the value is fixed at app boot.
   */
  readonly envVersion: HostAppEnvVersion;

  getBaseInfo(): AppBaseInfo;

  /**
   * Check whether the host app has an update.
   *
   * This is a host app level capability and is only available to the home
   * lxapp. Non-home lxapps receive a permission error.
   *
   * Calling this API opts the host app into custom update handling for this
   * process. LingXia will not show its built-in host app update UI or
   * auto-install host app updates after that point.
   *
   * If no update is available, or if an update requires a newer LingXia
   * runtime than the current host app provides, `hasUpdate` is `false`.
   *
   * `checkUpdate()` may return version metadata on platforms that cannot apply
   * host app packages directly. In that case, `update.apply()` rejects with an
   * unsupported-operation error.
   */
  checkUpdate(): Promise<HostAppUpdateCheckResult>;

  /**
   * Exit the host app immediately.
   *
   * This API does not show a confirmation dialog. If the user should confirm
   * first, call `lx.showModal(...)` in page logic and invoke `lx.app.exit()`
   * only after the user confirms.
   */
  exit(): void;

  /**
   * Capture the host app's window as a PNG and save it to the lxapp temp
   * directory.
   *
   * App-level semantics — one level above any page/WebView capture: the image
   * is what the user currently sees of the whole app, including host-drawn
   * navigation chrome, native overlays, and every composited WebView. Because
   * that view can include other lxapps' UI, this API is only available to the
   * home lxapp; other lxapps receive a permission error.
   */
  screenshot(options?: AppScreenshotOptions): Promise<AppScreenshotResult>;

  /**
   * Set the app-icon badge — e.g. an unread count.
   *
   * Cross-platform: the dock (macOS), the taskbar (Windows), and the home /
   * launcher icon (iOS, Android). Pass `null` or an empty string to clear it.
   * On platforms where it is not wired this is a no-op (it never throws), so it
   * is safe to call from portable code.
   */
  setBadge(value: string | number | null): void;

  /**
   * Launch-at-startup control. Present only on macOS / Windows with
   * `capabilities.autostart: true` declared; absent everywhere else — gate on
   * the member: `lx.app.autostart?.setEnabled(on)`. See {@link AutostartApi}.
   */
  autostart?: AutostartApi;
}

export interface AppLifecycleEventArgs {
  source: 'host' | 'lxapp';
  reason:
    | 'foreground'
    | 'background'
    | 'screenshot'
    | 'open'
    | 'close'
    | 'switch_back'
    | 'switch_away';
}

export interface AppLaunchOptions {
  path?: string;
  query?: Record<string, string>;
  scene?: number;
  referrerInfo?: {
    appId?: string;
    extraData?: Record<string, unknown>;
  };
}

export interface AppConfig {
  globalData?: Record<string, unknown>;
  onLaunch?: (options?: AppLaunchOptions) => void | Promise<void>;
  onShow?: (args?: AppLifecycleEventArgs) => void | Promise<void>;
  onHide?: (args?: AppLifecycleEventArgs) => void | Promise<void>;
  onUserCaptureScreen?: () => void | Promise<void>;
  [key: string]: unknown;
}

export interface AppInstance extends AppConfig {
  globalData: Record<string, unknown>;
}

export interface PageLoadOptions {
  [key: string]: string | undefined;
}

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

export interface PageMessagePort {
  postMessage(message: unknown): void;
  onMessage(handler: (message: unknown) => void): () => void;
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
