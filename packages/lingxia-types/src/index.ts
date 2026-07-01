/**
 * LingXia TypeScript Definitions
 *
 * Type declarations for the LingXia JS API, driven by Rust implementation.
 */

export * from './app';
export * from './lxapp';
export * from './device';
export * from './display';
export * from './input';
export * from './env';
export * from './storage';
export * from './file';
export * from './transfer';
export * from './location';
export * from './navigator';
export * from './system';
export * from './update';
export * from './media';
export * from './share';
export * from './ui';
export * from './error';
export * from './generated/error';
export * from './generated/i18n';

import type {
  AppConfig,
  AppInstance,
  HostAppApi,
  PageMessagePort,
  PageConfig,
  PageInstance,
  Surface,
  SurfaceHandle,
} from './app';

import type {
  LxAppInfo,
} from './lxapp';

import type {
  DeviceInfo,
  ScreenInfo,
  MakePhoneCallOptions,
  WifiInfo,
  ConnectWifiOptions,
  WifiConnectedCallback,
  NetworkInfo,
  NetworkChangeCallback,
} from './device';

import type { KeyEventCallback } from './input';

import type { LxEnv } from './env';

import type { Storage } from './storage';

import type {
  OpenFileOptions,
  ChooseDirectoryOptions,
  ChooseDirectoryResult,
  ChooseFileOptions,
  ChooseFileResult,
  FileManager,
} from './file';

import type {
  AppDownloadOptions,
  AppDownloadResult,
  DownloadDestination,
  DownloadOptions,
  DownloadResultForDestination,
  DownloadTask,
  DownloadsDownloadOptions,
  DownloadsDownloadResult,
  UploadOptions,
  UploadTask,
} from './transfer';

import type {
  DeviceOrientation,
  DeviceOrientationChangeEvent,
} from './display';

import type {
  GetLocationOptions,
  LocationInfo,
} from './location';

import type {
  SystemSettingInfo,
} from './system';

import type { NavigateToLxAppOptions } from './navigator';

import type { UpdateManager } from './update';

import type {
  GetImageInfoOptions,
  ImageInfo,
  CompressImageOptions,
  CompressImageResult,
  CompressVideoOptions,
  CompressVideoTask,
  GetVideoInfoOptions,
  VideoInfo,
  ExtractVideoThumbnailOptions,
  ExtractVideoThumbnailResult,
  ChooseMediaOptions,
  ChosenMediaEntry,
  PreviewMediaHandle,
  PreviewMediaOptions,
  SaveMediaOptions,
  ScanCodeOptions,
  ScanCodeResult,
  VideoContext,
} from './media';

import type {
  ShareOptions,
  ShareResult,
} from './share';

import type {
  ShowToastOptions,
  ShowModalOptions,
  ModalResult,
  ShowActionSheetOptions,
  ActionSheetResult,
  NavigateToOptions,
  NavigateBackOptions,
  RedirectToOptions,
  SwitchTabOptions,
  ReLaunchOptions,
  SetNavigationBarTitleOptions,
  SetNavigationBarColorOptions,
  TabBarRedDotOptions,
  SetTabBarBadgeOptions,
  RemoveTabBarBadgeOptions,
  SetTabBarStyleOptions,
  SetTabBarItemOptions,
  SurfaceEdge,
  SurfaceFloatPosition,
  OverlaySurfaceSize,
  SurfaceContext,
  CapsuleRect,
} from './ui';

interface WindowSurfaceSize {
  /** Initial window width in logical pixels. */
  width?: number;
  /** Initial window height in logical pixels. */
  height?: number;
}

