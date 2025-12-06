/**
 * Same-level rendering types for HarmonyOS
 * Uses official ArkWeb same-level rendering pattern
 */

import { hilog } from '@kit.PerformanceAnalysisKit';

const DOMAIN = 0x0000;
const TAG = 'LingXia.SameLevel';

/**
 * Log helper
 */
export function sameLevelLog(message: string): void {
  hilog.info(DOMAIN, TAG, message);
}

export function sameLevelError(message: string): void {
  hilog.error(DOMAIN, TAG, message);
}

