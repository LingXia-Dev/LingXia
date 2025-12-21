/**
 * TypeScript definitions for liblingxia.so native functions
 * This file provides type definitions for the Rust native functions
 */

import { resourceManager } from '@kit.LocalizationKit';

declare module 'liblingxia.so' {
  /**
   * LxApp information structure
   */
  export interface LxAppInfo {
    appName: string;
    cacheDir: string;
  }

  /**
   * Current LxApp information structure
   */
  export interface CurrentLxApp {
    appid: string;
    path: string;
  }

  /**
   * TabBar position enum
   */
  export enum TabBarPosition {
    Bottom = 0,
    Left = 1,
    Right = 2
  }

  /**
   * UI event type enum
   */
  export enum UiEventType {
    TabBarClick = 0,
    CapsuleClick = 1,
    NavigationClick = 2,
    BackPress = 3,
    PullDownRefresh = 4
  }

  /**
   * TabBar item structure
   */
  export interface TabBarItem {
    pagePath: string;
    text: string;
    iconPath: string;
    selectedIconPath: string;
    selected: boolean;
    group: number;          // 0=center (default), 1=start, 2=end
    badge?: string;         // Badge text (e.g., "99+", "NEW")
    hasRedDot?: boolean;    // Show red dot indicator
  }

  /**
   * TabBar state structure with items array
   */
  export interface TabBarState {
    color: number;
    selectedColor: number;
    backgroundColor: number;
    borderStyle: number;
    position: TabBarPosition;
    dimension: number;
    isVisible: boolean;
    items: TabBarItem[];
    selectedIndex: number;
  }

  /**
   * NavigationBar state structure
   */
  export interface NavigationBarState {
    navigationBarBackgroundColor: number;
    navigationBarTextStyle: string;
    navigationBarTitleText: string;
    showNavbar: boolean;
    showBackButton: boolean;
    showHomeButton: boolean;
  }

  /**
   * Check if pull-down refresh is enabled for a specific page
   */
  export function isPullDownRefreshEnabled(appid: string, path: string): boolean;

  /**
   * Register custom schemes (must be called before WebEngine initialization)
   * @returns true if schemes registered successfully, false otherwise
   */
  export function registerCustomSchemes(): boolean;

  /**
   * Initialize LxApp with callback function and system paths
   * @param callbackFunction - Callback function for native-to-JS communication
   * @param dataDir - Data directory path
   * @param cacheDir - Cache directory path
   * @param resourceManager - HarmonyOS resource manager (optional)
   * @param locale - System locale (e.g., "zh-CN", "en-US")
   * @returns Home LxApp ID or null if failed
   */
  export function lxappInit(
    callbackFunction: (name: string, ...args: Object[]) => Object | null,
    dataDir: string,
    cacheDir: string,
    resourceManager: resourceManager.ResourceManager | null,
    locale: string
  ): string | null;

  /**
   * Get LxApp information for a specific app
   * @param appid - LxApp ID
   * @returns LxApp information or null if not found
   */
  export function getLxAppInfo(appid: string): LxAppInfo | null;

  /**
   * Get TabBar state for a specific LxApp with complete items array
   * @param appid - LxApp ID
   * @returns TabBar state or null if not found
   */
  export function getTabBar(appid: string): TabBarState | null;

  /**
   * Get navigation bar state for a specific LxApp page
   * @param appid - LxApp ID
   * @param path - Page path
   * @returns NavigationBar state with boolean controls
   */
  export function getNavigationBarState(appid: string, path: string): NavigationBarState;

  /**
   * Notify that LxApp was opened
   * @param appid - LxApp ID
   * @param path - Page path
   * @returns Status code (0 for success)
   */
  // Returns resolved path string
  export function onLxappOpened(appid: string, path: string): string;

  /**
   * Notify that LxApp was closed
   * @param appid - LxApp ID
   * @returns Status code (0 for success)
   */
  export function onLxappClosed(appid: string): number;

