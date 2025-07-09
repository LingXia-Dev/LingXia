/**
 * TypeScript definitions for liblingxia.so native functions
 * This file provides type definitions for the Rust native functions
 */

import { resourceManager } from '@kit.LocalizationKit';

declare module 'liblingxia.so' {
  /**
   * Initialize LxApp with callback function and system paths
   * @param callbackFunction - Callback function for native-to-JS communication
   * @param dataDir - Data directory path
   * @param cacheDir - Cache directory path
   * @param resourceManager - HarmonyOS resource manager (optional)
   * @returns Home LxApp details in format "appId:path" or null if failed
   */
  export function miniappInit(
    callbackFunction: (name: string, ...args: Object[]) => Object | null,
    dataDir: string,
    cacheDir: string,
    resourceManager: resourceManager.ResourceManager | null
  ): string | null;

  /**
   * Get tab bar configuration for a specific LxApp
   * @param appid - LxApp ID
   * @returns Tab bar configuration as JSON string or null if not found
   */
  export function getTabBarConfig(appid: string): string | null;

  /**
   * Get page configuration for a specific LxApp page
   * @param appid - LxApp ID
   * @param path - Page path
   * @returns Page configuration as JSON string or null if not found
   */
  export function getPageConfig(appid: string, path: string): string | null;

  /**
   * Notify that LxApp was opened
   * @param appid - LxApp ID
   * @param path - Page path
   * @returns Status code (0 for success)
   */
  export function onMiniappOpened(appid: string, path: string): number;

  /**
   * Notify that LxApp was closed
   * @param appid - LxApp ID
   * @returns Status code (0 for success)
   */
  export function onMiniappClosed(appid: string): number;

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
