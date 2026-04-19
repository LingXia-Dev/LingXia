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
    version: string;
    releaseType: 'release' | 'preview' | 'developer';
    cacheDir: string;
  }

  /**
   * Current LxApp information structure
   */
  export interface CurrentLxApp {
    appid: string;
    path: string;
    session_id: number;
  }

  export type BrowserNavigationPolicyDecision = 'in_webview' | 'open_external' | 'deny';

  export interface BrowserNavigationPolicyRequest {
    raw_url: string;
    has_user_gesture?: boolean;
    is_main_frame?: boolean;
  }

  export interface BrowserNavigationPolicyResponse {
    decision: BrowserNavigationPolicyDecision;
    reason?: string | null;
  }

  /**
   * App lifecycle notifications from host UIAbility.
   */
  export function onAppShow(lxappid: string): void;
  export function onAppHide(lxappid: string): void;
  export function onUserCaptureScreen(lxappid: string): void;
  export function onDeviceOrientationChanged(appid: string, session_id: number, value: string): boolean;

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
  export function lingxiaInit(
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
   * Resolve a lx:// URI or sandbox path to a native-consumable URL/path.
   *
   * Returns `file://...` for local filesystem paths, or null if not accessible.
   * Passes through `http(s)://...` unchanged.
   */
  export function resolveLxUri(appid: string, input: string): string | null;

  /**
   * Emit an SDK-side log entry into the Rust log pipeline.
   *
   * level: 0=verbose, 1=debug, 2=info, 3=warn, 4=error.
   * Returns false when the native log pipeline is not initialized or level is invalid.
   */
  export function emitSdkLog(
    level: number,
    category: string,
    appid: string,
    path: string,
    message: string
  ): boolean;

  /**
   * Run the shared browser navigation policy classifier.
   *
   * Returns a decision: in_webview | open_external | deny.
   */
  export function handleBrowserNavigationPolicy(requestJson: string): string | null;

  /**
   * Open or navigate a managed internal browser tab and return tabId.
   */
  export function openBrowserTab(appid: string, sessionId: number, url: string): string | null;

  /**
   * Close a managed internal browser tab.
   */
  export function browserTabClose(tabId: string): boolean;

  /**
   * Get the built-in browser lxapp id.
   */
  export function getBuiltinBrowserAppId(): string;

  /**
   * Resolve managed browser tab path from tabId.
   */
  export function browserTabPathForId(tabId: string): string;

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
   * @param session_id - Runtime session id used to guard stale callbacks
   * @returns Resolved route path
   */
  export function onLxappOpened(appid: string, path: string, session_id: number): string;

  /**
   * Notify that LxApp was closed
   * @param appid - LxApp ID
   * @param session_id - Runtime session id used to guard stale callbacks
   * @returns true when close event matches current session and is accepted
   */
  export function onLxappClosed(appid: string, session_id: number): boolean;

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
   * @returns 1 = handled, 0 = ignored, -1 = rejected
   */
  export function onApplinkReceived(applinkUrl: string): number;

  /**
   * Handle UI events from ArkTS
   * @param appid - LxApp ID
   * @param eventType - UI event type enum
   * @param data - Event data (e.g., tab index, action name)
   * @returns true if handled successfully, false otherwise
   */
  export function onLxappEvent(appid: string, eventType: UiEventType, data: string): boolean;

  /**
   * Get current active LxApp ID and path from Rust stack
   * @returns Current LxApp information
   */
  export function getCurrentLxapp(): CurrentLxApp;

  /**
   * Get runtime session id for a specific LxApp.
   * Returns 0 when not available.
   */
  export function getLxappSessionId(appid: string): number;

  /**
   * Get page orientation for a specific page.
   * Returns: 0=auto, 1=portrait, 2=landscape, 3=reverse-portrait, 4=reverse-landscape
   */
  export function getPageOrientation(appid: string, path: string): number;

  /**
   * Callback function for async operations (modal, etc.)
   * @param id - Callback ID as string for correlating with pending operation
   * @param success - Whether the operation completed successfully
   * @param data - When success=true: JSON payload; when success=false: error code string
   * @returns true if callback was handled successfully, false otherwise
   */
  export function onCallback(id: string, success: boolean, data: string): boolean;
  export function onWebFileChooserRequested(
    requestId: string,
    webtag: string,
    sourceUrl: string,
    acceptTypesJson: string,
    allowMultiple: boolean,
    allowDirectories: boolean,
    capture: boolean
  ): boolean;

  /**
   * Dispatch NativeComponent event payload to Rust runtime.
   * Rust resolves bindings and invokes the target Page({}) function.
   * @param appid - LxApp ID
   * @param path - Page path
   * @param componentId - Native component ID
   * @param eventName - Normalized event name
   * @param payloadJson - Standardized event object as JSON string
   * @param bindingsJson - JSON object map: eventName -> pageFunctionName
   * @returns true if dispatch accepted by runtime
   */
  export function onNativeComponentEvent(
    appid: string,
    path: string,
    componentId: string,
    eventName: string,
    payloadJson: string,
    bindingsJson: string
  ): boolean;

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
   * Store video surface ID without creating/updating AVPlayer (streaming mode)
   * @param componentId - Video component ID
   * @param surfaceId - XComponent surface ID
   * @returns true if stored successfully
   */
  export function videoPlayerStoreSurface(componentId: string, surfaceId: string): boolean;

  /**
   * Clear stored video surface ID
   * @param componentId - Video component ID
   * @returns true if cleared
   */
  export function videoPlayerClearSurface(componentId: string): boolean;

  /**
   * Rebind stream decoder surface (streaming mode)
   * @param componentId - Video component ID
   * @param surfaceId - XComponent surface ID
   * @returns true if rebound successfully
   */
  export function videoPlayerRebindStreamSurface(componentId: string, surfaceId: string): boolean;

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
   * Ask native layer whether current navigation should be intercepted.
   */
  export function onNavigationPolicy(webtag: string, url: string): boolean;

  /**
   * Forward Web download-start event to native layer.
   */
  export function onDownloadStart(
    webtag: string,
    url: string,
    userAgent: string,
    contentDisposition: string,
    mimeType: string,
    contentLength: number
  ): boolean;

  /**
   * Forward main-frame Web load error to native layer.
   */
  export function onLoadError(
    webtag: string,
    url: string,
    errorCode: number,
    description: string
  ): boolean;

  /**
   * Register native host addon (bootstrap hooks, services, JS APIs, etc.)
   * Must be called before lingxiaInit()
   */
  export function lingxiaRegisterHostAddon(): void;

  /** Returns a bitmask of host app capabilities. */
  export function getAppCapabilities(): number;
}
