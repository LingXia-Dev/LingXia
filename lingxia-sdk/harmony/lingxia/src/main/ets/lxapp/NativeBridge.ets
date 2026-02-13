import { hilog } from '@kit.PerformanceAnalysisKit';
import { setComponentCallback } from './NativeComponents/NativeCallbackRegistry';
import { LxApp } from './LxApp';
import { LxAppToast, ToastIcon, ToastPosition } from './APIs/LxAppToast';
import { LxAppModal } from './APIs/LxAppModal';
import { LxAppActionSheet } from './APIs/LxAppActionSheet';
import { LxAppPicker } from './APIs/LxAppPicker';
import { LxAppDevice } from './APIs/LxAppDevice';
import { LxAppMedia } from './APIs/LxAppMedia';
import { LxAppScan } from './APIs/LxAppScan';
import { LxAppContainer } from './LxAppContainer';
import { LxAppPopup } from './APIs/LxAppPopup';
import { LxAppDocument } from './APIs/LxAppDocument';
import { LxAppWifi } from './APIs/LxAppWifi';
import { LxAppCapsule } from './APIs/LxAppCapsule';
import {
  createWebViewController,
  destroyWebViewController,
  loadUrl,
  clearBrowsingData,
  setUserAgent
} from './WebView';
import { animationTypeFromInt } from './NavigationBar';
import { PermissionManager } from './PermissionManager';
import { LxNativeVideoPlayer } from './media/LxNativeVideoPlayer';

const DOMAIN = 0x0000;
const TAG = 'LingXia.NativeBridge';

// UI Event Constants
export const UI_EVENT_TABBAR_CLICK = 0;
export const UI_EVENT_CAPSULE_CLICK = 1;
export const UI_EVENT_NAVIGATION_CLICK = 2;
export const UI_EVENT_BACK_PRESS = 3;

// UI Event Data Constants
export const CAPSULE_ACTION_CLOSE = "close";
export const CAPSULE_ACTION_CLEAN_CACHE_RESTART = "clean_cache_restart";
export const CAPSULE_ACTION_RESTART = "restart";
export const CAPSULE_ACTION_UNINSTALL = "uninstall";
export const NAVIGATION_ACTION_BACK = "back";
export const NAVIGATION_ACTION_HOME = "home";

function parseBoolean(value: Object | undefined, defaultValue: boolean): boolean {
  if (value === undefined || value === null) {
    return defaultValue;
  }
  if (typeof value === 'boolean') {
    return value;
  }
  const normalized = String(value).toLowerCase().trim();
  if (normalized === 'true' || normalized === '1' || normalized === 'yes') {
    return true;
  }
  if (normalized === 'false' || normalized === '0' || normalized === 'no') {
    return false;
  }
  return defaultValue;
}

/**
 * Native Bridge - Internal callback manager
 * Handles all native-to-ArkTS communication
 */
class NativeBridge {
  private static callbacks: Map<string, Function> = new Map();
  private static instance: NativeBridge | null = null;

  private constructor() {
    this.registerDefaultCallbacks();
  }

  public static getInstance(): NativeBridge {
    if (!NativeBridge.instance) {
      NativeBridge.instance = new NativeBridge();
    }
    return NativeBridge.instance;
  }

