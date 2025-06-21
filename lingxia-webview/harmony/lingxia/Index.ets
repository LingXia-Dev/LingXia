/**
 * HAR package main entry point
 * LingXia MiniApp SDK for HarmonyOS
 *
 * PUBLIC API:
 * - MiniApp.initialize() - Initialize the MiniApp SDK (call in onWindowStageCreate)
 * - MiniApp.openHomeMiniApp() - Open the home MiniApp (call in page component)
 * - MiniAppNavigation - Navigation-based architecture
 *
 * OPTION 1 - Auto Mode (Recommended):
 * ```typescript
 * // In EntryAbility.ets onWindowStageCreate():
 * MiniApp.initialize(this.context, windowStage);
 *
 * // In your main page:
 * MiniAppNavigation()  // Auto opens home MiniApp internally
 * ```
 *
 * OPTION 2 - Manual Mode (More control):
 * ```typescript
 * // In EntryAbility.ets onWindowStageCreate():
 * MiniApp.initialize(this.context, windowStage);
 *
 * // In your main page aboutToAppear():
 * MiniApp.openHomeMiniApp();
 *
 * // In your main page build():
 * MiniAppNavigation({ autoOpenHome: false })
 * ```
 */

export { MiniApp } from './src/main/ets/miniapp/MiniApp';
export { MiniAppNavigation } from './src/main/ets/miniapp/MiniAppNavigation';