  /**
   * Notify that a page is being shown (WebView becomes visible)
   * @param appid - LxApp ID
   * @param path - Page path
   * @returns Status code (0 for success, -1 for error)
   */
  export function onPageShow(appid: string, path: string): number;

  /**
   * Handle AppLink (https) received by platform
   * @param applinkUrl - Full https URL
   * @returns Status code (0 for success)
   */
  export function onApplinkReceived(applinkUrl: string): number;

  /**
   * Handle UI events from ArkTS
   * @param appid - LxApp ID
   * @param eventType - UI event type enum
   * @param data - Event data (e.g., tab index, action name)
   * @returns true if handled successfully, false otherwise
   */
  export function onUiEvent(appid: string, eventType: UiEventType, data: string): boolean;

  /**
   * Get current active LxApp ID and path from Rust stack
   * @returns Current LxApp information
   */
  export function getCurrentLxapp(): CurrentLxApp;

  /**
   * Callback function for async operations (modal, etc.)
   * @param id - Callback ID as string for correlating with pending operation
   * @param success - Whether the operation completed successfully
   * @param data - When success=true: JSON payload; when success=false: error code string
   * @returns true if callback was handled successfully, false otherwise
   */
  export function onCallback(id: string, success: boolean, data: string): boolean;

  /**
   * Push: deliver device token from ArkTS to native
   * @param token - Push device token string
   * @returns 0 on success
   */
  export function onPushTokenReceived(token: string): number;

  /**
   * Push: deliver push link/message to native with trigger
   * @param url - App link or message target URL
   * @param trigger - 0=Background, 1=Tap, 2=Launch
   * @returns 0 on success
   */
  export function onPushlinkReceived(url: string, trigger: number): number;

  /**
   * Initialize camera with surface ID and facing preference
   * @param surfaceId - XComponent surface ID for camera preview
   * @param facing - Camera facing preference ("front" or "back")
   * @returns true if initialization successful, false otherwise
   */
  export function cameraInit(surfaceId: string, facing: string): boolean;

  /**
   * Release camera resources and stop preview
   */
  export function cameraRelease(): void;

  /**
   * Switch camera facing
   * @param isBack - true for back camera, false for front camera
   * @returns true if switch successful, false otherwise
   */
  export function cameraSwitchFacing(isBack: boolean): boolean;

  /**
   * Set camera flash mode
   * @param flashOn - true to enable flash, false to disable
   * @returns true if flash mode was set successfully
   */
  export function cameraSetFlashMode(flashOn: boolean): boolean;

  /**
   * Take a photo (notifies via callback when done)
   * @returns true if capture started successfully
   */
  export function cameraTakePhoto(): boolean;


  /**
   * Start photo capture with dedicated surface, callback and cache dir
   * @param surfaceId - Surface ID for photo capture
   * @param callbackId - Callback ID for photo result
   * @param cacheDir - Directory to save captured photo
   * @returns true if photo capture setup successful, false otherwise
   */
  export function cameraStartPhotoWithSurface(surfaceId: string, callbackId: string, cacheDir: string): boolean;

  /**
   * Start video recording with dedicated surface
   * @param surfaceId - Surface ID for video recording
   * @returns true if video recording setup successful, false otherwise
   */
  export function cameraStartVideoWithSurface(surfaceId: string): boolean;

  /**
   * Start video output recording
   * @returns true if video output started successfully, false otherwise
   */
  export function cameraVideoOutputStart(): boolean;

  /**
   * Stop video output and release resources
   * @returns true if video output stopped successfully, false otherwise
   */
  export function cameraVideoOutputStopAndRelease(): boolean;

  /**
   * Notify native layer that a WebView controller finished creation.
   */
  export function onWebviewControllerCreated(webtag: string): boolean;

  /**
   * Notify native layer that a WebView controller finished destruction.
   */
  export function onWebviewControllerDestroyed(webtag: string): boolean;

  /**
   * Register user extensions (cloud provider, JS extensions, etc.)
   * Must be called before lxappInit()
   */
  export function lingxiaRegisterExtensions(): void;
}
