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
  DownloadOptions,
  DownloadTask,
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
  OpenURLOptions,
} from './system';

import type { NavigateToLxAppOptions } from './navigator';

import type { UpdateManager } from './update';

import type {
  GetImageInfoOptions,
  ImageInfo,
  CompressImageOptions,
  CompressImageResult,
  CompressVideoOptions,
  CompressVideoResult,
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
  SurfaceOpenOptions,
  CapsuleRect,
} from './ui';

export interface SurfaceApi {
  /**
   * Dynamically opens a page or URL in a runtime-managed surface.
   *
   * `overlay` is cross-platform. `window` is a desktop capability and rejects
   * on mobile platforms.
   */
  open(options: SurfaceOpenOptions): Promise<Surface>;
}

export interface Lx {
  env: LxEnv;
  app: HostAppApi;
  surface: SurfaceApi;

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
  downloadFile(options: DownloadOptions): DownloadTask;
  uploadFile(options: UploadOptions): UploadTask;
  getFileManager(): FileManager;

  getStorage(): Storage;

  getLocation(options?: GetLocationOptions): Promise<LocationInfo>;

  navigateToLxApp(options: NavigateToLxAppOptions): Promise<void>;
  navigateBackLxApp(): Promise<void>;

  getLxAppInfo(): LxAppInfo;
  getSystemSetting(): SystemSettingInfo;
  openURL(options: OpenURLOptions): void;

  getUpdateManager(): UpdateManager;

  getImageInfo(options: GetImageInfoOptions): Promise<ImageInfo>;
  compressImage(options: CompressImageOptions): Promise<CompressImageResult>;
  compressVideo(options: CompressVideoOptions): Promise<CompressVideoResult>;
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
   * for the final session result (`reason`, `lastIndex`). Subscribe to
   * `handle.presented` to learn when the first pixel hits the screen — useful
   * for coordinating a hide of any overlay surface that was previously visible.
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
