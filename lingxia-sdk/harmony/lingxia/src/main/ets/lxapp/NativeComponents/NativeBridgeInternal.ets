/**
 * Bridge for native component rendering - manages EmbedManagers by webTag
 * Uses official ArkWeb same-level rendering pattern with NodeController
 */
import { webview } from '@kit.ArkWeb';
import { NativeComponentRecord, NativeComponentValue, nativeComponentError, nativeComponentLog } from './NativeComponentTypes';
import { NativeComponentEmbedManager } from './NativeComponentEmbed';
import { NativeComponentNodeController } from './NativeComponentNodeController';
import { registerVideoBuilder } from './Builders/VideoBuilder';
import { registerPickerBuilder } from './Builders/PickerBuilder';
import { registerInputBuilder } from './Builders/InputBuilder';
import { registerTextareaBuilder } from './Builders/TextareaBuilder';

// Embed managers by webTag
const webViewEmbedManagers: Map<string, NativeComponentEmbedManager> = new Map();
const webViewControllers: Map<string, webview.WebviewController> = new Map();
const componentManagerCache: Map<string, NativeComponentEmbedManager> = new Map();

class PendingPropsUpdate {
  webTag: string = '';
  componentId: string = '';
  propsJson: string = '';
  retries: number = 0;
}

const pendingPropsUpdates: Map<string, PendingPropsUpdate> = new Map();
let pendingPropsFlushTimer: number | null = null;
const PENDING_PROPS_RETRY_MS = 16;
const MAX_PENDING_PROPS_RETRIES = 50;
const INACTIVE_COMPONENT_STOP_DELAY_MS = 60_000;
const pendingStopTimers: Map<string, number> = new Map();

function pendingKey(webTag: string, componentId: string): string {
  return `${webTag}::${componentId}`;
}

function createPendingPropsUpdate(
  webTag: string,
  componentId: string,
  propsJson: string
): PendingPropsUpdate {
  const update = new PendingPropsUpdate();
  update.webTag = webTag;
  update.componentId = componentId;
  update.propsJson = propsJson;
  return update;
}

function enqueuePendingPropsUpdate(webTag: string, componentId: string, propsJson: string): void {
  const key = pendingKey(webTag, componentId);
  pendingPropsUpdates.set(key, createPendingPropsUpdate(webTag, componentId, propsJson));
  schedulePendingPropsFlush();
}

function schedulePendingPropsFlush(): void {
  if (pendingPropsFlushTimer !== null || pendingPropsUpdates.size === 0) {
    return;
  }
  pendingPropsFlushTimer = setTimeout(() => {
    pendingPropsFlushTimer = null;
    flushPendingPropsUpdates();
  }, PENDING_PROPS_RETRY_MS);
}

function flushPendingPropsUpdates(): void {
  if (pendingPropsUpdates.size === 0) {
    return;
  }

  const keys: string[] = [];
  for (const key of pendingPropsUpdates.keys()) {
    keys.push(key);
  }
  for (let i = 0; i < keys.length; i++) {
    const key = keys[i];
    const pending = pendingPropsUpdates.get(key);
    if (!pending) {
      continue;
    }
    const manager = resolveEmbedManager(pending.webTag);
    if (!manager) {
      pending.retries += 1;
      if (pending.retries >= MAX_PENDING_PROPS_RETRIES) {
        pendingPropsUpdates.delete(key);
      }
      continue;
    }

    const updated = manager.updateComponentProps(pending.componentId, pending.propsJson);
    if (updated) {
      pendingPropsUpdates.delete(key);
    } else {
      pending.retries += 1;
      if (pending.retries >= MAX_PENDING_PROPS_RETRIES) {
        pendingPropsUpdates.delete(key);
      }
    }
  }

  if (pendingPropsUpdates.size > 0) {
    schedulePendingPropsFlush();
  }
}

// Track if default builders are registered
let defaultsRegistered = false;

function registerDefaultBuilders(): void {
  if (defaultsRegistered) return;
  defaultsRegistered = true;
  registerVideoBuilder();
  registerPickerBuilder();
  registerInputBuilder();
  registerTextareaBuilder();
}

function resolveLogicalWebTag(webTag: string): string {
  const logicalTag = webTag.split('#')[0];
  if (logicalTag && logicalTag.length > 0) {
    return logicalTag;
  }
  return webTag;
}

function cancelDelayedStop(webTag: string): void {
  const timer = pendingStopTimers.get(webTag);
  if (timer !== undefined) {
    clearTimeout(timer);
    pendingStopTimers.delete(webTag);
  }
}

function cancelDelayedStopForTag(webTag: string): void {
  cancelDelayedStop(webTag);
  const logicalTag = resolveLogicalWebTag(webTag);
  if (logicalTag !== webTag) {
    cancelDelayedStop(logicalTag);
  }
}

