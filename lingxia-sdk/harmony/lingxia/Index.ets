/**
 * HAR package main entry point
 * LingXia SDK for HarmonyOS
 *
 * PUBLIC API:
 * - Lingxia.quickStart() - Initialize the SDK for product apps (call in onWindowStageCreate)
 * - LxAppNavigation - Navigation-based architecture with automatic home LxApp opening
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

export { Lingxia } from './src/main/ets/lxapp/Lingxia';
export { LxApp } from './src/main/ets/lxapp/LxApp';
export { LxAppNavigation } from './src/main/ets/lxapp/LxAppNavigation';
export { LingxiaBaseAbility } from './src/main/ets/lxapp/LingxiaBaseAbility';