/**
 * Spec for {@link Lx.openSurface}. A discriminated union keyed by source so a
 * page name and a declared surface id never collide (each is its own string
 * space, separately type-checkable).
 *
 * - `{ page }` — one of this lxapp's own pages, by name, arranged as `as`
 *   (`float` is a popup; `window` is a bare desktop window, which rejects on
 *   mobile). `position` applies to `float`, and `size` is a Host-clamped hint.
 *   They are fixed at open (re-open to change). Your own pages **cannot** be
 *   docked as an `aside` — an aside is external content only (see `{ url }`).
 *   For a side panel of your own, use a declared `surface`, an in-page split
 *   layout, or `role: main` for a switchable destination.
 *
 *   `float` is a popup layered above the main at `position` (like a dialog); it
 *   takes no layout space. The SDK gives it **no chrome of its own — there is no
 *   built-in close button**: the lxapp owns the popup UI and dismisses it by
 *   calling `surface.close()` (or `.hide()`). A float sized to the full container
 *   (`size: { width: '100%', height: '100%' }`) presents immersively on mobile
 *   (system bars hidden) and is likewise chrome-less — draw your own close
 *   affordance. (iOS retains a silent left-edge swipe as a last-resort escape so a
 *   full-screen float can never trap the user; don't rely on it as the primary
 *   dismissal.)
 * - `{ surface }` — a surface declared in `lingxia.yaml` `surfaces:`, by id
 *   (e.g. `'terminal'`, `'ai-assistant'`). Form, position, and startup data come
 *   from the declaration.
 * - `{ url }` — external content in the in-app browser. Without `as` it opens as
 *   a main browser tab (the **self** browser: full chrome **with an editable
 *   address bar**, no handle). With `as: 'aside'` it opens in the **browser
 *   aside** — a docked (large screen) / full-screen (phone) **multi-tab** browser
 *   for external content only (`https://` or `file://`).
 *
 *   The aside is **API-only and has no address input** (its one difference from
 *   the self browser): each `openSurface({ url, as: 'aside' })` call opens a tab;
 *   there is no manual "new tab" affordance and the address is never editable.
 *   Tabs are **deduped by URL** — reopening a URL focuses the existing tab and
 *   returns its handle. The handle is **tab-scoped**: `close()` closes that tab,
 *   and closing the last tab closes the aside. The tab strip shows page
 *   **titles** (never the URL), plus per-tab close, back/forward, refresh, and a
 *   close-aside control.
 *
 *   Presentation is the only large/small difference: on `medium` / `expanded`
 *   the aside **docks** and splits beside the main at `edge` (default `'right'`)
 *   with a horizontal title tab strip; on `compact` (phone / runner) it presents
 *   **full-screen** with a **bottom** browser toolbar (tabs reached via a tab
 *   switcher), dismissed by the host back affordance. `size` is a host-clamped
 *   preferred size (large screen only).
 */
export type OpenPageSurfaceSpec =
  | {
      page: string;
      /**
       * A chrome-less popup above the main: the lxapp draws its own UI and close
       * affordance — there is no SDK-provided close button (see
       * {@link OpenPageSurfaceSpec}).
       */
      as: 'float';
      position?: SurfaceFloatPosition;
      size?: OverlaySurfaceSize;
      query?: Record<string, unknown>;
      edge?: never;
      surface?: never;
      url?: never;
    }
  | {
      page: string;
      as: 'window';
      size?: WindowSurfaceSize;
      query?: Record<string, unknown>;
      edge?: never;
      position?: never;
      surface?: never;
      url?: never;
    };

export interface OpenDeclaredSurfaceSpec {
  surface: string;
  page?: never;
  url?: never;
  as?: never;
  edge?: never;
  position?: never;
  size?: never;
  query?: never;
}

export interface OpenUrlTabSpec {
  url: string;
  as?: never;
  page?: never;
  surface?: never;
  edge?: never;
  position?: never;
  size?: never;
  query?: never;
}

/**
 * Open `url` in the multi-tab browser aside. `url` must be `https://` or
 * `file://` (external content only). Repeated calls add/focus tabs (deduped by
 * URL) in the single aside per window; the returned handle is scoped to that
 * tab. See {@link OpenSurfaceSpec} for the full aside contract.
 */
export interface OpenUrlAsideSpec {
  url: string;
  as: 'aside';
  edge?: SurfaceEdge;
  size?: OverlaySurfaceSize;
  page?: never;
  surface?: never;
  position?: never;
  query?: never;
}

export type OpenSurfaceSpec =
  | OpenPageSurfaceSpec
  | OpenDeclaredSurfaceSpec
  | OpenUrlTabSpec
  | OpenUrlAsideSpec;

/**
 * Runtime control of the menu-bar (macOS) / system-tray (Windows) status item.
 * The tray is declared in `lingxia.yaml` (`tray:`); these update its dynamic
 * content at runtime.
 *
 * **Desktop only.** Mobile platforms have no tray, so every method here is a
 * no-op there (it never throws) — safe to call from portable code. For an
 * app-icon badge that *is* cross-platform (including mobile), use
 * {@link HostAppApi.setBadge}.
 */
export interface TrayMenuItem {
  label: string;
  /** Invoked when this item is clicked. */
  onClick?: () => void;
  enabled?: boolean;
  checked?: boolean;
}

export interface TrayMenuSeparator {
  separator: true;
}

