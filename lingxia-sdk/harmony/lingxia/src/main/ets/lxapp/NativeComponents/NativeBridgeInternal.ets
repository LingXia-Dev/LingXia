/**
 * Bridge for native component rendering - manages EmbedManagers by webTag
 * Uses official ArkWeb same-level rendering pattern with NodeController
 */
import { webview } from '@kit.ArkWeb';
import { NativeComponentRecord, nativeComponentError } from './NativeComponentTypes';
import { NativeComponentEmbedManager } from './NativeComponentEmbed';
import { NativeComponentNodeController } from './NativeComponentNodeController';
import { registerVideoBuilder } from './Builders/VideoBuilder';
import { registerPickerBuilder } from './Builders/PickerBuilder';

// Embed managers by webTag
const webViewEmbedManagers: Map<string, NativeComponentEmbedManager> = new Map();

// Controller references for sending events back to JavaScript
const webViewControllers: Map<string, webview.WebviewController> = new Map();

// Track if default builders are registered
let defaultsRegistered = false;

function registerDefaultBuilders(): void {
  if (defaultsRegistered) return;
  defaultsRegistered = true;
  registerVideoBuilder();
  registerPickerBuilder();
}

// ============================================================================
// Embed Manager APIs
// ============================================================================

/**
 * Get or create an EmbedManager for a webTag
 */
export function getOrCreateEmbedManager(webTag: string): NativeComponentEmbedManager {
  let manager = webViewEmbedManagers.get(webTag);
  const dispatcher = (payload: NativeComponentRecord) => {
    sendEventToJavaScript(webTag, payload);
  };
  if (!manager) {
    registerDefaultBuilders();
    manager = new NativeComponentEmbedManager(webTag, dispatcher);
    webViewEmbedManagers.set(webTag, manager);
  } else {
    manager.setEventDispatcher(dispatcher);
  }
  return manager;
}

/**
 * Get existing EmbedManager for a webTag
 */
export function getEmbedManager(webTag: string): NativeComponentEmbedManager | undefined {
  return webViewEmbedManagers.get(webTag);
}

/**
 * Teardown EmbedManager for a webTag - cleanup all native components
 * Safe to call multiple times - will only teardown once
 */
export function teardownEmbedManager(webTag: string): void {
  const manager = webViewEmbedManagers.get(webTag);
  if (manager) {
    // Remove from maps first to prevent re-entry
    webViewEmbedManagers.delete(webTag);
    webViewControllers.delete(webTag);
    // Then teardown
    manager.teardown();
  }
}

// ============================================================================
// Event Communication
// ============================================================================

/**
 * Set controller for sending events back to JavaScript
 */
export function setEmbedController(webTag: string, controller: webview.WebviewController): void {
  webViewControllers.set(webTag, controller);
  registerDefaultBuilders();
  // Ensure existing manager has dispatcher available
  const manager = webViewEmbedManagers.get(webTag);
  if (manager) {
    manager.setEventDispatcher((payload: NativeComponentRecord) => {
      sendEventToJavaScript(webTag, payload);
    });
  }
}

/**
 * Send event to JavaScript for a webTag
 */
export function sendEventToJavaScript(webTag: string, payload: NativeComponentRecord): void {
  const controller = webViewControllers.get(webTag);
  if (!controller) {
    nativeComponentError(`No controller for webTag: ${webTag}`);
    return;
  }

  try {
    const jsonString = `{"type":"event","name":"nativecomponent","payload":${payload.toJSONString()}}`;
    const script = `
      (function(){
        if (typeof window.__LingXiaRecvMessage === 'function') {
          try { window.__LingXiaRecvMessage(${JSON.stringify(jsonString)}); } catch (e) {}
        }
      })();
    `;
    controller.runJavaScript(script);
  } catch (e) {
    nativeComponentError(`Failed to send event to JavaScript: ${e}`);
  }
}

// ============================================================================
// Lifecycle Management
// ============================================================================

/**
 * Pause all native components for a webTag (e.g., on page navigate away)
 */
export function pauseAllComponents(webTag: string): void {
  const manager = webViewEmbedManagers.get(webTag);
  if (manager) {
    manager.pauseAll();
  }
}

/**
 * Resume all native components for a webTag (e.g., on page navigate back)
 */
export function resumeAllComponents(webTag: string): void {
  const manager = webViewEmbedManagers.get(webTag);
  if (manager) {
    manager.resumeAll();
  }
}

/**
 * Get NodeController by componentId (searches all embed managers)
 */
export function getNodeControllerByComponentId(componentId: string): NativeComponentNodeController | undefined {
  for (const manager of webViewEmbedManagers.values()) {
    const controller = manager.getNodeController(componentId);
    if (controller) {
      return controller;
    }
  }
  return undefined;
}
