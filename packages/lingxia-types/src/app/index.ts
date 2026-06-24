/**
 * Host app, app lifecycle, and page instance APIs.
 */

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

export type SurfaceCloseReason =
  | 'user'
  | 'programmatic'
  | 'owner_closed'
  | 'app_closed'
  | 'failed'
  /**
   * The SDK reclaimed a long-hidden overlay surface for resource reasons.
   * Treat as a normal close: the page instance is gone; further postMessage /
   * show / hide calls will fail. The opener may immediately reopen if needed.
   */
  | 'reclaimed'
  | 'unknown';

export interface SurfaceClosedEvent {
  id: string;
  kind: 'overlay' | 'window';
  reason: SurfaceCloseReason;
}

/**
 * Detail payload for `onShow` / `onHide` events. `source` identifies which
 * Surface object initiated the visibility change so observers can
 * distinguish self-driven transitions from peer-driven ones (e.g. an opener
 * UI that wants to update its own button state only when the page side
 * toggled visibility).
 */
export interface SurfaceVisibilityEvent {
  id: string;
  kind: 'overlay' | 'window';
  source: 'opener' | 'page';
}

export interface SurfaceHandle {
  readonly id: string;
  /**
   * Show a host-managed surface. Dynamic page/url surfaces return a Promise;
   * host-declared surfaces may complete synchronously.
   */
  show(): void | Promise<void>;
  /**
   * Hide without destroying user-visible state when the platform supports it.
   */
  hide(): void | Promise<void>;
  /**
   * Close or hide the surface depending on how it is managed by the host.
   */
  close(): void | Promise<void>;
}

export interface Surface extends SurfaceHandle {
  readonly kind: 'overlay' | 'window';
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
