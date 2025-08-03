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
    initialRoute: string;
    appName: string;
    debug: boolean;
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
  }

  /**
   * TabBar configuration structure
   */
  export interface TabBarConfig {
    color: number;
    selectedColor: number;
    backgroundColor: number;
    borderStyle: number;
    position: number;
    dimension: number;
    list: TabBarItem[];
  }

  /**
   * NavigationBar configuration structure
   */
  export interface NavigationBarConfig {
    navigationBarBackgroundColor: number;
    navigationBarTextStyle: string;
    navigationBarTitleText: string;
    navigationStyle: number;
  }

  /**
   * Initialize LxApp with callback function and system paths
   * @param callbackFunction - Callback function for native-to-JS communication
   * @param dataDir - Data directory path
   * @param cacheDir - Cache directory path
   * @param resourceManager - HarmonyOS resource manager (optional)
   * @returns Home LxApp ID or null if failed
   */
  export function lxappInit(
    callbackFunction: (name: string, ...args: Object[]) => Object | null,
    dataDir: string,
    cacheDir: string,
    resourceManager: resourceManager.ResourceManager | null
  ): string | null;

  /**
   * Get LxApp information for a specific app
   * @param appid - LxApp ID
   * @returns LxApp information or null if not found
   */
  export function getLxAppInfo(appid: string): LxAppInfo | null;

  /**
   * Get tab bar configuration for a specific LxApp
   * @param appid - LxApp ID
   * @returns Tab bar configuration or null if not found
   */
  export function getTabBarConfig(appid: string): TabBarConfig | null;

  /**
   * Get page configuration for a specific LxApp page
   * @param appid - LxApp ID
   * @param path - Page path
   * @returns Page configuration
   */
  export function getNavigationBarConfig(appid: string, path: string): NavigationBarConfig;

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
   * Notify that WebView scroll position has changed
   * @param appid - LxApp ID
   * @param path - Page path
   * @param scrollX - Horizontal scroll position
   * @param scrollY - Vertical scroll position
   * @returns Status code (0 for success, -1 for error)
   */
  export function onScrollChanged(appid: string, path: string, scrollX: number, scrollY: number): number;
}
