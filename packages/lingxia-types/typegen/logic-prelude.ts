/* Curated public names used by precise `ts_params` / `ts_return` overrides. */
import type {
  AppScreenshotOptions,
  AppScreenshotResult,
  HostAppEnvVersion,
  HostAppUpdateCheckResult,
} from "../app";
import type { Storage } from "../storage";
import type {
  ConnectWifiOptions,
  DeviceInfo,
  MakePhoneCallOptions,
  NetworkChangeCallback,
  NetworkInfo,
  ScreenInfo,
  WifiConnectedCallback,
  WifiInfo,
} from "../device";
import type { DeviceOrientation, DeviceOrientationChangeEvent } from "../display";
import type { KeyEventCallback } from "../input";
import type { GetLocationOptions, LocationInfo } from "../location";
import type { LxAppInfo as PublicLxAppInfo } from "../lxapp";
import type {
  ChooseDirectoryOptions,
  ChooseDirectoryResult,
  ChooseFileOptions,
  ChooseFileResult,
  FileManager as PublicFileManager,
  OpenFileOptions,
} from "../file";
import type { UploadOptions, UploadTask } from "../transfer";
import type {
  ChooseMediaOptions,
  ChosenMediaEntry,
  CompressImageOptions,
  CompressImageResult,
  CompressVideoOptions,
  CompressVideoTask,
  ExtractVideoThumbnailOptions,
  ExtractVideoThumbnailResult,
  GetImageInfoOptions,
  GetVideoInfoOptions,
  ImageInfo,
  PreviewMediaHandle,
  PreviewMediaOptions,
  SaveMediaOptions,
  ScanCodeOptions,
  ScanCodeResult,
  VideoContext,
  VideoInfo,
} from "../media";
import type { NavigateToLxAppOptions } from "../navigator";
import type { ShareOptions, ShareResult } from "../share";
import type { SystemSettingInfo as PublicSystemSettingInfo } from "../system";
import type { UpdateManager } from "../update";
import type { SurfaceContext } from "../surface";
import type {
  ActionSheetResult,
  CapsuleRect,
  ModalResult,
  NavigateBackOptions,
  NavigateToOptions as PublicNavigateToOptions,
  ReLaunchOptions,
  RedirectToOptions,
  RemoveTabBarBadgeOptions as PublicRemoveTabBarBadgeOptions,
  SetNavigationBarColorOptions as PublicSetNavigationBarColorOptions,
  SetNavigationBarTitleOptions as PublicSetNavigationBarTitleOptions,
  SetTabBarBadgeOptions as PublicSetTabBarBadgeOptions,
  SetTabBarItemOptions as PublicSetTabBarItemOptions,
  SetTabBarStyleOptions as PublicSetTabBarStyleOptions,
  ShowActionSheetOptions,
  ShowModalOptions,
  ShowToastOptions,
  SwitchTabOptions,
  TabBarRedDotOptions,
} from "../ui";
import type { PageMessagePort } from "../app";
import type { TrayMenuItem, TrayMenuSeparator } from "../surface";
