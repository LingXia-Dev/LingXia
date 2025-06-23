/**
 * TypeScript definitions for liblingxia.so native functions
 * This file provides type definitions for the Rust native functions
 */

import { resourceManager } from '@kit.LocalizationKit';

declare module 'liblingxia.so' {
  /**
   * Initialize MiniApp with callback function and system paths
   * @param callbackFunction - Callback function for native-to-JS communication
   * @param dataDir - Data directory path
   * @param cacheDir - Cache directory path
   * @param resourceManager - HarmonyOS resource manager (optional)
   * @returns Home MiniApp details in format "appId:path" or null if failed
   */
  export function miniappInit(
    callbackFunction: (name: string, ...args: Object[]) => Object | null,
    dataDir: string,
    cacheDir: string,
    resourceManager: resourceManager.ResourceManager | null
  ): string | null;

  /**
   * Get tab bar configuration for a specific MiniApp
   * @param appid - MiniApp ID
   * @returns Tab bar configuration as JSON string or null if not found
   */
  export function getTabBarConfig(appid: string): string | null;

  /**
   * Get page configuration for a specific MiniApp page
   * @param appid - MiniApp ID
   * @param path - Page path
   * @returns Page configuration as JSON string or null if not found
   */
  export function getPageConfig(appid: string, path: string): string | null;

  /**
   * Notify that MiniApp was opened
   * @param appid - MiniApp ID
   * @param path - Page path
   * @returns Status code (0 for success)
   */
  export function onMiniappOpened(appid: string, path: string): number;

  /**
   * Notify that MiniApp was closed
   * @param appid - MiniApp ID
   * @returns Status code (0 for success)
   */
  export function onMiniappClosed(appid: string): number;
}