export interface TrayApi {
  /** Replace the status-item icon (a resource path). */
  setIcon(icon: string): void;
  /** Set the text shown beside the icon (macOS). Pass `null`/empty to clear. */
  setTitle(text: string | null): void;
  /** Set the badge — e.g. an unread count. Pass `null`/empty to clear. */
  setBadge(value: string | number | null): void;
  /**
   * Replace the right-click dropdown menu. There is no default menu — provide
   * your own items (e.g. `{ label: 'Quit', onClick: () => lx.app.exit() }`).
   *
   * The menu is a snapshot: to change an item's `checked`/`enabled`/`label`
   * state, call `setMenu` again with the full updated array. There is no
   * per-item mutation API.
   */
  setMenu(items: Array<TrayMenuItem | TrayMenuSeparator>): void;
  /**
   * Handle a left-click on the tray icon yourself. While a handler is
   * registered the click runs only the handler — the tray's configured surface
   * `action` is suppressed, so the click is fully yours (e.g. toggle a state and
   * `setIcon`). Returns an unsubscribe function.
   */
  onClick(handler: () => void): () => void;
  /** Show the tray status item. */
  show(): void;
  /** Hide the tray status item (without removing the app). */
  hide(): void;
}

export interface Lx {
  env: LxEnv;
  app: HostAppApi;
  tray: TrayApi;

  /**
   * Open a surface.
   *
   * - `{ url }` without `as` opens a host browser tab and resolves to `null`.
   * - `{ surface }` resolves to a host-managed handle with show/hide/close.
   * - page surfaces and `{ url, as: 'aside' }` resolve to a full Surface handle.
   *
   * `as: 'window'` is desktop-only and rejects on mobile. See
   * {@link OpenSurfaceSpec}.
   */
  openSurface(spec: OpenUrlTabSpec): Promise<null>;
  openSurface(spec: OpenDeclaredSurfaceSpec): Promise<SurfaceHandle>;
  openSurface(spec: OpenPageSurfaceSpec | OpenUrlAsideSpec): Promise<Surface>;
  openSurface(spec: OpenSurfaceSpec): Promise<Surface | SurfaceHandle | null>;
  /** Hand a url off to the OS default browser (leaves the app). */
  openExternal(url: string): void;
  /**
   * Subscribe to adaptive-context changes ({@link SurfaceContext}); the handler
   * fires with the new context. Returns an unsubscribe function.
   */
  onSurfaceContext(handler: (context: SurfaceContext) => void): () => void;

  getDeviceInfo(): DeviceInfo;
  getScreenInfo(): ScreenInfo;
  vibrateShort(): boolean;
  vibrateLong(): boolean;
  makePhoneCall(options: MakePhoneCallOptions): boolean;

  startWifi(): Promise<void>;
  stopWifi(): Promise<void>;
  connectWifi(options: ConnectWifiOptions): Promise<void>;
  getWifiList(): Promise<WifiInfo[]>;
  getConnectedWifi(): Promise<WifiInfo>;
  onWifiConnected(callback: WifiConnectedCallback): void;
  offWifiConnected(callback?: WifiConnectedCallback): void;
  getNetworkInfo(): Promise<NetworkInfo>;
  onNetworkChange(callback: NetworkChangeCallback): void;
  offNetworkChange(callback?: NetworkChangeCallback): void;

  setDeviceOrientation(orientation: DeviceOrientation): boolean;
  onDeviceOrientationChange(callback: (event: DeviceOrientationChangeEvent) => void): void;
  offDeviceOrientationChange(callback?: (event: DeviceOrientationChangeEvent) => void): void;

  /**
   * Open a local file with the requested strategy.
   * Use `mode: 'review'` when the UX requires in-app preview,
   * otherwise prefer `mode: 'auto'`.
   */
  openFile(options: OpenFileOptions): void;
  downloadFile(options: DownloadsDownloadOptions): DownloadTask<DownloadsDownloadResult>;
  downloadFile(options: AppDownloadOptions): DownloadTask<AppDownloadResult>;
  downloadFile<TDestination extends DownloadDestination = 'app'>(
    options: DownloadOptions<TDestination>,
  ): DownloadTask<DownloadResultForDestination<TDestination>>;
  uploadFile(options: UploadOptions): UploadTask;
  getFileManager(): FileManager;

  getStorage(): Storage;

  getLocation(options?: GetLocationOptions): Promise<LocationInfo>;

