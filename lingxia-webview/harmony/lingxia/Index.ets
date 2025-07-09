/**
 * HAR package main entry point
 * LingXia LxApp SDK for HarmonyOS
 *
 * PUBLIC API:
 * - LxApp.initialize() - Initialize the LxApp SDK (call in onWindowStageCreate)
 * - LxApp.openHomeLxApp() - Open the home LxApp (call in page component)
 * - LxAppNavigation - Navigation-based architecture
 *
 * OPTION 1 - Auto Mode (Recommended):
 * ```typescript
 * // In EntryAbility.ets onWindowStageCreate():
 * LxApp.initialize(this.context, windowStage);
 *
 * // In your main page:
 * LxAppNavigation()  // Auto opens home LxApp internally
 * ```
 *
 * OPTION 2 - Manual Mode (More control):
 * ```typescript
 * // In EntryAbility.ets onWindowStageCreate():
 * LxApp.initialize(this.context, windowStage);
 *
 * // In your main page aboutToAppear():
 * LxApp.openHomeLxApp();
 *
 * // In your main page build():
 * LxAppNavigation({ autoOpenHome: false })
 * ```
 */

export { LxApp } from './src/main/ets/lxapp/LxApp';
export { LxAppNavigation } from './src/main/ets/lxapp/LxAppNavigation';
