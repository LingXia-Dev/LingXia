/**
 * HAR package main entry point
 * LingXia LxApp SDK for HarmonyOS
 *
 * PUBLIC API:
 * - LxApp.initialize() - Initialize the LxApp SDK (call in onWindowStageCreate)
 * - LxAppNavigation - Navigation-based architecture with automatic home LxApp opening
 *
 * USAGE:
 * ```typescript
 * // In EntryAbility.ets onWindowStageCreate():
 * LxApp.initialize(this.context, windowStage);
 *
 * // In your main page:
 * LxAppNavigation()  // Automatically opens home LxApp after initialization
 * ```
 */

export { LxApp } from './src/main/ets/lxapp/LxApp';
export { LxAppNavigation } from './src/main/ets/lxapp/LxAppNavigation';
export { LingxiaBaseAbility } from './src/main/ets/lxapp/LingxiaBaseAbility';
export { LxAppToast, ToastManager, ToastIcon, ToastPosition, ToastConfig } from './src/main/ets/lxapp/LxAppToast';
export { LxAppModal, ModalManager, ModalConfig, ModalResult } from './src/main/ets/lxapp/LxAppModal';
