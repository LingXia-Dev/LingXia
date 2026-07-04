/**
 * HAR package main entry point
 * LingXia SDK for HarmonyOS
 *
 * PUBLIC API:
 * - Lingxia.quickStart() - Initialize the SDK for product apps (call in onWindowStageCreate)
 * - LxAppNavigation - Navigation-based architecture with automatic home LxApp opening
 * - LxLog - Route host-side SDK logs into LingXia dev log streams
 *
 * USAGE:
 * ```typescript
 * // In EntryAbility.ets onWindowStageCreate():
 * Lingxia.quickStart(this.context, windowStage);
 *
 * // In your main page:
 * LxAppNavigation()  // Automatically opens home LxApp after initialization
 * ```
 */

export { Lingxia } from './src/main/ets/app/Lingxia';
export { LxApp } from './src/main/ets/lxapp/LxApp';
export { LxAppNavigation } from './src/main/ets/lxapp/LxAppNavigation';
export { LingxiaBaseAbility } from './src/main/ets/app/LingxiaBaseAbility';
export { LxLog } from './src/main/ets/app/LxLog';
