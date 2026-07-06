/**
 * LingXia TypeScript Definitions
 *
 * Type declarations for the LingXia JS API, driven by Rust implementation.
 */

export * from './app';
export * from './surface';
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
} from './app';

import type {
  Surface,
  SurfaceHandle,
  SurfaceContext,
  TrayApi,
  OpenSurfaceSpec,
  OpenPageSurfaceSpec,
  OpenDeclaredSurfaceSpec,
  OpenUrlTabSpec,
  OpenUrlAsideSpec,
} from './surface';

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
  CapsuleRect,
} from './ui';

/**
 * The global `lx` object — the full Logic-side platform surface, and the
 * authoritative capability index (grouped by the banners below). Mostly flat,
 * with a few nested namespaces (`env`, `app`, `tray`).
 */
export interface Lx {
  // Environment, host app & tray
  env: LxEnv;
  app: HostAppApi;
  tray: TrayApi;

  // Surfaces (asides, floats, windows, browser tabs)
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

  // Device & system info
  getDeviceInfo(): DeviceInfo;
  getScreenInfo(): ScreenInfo;
  vibrateShort(): boolean;
  vibrateLong(): boolean;
  makePhoneCall(options: MakePhoneCallOptions): boolean;

  // WiFi & network
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

  // Display / orientation
  setDeviceOrientation(orientation: DeviceOrientation): boolean;
  onDeviceOrientationChange(callback: (event: DeviceOrientationChangeEvent) => void): void;
  offDeviceOrientationChange(callback?: (event: DeviceOrientationChangeEvent) => void): void;

  // File & transfer
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

  // Storage (key/value)
  getStorage(): Storage;

  // Location
  getLocation(options?: GetLocationOptions): Promise<LocationInfo>;

  // Cross-lxapp navigation
  navigateToLxApp(options: NavigateToLxAppOptions): Promise<void>;
  navigateBackLxApp(): Promise<void>;

  // LxApp & system info
  getLxAppInfo(): LxAppInfo;
  getSystemSetting(): SystemSettingInfo;

  // LxApp bundle update
  getUpdateManager(): UpdateManager;

  // Media
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

  createVideoContext(componentId: string): VideoContext;

  // Share
  share(options: ShareOptions): Promise<ShareResult>;

  // UI feedback
  showToast(options: ShowToastOptions): void;
  hideToast(): void;

  showModal(options: ShowModalOptions): Promise<ModalResult>;

  showActionSheet(options: ShowActionSheetOptions): Promise<ActionSheetResult>;

  // Page navigation
  navigateTo(options: NavigateToOptions): Promise<PageMessagePort>;
  navigateBack(options: NavigateBackOptions): void;
  redirectTo(options: RedirectToOptions): Promise<void>;
  switchTab(options: SwitchTabOptions): Promise<void>;
  reLaunch(options: ReLaunchOptions): Promise<void>;

  // Navigation bar / home button
  setNavigationBarTitle(options: SetNavigationBarTitleOptions): boolean;
  setNavigationBarColor(options: SetNavigationBarColorOptions): boolean;
  hideHomeButton(): boolean;

  // Tab bar
  showTabBarRedDot(options: TabBarRedDotOptions): boolean;
  hideTabBarRedDot(options: TabBarRedDotOptions): boolean;
  setTabBarBadge(options: SetTabBarBadgeOptions): boolean;
  removeTabBarBadge(options: RemoveTabBarBadgeOptions): boolean;
  showTabBar(): boolean;
  hideTabBar(): boolean;
  setTabBarStyle(options: SetTabBarStyleOptions): boolean;
  setTabBarItem(options: SetTabBarItemOptions): boolean;

  // Pull-down refresh
  startPullDownRefresh(): void;
  stopPullDownRefresh(): void;

  // Capsule
  getCapsuleRect(): Promise<CapsuleRect>;

  // File / directory picker
  chooseFile(options?: ChooseFileOptions): Promise<ChooseFileResult>;
  chooseDirectory(options?: ChooseDirectoryOptions): Promise<ChooseDirectoryResult>;

  // Keyboard / hardware keys (TV / desktop)
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