function scheduleDelayedStop(webTag: string): void {
  cancelDelayedStop(webTag);
  const timer = setTimeout(() => {
    pendingStopTimers.delete(webTag);
    const manager = webViewEmbedManagers.get(webTag);
    if (manager) {
      manager.stopAll();
    }
  }, INACTIVE_COMPONENT_STOP_DELAY_MS);
  pendingStopTimers.set(webTag, timer);
}

// ============================================================================
// Embed Manager APIs
// ============================================================================

/**
 * Get or create an EmbedManager for a webTag
 */
export function getOrCreateEmbedManager(webTag: string): NativeComponentEmbedManager {
  let manager = webViewEmbedManagers.get(webTag);
  const dispatcher = (payload: NativeComponentRecord): void => {
    sendEventToJavaScript(webTag, payload);
  };
  if (!manager) {
    registerDefaultBuilders();
    manager = new NativeComponentEmbedManager(webTag, dispatcher);
    webViewEmbedManagers.set(webTag, manager);
  } else {
    manager.setEventDispatcher(dispatcher);
  }
  flushPendingPropsUpdates();
  return manager;
}

/**
 * Get existing EmbedManager for a webTag
 */
export function getEmbedManager(webTag: string): NativeComponentEmbedManager | undefined {
  return webViewEmbedManagers.get(webTag);
}

export function getFocusedNativeComponentType(webTag: string): string {
  const manager = resolveEmbedManager(webTag);
  if (!manager) {
    return '';
  }
  return manager.getFocusedComponentType();
}

export function getFocusedNativeComponentAdjustPosition(webTag: string): boolean {
  const manager = resolveEmbedManager(webTag);
  if (!manager) {
    return true;
  }
  return manager.getFocusedAdjustPosition();
}

function resolveManagerKey(webTag: string): string | undefined {
  if (webViewEmbedManagers.has(webTag)) {
    return webTag;
  }

  const logicalTag = resolveLogicalWebTag(webTag);
  if (logicalTag !== webTag && webViewEmbedManagers.has(logicalTag)) {
    return logicalTag;
  }
  return undefined;
}

function resolveEmbedManager(webTag: string): NativeComponentEmbedManager | undefined {
  const key = resolveManagerKey(webTag);
  if (!key) {
    return undefined;
  }
  return webViewEmbedManagers.get(key);
}

function resolveEmbedManagerByComponentId(componentId: string): NativeComponentEmbedManager | undefined {
  const cached = componentManagerCache.get(componentId);
  if (cached?.hasComponent(componentId)) {
    return cached;
  }
  componentManagerCache.delete(componentId);
  for (const manager of webViewEmbedManagers.values()) {
    if (manager.hasComponent(componentId)) {
      componentManagerCache.set(componentId, manager);
      return manager;
    }
  }
  return undefined;
}

function removePendingPropsForTag(webTag: string): void {
  const logicalTag = resolveLogicalWebTag(webTag);
  const keys: string[] = [];
  for (const key of pendingPropsUpdates.keys()) {
    keys.push(key);
  }
  for (let i = 0; i < keys.length; i++) {
    const key = keys[i];
    if (key.startsWith(`${webTag}::`)) {
      pendingPropsUpdates.delete(key);
      continue;
    }
    if (logicalTag !== webTag && key.startsWith(`${logicalTag}::`)) {
      pendingPropsUpdates.delete(key);
    }
  }
}

/**
 * Teardown EmbedManager for a webTag - cleanup all native components
 * Safe to call multiple times - will only teardown once
 */