  navigateToLxApp(options: NavigateToLxAppOptions): Promise<void>;
  navigateBackLxApp(): Promise<void>;

  getLxAppInfo(): LxAppInfo;
  getSystemSetting(): SystemSettingInfo;

  getUpdateManager(): UpdateManager;

  getImageInfo(options: GetImageInfoOptions): Promise<ImageInfo>;
  compressImage(options: CompressImageOptions): Promise<CompressImageResult>;
  compressVideo(options: CompressVideoOptions): CompressVideoTask;
  getVideoInfo(options: GetVideoInfoOptions): Promise<VideoInfo>;
  extractVideoThumbnail(options: ExtractVideoThumbnailOptions): Promise<ExtractVideoThumbnailResult>;

  chooseMedia(options?: ChooseMediaOptions): Promise<ChosenMediaEntry[]>;

  /**
   * Opens native media preview.
   *
   * Supports:
   * - a single source path string for the simplest case
   * - a single-item object for per-item options like `rotate`, `objectFit`, or `durationMs`
   * - a sequence object for multi-item preview with `sources`, `startIndex`, and `advance`
   *
   * Returns a {@link PreviewMediaHandle} synchronously. Await `handle.completed`
   * for the final session result (`reason`, `index`, `source` — the item the
   * user was viewing when the preview closed). Subscribe to
   * `handle.onChange(...)` / read `handle.current` to follow the viewed item
   * live, and to `handle.presented` to learn when the first pixel hits the
   * screen — useful for coordinating a hide of any overlay surface that was
   * previously visible.
   *
   * `handle.completed` rejects with a cancellation error if `signal` is aborted;
   * abort also requests the active native preview session to close immediately.
   * `handle.presented` never rejects.
   */
  previewMedia(options: PreviewMediaOptions): PreviewMediaHandle;

  saveImageToPhotosAlbum(options: SaveMediaOptions): Promise<void>;
  saveVideoToPhotosAlbum(options: SaveMediaOptions): Promise<void>;

  scanCode(options?: ScanCodeOptions): Promise<ScanCodeResult>;

  share(options: ShareOptions): Promise<ShareResult>;

  createVideoContext(componentId: string): VideoContext;

  showToast(options: ShowToastOptions): void;
  hideToast(): void;

  showModal(options: ShowModalOptions): Promise<ModalResult>;

  showActionSheet(options: ShowActionSheetOptions): Promise<ActionSheetResult>;

  navigateTo(options: NavigateToOptions): Promise<PageMessagePort>;
  navigateBack(options: NavigateBackOptions): void;
  redirectTo(options: RedirectToOptions): Promise<void>;
  switchTab(options: SwitchTabOptions): Promise<void>;
  reLaunch(options: ReLaunchOptions): Promise<void>;

  setNavigationBarTitle(options: SetNavigationBarTitleOptions): boolean;
  setNavigationBarColor(options: SetNavigationBarColorOptions): boolean;
  hideHomeButton(): boolean;

  showTabBarRedDot(options: TabBarRedDotOptions): boolean;
  hideTabBarRedDot(options: TabBarRedDotOptions): boolean;
  setTabBarBadge(options: SetTabBarBadgeOptions): boolean;
  removeTabBarBadge(options: RemoveTabBarBadgeOptions): boolean;
  showTabBar(): boolean;
  hideTabBar(): boolean;
  setTabBarStyle(options: SetTabBarStyleOptions): boolean;
  setTabBarItem(options: SetTabBarItemOptions): boolean;

  startPullDownRefresh(): void;
  stopPullDownRefresh(): void;

  getCapsuleRect(): Promise<CapsuleRect>;

  chooseFile(options?: ChooseFileOptions): Promise<ChooseFileResult>;
  chooseDirectory(options?: ChooseDirectoryOptions): Promise<ChooseDirectoryResult>;

  onKeyDown(callback: KeyEventCallback): void;
  offKeyDown(callback?: KeyEventCallback): void;
  onKeyUp(callback: KeyEventCallback): void;
  offKeyUp(callback?: KeyEventCallback): void;
}

declare global {
  const lx: Lx;

  function App(config: AppConfig): AppInstance;
  function getApp<T extends AppInstance = AppInstance>(): T | null;
  function Page<TData extends Record<string, unknown> = Record<string, unknown>>(
    config: PageConfig<TData> & ThisType<PageInstance<TData> & PageConfig<TData>>
  ): void;
  function getCurrentPages<T extends PageInstance = PageInstance>(): T[];
}

export {};
