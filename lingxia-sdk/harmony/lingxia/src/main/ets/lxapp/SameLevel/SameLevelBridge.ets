/**
 * Bridge for same-level rendering - manages EmbedManagers by webTag
 * Uses official ArkWeb same-level rendering pattern with NodeController
 */
import { webview } from '@kit.ArkWeb';
import { SameLevelRecord, sameLevelError } from './SameLevelTypes';
import { SameLevelEmbedManager } from './SameLevelEmbed';
import { SameLevelNodeController } from './SameLevelNodeController';
import { registerVideoBuilder } from './Builders/VideoBuilder';
import { registerPickerBuilder } from './Builders/PickerBuilder';

// Embed managers by webTag
const webViewEmbedManagers: Map<string, SameLevelEmbedManager> = new Map();

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
export function getOrCreateEmbedManager(webTag: string): SameLevelEmbedManager {
  let manager = webViewEmbedManagers.get(webTag);
  const dispatcher = (payload: SameLevelRecord) => {
    sendEventToJavaScript(webTag, payload);
  };
  if (!manager) {
    registerDefaultBuilders();
    manager = new SameLevelEmbedManager(webTag, dispatcher);
    webViewEmbedManagers.set(webTag, manager);
  } else {
    manager.setEventDispatcher(dispatcher);
  }
  return manager;
}

/**
 * Get existing EmbedManager for a webTag
 */
export function getEmbedManager(webTag: string): SameLevelEmbedManager | undefined {
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
    manager.setEventDispatcher((payload: SameLevelRecord) => {
      sendEventToJavaScript(webTag, payload);
    });
  }
}

/**
 * Send event to JavaScript for a webTag
 */
export function sendEventToJavaScript(webTag: string, payload: SameLevelRecord): void {
  const controller = webViewControllers.get(webTag);
  if (!controller) {
    sameLevelError(`No controller for webTag: ${webTag}`);
    return;
  }

  try {
    const jsonString = `{"type":"event","name":"samelevel","payload":${payload.toJSONString()}}`;
    const script = `
      (function(){
        if (typeof window.__LingXiaRecvMessage === 'function') {
          try { window.__LingXiaRecvMessage(${JSON.stringify(jsonString)}); } catch (e) {}
        }
      })();
    `;
    controller.runJavaScript(script);
  } catch (e) {
    sameLevelError(`Failed to send event to JavaScript: ${e}`);
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
export function getNodeControllerByComponentId(componentId: string): SameLevelNodeController | undefined {
  for (const manager of webViewEmbedManagers.values()) {
    const controller = manager.getNodeController(componentId);
    if (controller) {
      return controller;
    }
  }
  return undefined;
}
