/**
 * Platform detection utilities.
 * Uses global LingXiaBridge to detect the current platform.
 */

import './types.js';

/**
 * Check if the current platform is HarmonyOS.
 */
export function isHarmony(): boolean {
  return window.LingXiaBridge?.platform?.isHarmony?.() === true;
}

/**
 * Check if the current platform is iOS.
 */
export function isIOS(): boolean {
  return window.LingXiaBridge?.platform?.isIOS?.() === true;
}

/**
 * Check if the current platform is Android.
 */
export function isAndroid(): boolean {
  return window.LingXiaBridge?.platform?.isAndroid?.() === true;
}

/**
 * Get the current platform OS name.
 * Returns "Harmony", "iOS", "Android", or "unknown".
 */
export function getOS(): string {
  return window.LingXiaBridge?.platform?.getOS?.() ?? "unknown";
}