  /**
   * Register default callbacks that native layer can call
   */
  private registerDefaultCallbacks(): void {
    // Core LxApp functions
    this.register('openLxApp', (appId: string, path: string) => {
      LxApp.openLxApp(appId, path);
      return true;
    });

    this.register('closeLxApp', (appId: string) => {
      LxApp.closeLxApp(appId);
      return true;
    });

    // Navigate with animation type
    // 0=None, 1=Forward, 2=Backward
    this.register('navigate', (appId: string, path: string, animationType: number) => {
      // Use helper function from NavigationBar to convert int to enum
      const animType = animationTypeFromInt(animationType);
      if (animType === null) {
        hilog.error(DOMAIN, TAG, `Invalid animation type: ${animationType}`);
        return false;
      }

      // Call LxApp.navigate with enum
      return LxApp.navigate(appId, path, animType);
    });

    // Launch external URL
    this.register('launchWithUrl', (url: string) => {
      LxApp.launchWithUrl(url);
      return true;
    });

    // WebView management
    this.register('createWebViewController', (webtag: string) => {
      return createWebViewController(webtag);
    });

    this.register('destroyWebViewController', (webtag: string) => {
      return destroyWebViewController(webtag);
    });

    // WebView URL loading
    this.register('loadUrl', (webtag: string, url: string) => {
      return loadUrl(webtag, url);
    });

    // WebView data management
    this.register('clearBrowsingData', (webtag: string) => {
      return clearBrowsingData(webtag);
    });

    // WebView user agent
    this.register('setUserAgent', (webtag: string, userAgent: string) => {
      return setUserAgent(webtag, userAgent);
    });

    // Toast APIs
    this.register('showToast', (title: string, icon: number = 0, image?: string, duration: number = 1.5, mask: boolean = false, position: number = 1) => {
      LxAppToast.showToast(title, icon as ToastIcon, image, duration, mask, position as ToastPosition);
      return true;
    });

    this.register('hideToast', () => {
      LxAppToast.hideToast();
      return true;
    });

    // Modal APIs
    this.register('showModal', (
      title: string,
      content: string,
      showCancel: string,
      cancelText: string,
      confirmText: string,
      confirmColor: string,
      callbackId: string
    ): void => {
      try {
        LxAppModal.showModal(
          title,
          content,
          showCancel,
          cancelText,
          confirmText,
          confirmColor,
          callbackId
        );
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Error showing modal: ${error}`);
      }
    });

    // ActionSheet APIs
    this.register('showActionSheet', (options: string, cancelText: string, itemColor: string, callbackId: string): boolean => {
      return LxAppActionSheet.showActionSheetFromJson(options, cancelText, itemColor, callbackId);
    });

    // Picker APIs (matching Rust interface)
    this.register('showPicker', (
      columns: string,
      cancelText: string,
      cancelButtonColor: string,
      cancelTextColor: string,
      confirmText: string,
      confirmButtonColor: string,
      confirmTextColor: string,
      callbackId: string
    ): boolean => {
      return LxAppPicker.showPicker(
        columns,
        cancelText,
        cancelButtonColor,
        cancelTextColor,
        confirmText,
        confirmButtonColor,
        confirmTextColor,
        callbackId
      );
    });

    this.register('showPopup', (
      appId: string,
      path: string,
      widthRatio: string,
      heightRatio: string,
      position: string
    ): boolean => {
      return LxAppPopup.showPopup(appId, path, widthRatio, heightRatio, position);
    });

    this.register('hidePopup', (appId: string): boolean => {
      return LxAppPopup.hidePopup(appId);
    });

    // Update TabBar APIs
    this.register('updateTabBarUI', (appId: string): boolean => {
      try {
        const success = LxAppContainer.updateTabBarUI(appId);
        if (success) {
          hilog.info(DOMAIN, TAG, `TabBar UI updated successfully for ${appId}`);
        } else {
          hilog.warn(DOMAIN, TAG, `Failed to find active container for TabBar update: ${appId}`);
        }
        return success;
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Failed to update TabBar UI: ${error}`);
        return false;
      }
    });

    // NavigationBar APIs
    this.register('refreshNavBar', (appId: string): boolean => {
      return LxAppContainer.refreshNavBar(appId);
    });

    this.register('getScreenInfo', (callbackId: string): void => {
      LxAppDevice.getScreenInfo(callbackId);
    });

    this.register('getCapsuleRect', (callbackId: string): void => {
      LxAppCapsule.getCapsuleRect(callbackId);
    });

    // WiFi APIs
    this.register('startWifi', (callbackId: string): void => {
      LxAppWifi.startWifi(callbackId);
    });

    this.register('stopWifi', (callbackId: string): void => {
      LxAppWifi.stopWifi(callbackId);
    });

    this.register('connectWifi', (callbackId: string, ssid: string, password: string): void => {
      LxAppWifi.connectWifi(callbackId, ssid, password);
    });

    this.register('getWifiList', (callbackId: string): void => {
      LxAppWifi.getWifiList(callbackId);
    });

    this.register('getConnectedWifi', (callbackId: string): void => {
      LxAppWifi.getConnectedWifi(callbackId);
    });

    this.register('addWifiStateListener', (callbackId: string): void => {
      LxAppWifi.addWifiStateListener(callbackId);
    });