export function teardownEmbedManager(webTag: string): void {
  cancelDelayedStopForTag(webTag);
  const resolvedKey = resolveManagerKey(webTag);
  if (resolvedKey) {
    const manager = webViewEmbedManagers.get(resolvedKey);
    if (manager) {
      // Remove from maps first to prevent re-entry
      webViewEmbedManagers.delete(resolvedKey);
      webViewControllers.delete(resolvedKey);
      const logicalTag = resolveLogicalWebTag(resolvedKey);
      if (logicalTag !== resolvedKey) {
        webViewControllers.delete(logicalTag);
      }
      const cacheKeys: string[] = [];
      for (const componentId of componentManagerCache.keys()) {
        cacheKeys.push(componentId);
      }
      for (let i = 0; i < cacheKeys.length; i++) {
        const componentId = cacheKeys[i];
        const cached = componentManagerCache.get(componentId);
        if (cached === manager) {
          componentManagerCache.delete(componentId);
        }
      }
      // Then teardown
      manager.teardown();
    }
  }
  removePendingPropsForTag(webTag);
  if (pendingPropsUpdates.size === 0 && pendingPropsFlushTimer !== null) {
    clearTimeout(pendingPropsFlushTimer);
    pendingPropsFlushTimer = null;
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
  const logicalTag = resolveLogicalWebTag(webTag);
  if (logicalTag !== webTag) {
    webViewControllers.set(logicalTag, controller);
  }
  registerDefaultBuilders();
  const manager = resolveEmbedManager(webTag);
  if (manager) {
    manager.setEventDispatcher((payload: NativeComponentRecord): void => {
      sendEventToJavaScript(webTag, payload);
    });
  }
  flushPendingPropsUpdates();
}

function resolveController(webTag: string): webview.WebviewController | undefined {
  const key = resolveManagerKey(webTag);
  if (key && webViewControllers.has(key)) {
    return webViewControllers.get(key);
  }
  if (webViewControllers.has(webTag)) {
    return webViewControllers.get(webTag);
  }
  const logicalTag = resolveLogicalWebTag(webTag);
  if (webViewControllers.has(logicalTag)) {
    return webViewControllers.get(logicalTag);
  }
  return undefined;
}

function sendEventToJavaScript(webTag: string, payload: NativeComponentRecord): void {
  const controller = resolveController(webTag);
  if (!controller) {
    return;
  }
  try {
    const jsonString = `{"type":"event","name":"nativecomponent","payload":${payload.toJSONString()}}`;
    const script = `
      (function(){
        try { window.__LingXiaRecvMessage(${JSON.stringify(jsonString)}); } catch (e) {}
      })();
    `;
    controller.runJavaScript(script);
  } catch (e) {
    nativeComponentError(`Failed to send event to JavaScript: ${e}`);
  }
}

/**
 * Push latest native component props from Web to ArkTS.
 * This is the primary Harmony update path and avoids WebView DOM polling.
 */
export function updateNativeComponentProps(
  webTag: string,
  componentId: string,
  propsJson: string
): boolean {
  const resolvedKey = resolveManagerKey(webTag);
  if (!resolvedKey) {
    enqueuePendingPropsUpdate(webTag, componentId, propsJson);
    return false;
  }
  const manager = webViewEmbedManagers.get(resolvedKey);
  if (!manager) {
    enqueuePendingPropsUpdate(webTag, componentId, propsJson);
    return false;
  }
  nativeComponentLog(
    `updateNativeComponentProps: webTag=${webTag} componentId=${componentId} propsLen=${propsJson.length}`
  );
  const updated = manager.updateComponentProps(componentId, propsJson);
  if (updated) {
    pendingPropsUpdates.delete(pendingKey(webTag, componentId));
    if (resolvedKey !== webTag) {
      pendingPropsUpdates.delete(pendingKey(resolvedKey, componentId));
    }
    return true;
  }
  enqueuePendingPropsUpdate(webTag, componentId, propsJson);
  return false;
}

/**
 * Dispatch component event from Builder -> EmbedManager -> Rust page function entry.
 */
export function onNativeComponentEventFromBuilder(
  componentId: string,
  eventName: string,
  detail: NativeComponentValue
): boolean {
  if (!componentId || !eventName) {
    nativeComponentLog(`onNativeComponentEventFromBuilder: invalid args componentId=${componentId} event=${eventName}`);
    return false;
  }
  const manager = resolveEmbedManagerByComponentId(componentId);
  if (!manager) {
    nativeComponentLog(`onNativeComponentEventFromBuilder: manager missing componentId=${componentId} event=${eventName}`);
    return false;
  }
  componentManagerCache.set(componentId, manager);
  manager.dispatchComponentEvent(componentId, eventName, detail);
  return true;
}

// ============================================================================
// Lifecycle Management
// ============================================================================

/**
 * Pause all native components for a webTag (e.g., on page navigate away)
 */
export function pauseAllComponents(webTag: string): void {
  const resolvedKey = resolveManagerKey(webTag);
  if (!resolvedKey) {
    return;
  }
  const manager = webViewEmbedManagers.get(resolvedKey);
  if (manager) {
    manager.pauseAll();
    scheduleDelayedStop(resolvedKey);
  }
}

/**
 * Resume all native components for a webTag (e.g., on page navigate back)
 */
export function resumeAllComponents(webTag: string): void {
  cancelDelayedStopForTag(webTag);
  const manager = resolveEmbedManager(webTag);
  if (manager) {
    manager.resumeAll();
  }
}

/**
 * Get NodeController by componentId (searches all embed managers)
 */
export function getNodeControllerByComponentId(componentId: string): NativeComponentNodeController | undefined {
  const cached = componentManagerCache.get(componentId);
  if (cached?.hasComponent(componentId)) {
    return cached.getNodeController(componentId);
  }
  componentManagerCache.delete(componentId);
  for (const manager of webViewEmbedManagers.values()) {
    const controller = manager.getNodeController(componentId);
    if (controller) {
      componentManagerCache.set(componentId, manager);
      return controller;
    }
  }
  return undefined;
}
