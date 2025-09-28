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
    BackPress = 3
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
  export function onLxappOpened(appid: string, path: string): number;

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
   * @param id - Callback ID as string
   * @param success - Whether the operation was successful
   * @param data - Result data as JSON string
   * @returns true if callback was handled successfully, false otherwise
   */
  export function onCallback(id: string, success: boolean, data: string): boolean;
}