    this.register('removeWifiStateListener', (callbackId: string): void => {
      LxAppWifi.removeWifiStateListener(callbackId);
    });

    this.register('makePhoneCall', (phoneNumber: string): void => {
      LxAppDevice.makePhoneCall(phoneNumber);
    });

    this.register('previewMedia', (payload: string): boolean => {
      return LxAppMedia.previewMedia(payload);
    });

    this.register('chooseMedia', (payload: string): boolean => {
      return LxAppMedia.chooseMedia(payload);
    });

    this.register('saveMedia', (path: string, mediaTypeStr: string, callbackId: string): boolean => {
      const cb = String(callbackId ?? '').trim();
      if (!cb) { return false; }
      const mediaType = Number(mediaTypeStr);
      return LxAppMedia.saveMedia(path, mediaType, cb);
    });

    this.register('openDocument', (filePath: string, mimeType: string = '', showMenu: string = '1'): boolean => {
      const showFlag = parseBoolean(showMenu, true);
      return LxAppDocument.openDocument(filePath, mimeType, showFlag);
    });

    this.register('scanCode', (payload: string): boolean => {
      return LxAppScan.scanCode(payload);
    });

    this.register('setVideoPlayerCallback', (componentId: string, callbackId: string): boolean => {
      const id = String(componentId ?? '').trim();
      const cb = String(callbackId ?? '').trim();
      if (!id) {
        hilog.warn(DOMAIN, TAG, 'setVideoPlayerCallback: missing componentId');
        return false;
      }
      setComponentCallback(id, cb);
      return true;
    });

    this.register('videoPlayerEvent', (componentId: string, event: string, payload?: string): boolean => {
      return LxNativeVideoPlayer.handleNativeEvent(componentId, event, payload);
    });


    // Pull-to-Refresh APIs
    this.register('startPullDownRefresh', (appId: string, path: string): void => {
      LxAppContainer.startPullRefresh(appId, path);
      hilog.info(DOMAIN, TAG, `startPullDownRefresh called for ${appId}:${path}`);
    });

    this.register('stopPullDownRefresh', (appId: string, path: string): void => {
      LxAppContainer.stopPullRefresh(appId);
      hilog.info(DOMAIN, TAG, `stopPullDownRefresh called for ${appId}:${path}`);
    });

    // The result is reported back to Rust using the shared callback mechanism
    // so that the logic layer can decide whether to proceed with the location
    // request or surface an error.
    this.register('requestLocationPermission', (callbackId: string): boolean => {
      return PermissionManager.requestLocationPermission(callbackId);
    });

    hilog.info(DOMAIN, TAG, `Registered ${NativeBridge.callbacks.size} default callbacks`);
  }

  /**
   * Register a callback function
   */
  public register(name: string, callback: Function): void {
    NativeBridge.callbacks.set(name, callback);
  }

  /**
   * Call a registered callback function
   * This is the main entry point for native layer
   */
  public static call(name: string, ...args: Object[]): Object | null {
    const callback = NativeBridge.callbacks.get(name);
    if (callback) {
      try {
        return callback(...args);
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Error calling callback ${name}: ${error}`);
        return null;
      }
    } else {
      hilog.warn(DOMAIN, TAG, `Callback not found: ${name}`);
      return null;
    }
  }
}

/**
 * Get the callback function for native layer
 * Node-API ThreadSafe Function limitation: can only pass single string
 * Format: "function_name|arg1|arg2|arg3|..." (using | as separator to avoid conflicts with URLs)
 */
export function getNativeCallbackFunction(): (data: string) => Object | null {
  return (data: string) => {
    const parts = data.split('|');
    if (parts.length >= 1) {
      const functionName = parts[0];
      const rawArgs = parts.slice(1); // Get all arguments after function name

      // Convert arguments based on function type
      let args: Object[] = rawArgs;
      if (functionName === 'showToast') {
        args = LxAppToast.normalizeNativeArgs(rawArgs) ?? rawArgs;
      } else if (functionName === 'showModal') {
        args = LxAppModal.normalizeNativeArgs(rawArgs) ?? rawArgs;
      }

      const result = NativeBridge.call(functionName, ...args);
      return result;
    }
    return null;
  };
}

/**
 * Initialize the native bridge
 * This should be called during LxApp initialization
 */
export function initNativeBridge(): void {
  NativeBridge.getInstance();
}
