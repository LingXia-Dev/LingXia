import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import common from '@ohos.app.ability.common';
import { onCallback } from 'liblingxia.so';
import { LxApp } from './LxApp';
import { LxAppToast, ToastIcon, ToastPosition } from './APIs/LxAppToast';
import { LxAppModal, ModalResult } from './APIs/LxAppModal';
import { LxAppActionSheet } from './APIs/LxAppActionSheet';
import { LxAppPicker } from './APIs/LxAppPicker';
import { LxAppDevice } from './APIs/LxAppDevice';
import { LxAppMedia } from './APIs/LxAppMedia';
import { LxAppScan } from './APIs/LxAppScan';
import { LxAppContainer } from './LxAppContainer';
import { LxAppPopup } from './APIs/LxAppPopup';
import { LxAppDocument } from './APIs/LxAppDocument';
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

class StreamVideoConfig {
  width?: number;
  height?: number;
}

// UI Event Constants
export const UI_EVENT_TABBAR_CLICK = 0;
export const UI_EVENT_CAPSULE_CLICK = 1;
export const UI_EVENT_NAVIGATION_CLICK = 2;
export const UI_EVENT_BACK_PRESS = 3;

// UI Event Data Constants
export const CAPSULE_ACTION_MORE = "more";
export const CAPSULE_ACTION_CLOSE = "close";
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
    this.register('showActionSheet', (options: string, cancelText: string, itemColor: string, callbackId: string): void => {
      try {
        // Parse options from JSON string
        const optionsArray: string[] = JSON.parse(options);

        LxAppActionSheet.showActionSheet(optionsArray, cancelText, itemColor, callbackId);
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Error showing action sheet: ${error}`);
      }
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
    ): void => {
      try {
        LxAppPicker.showPicker(
          columns,
          cancelText,
          cancelButtonColor,
          cancelTextColor,
          confirmText,
          confirmButtonColor,
          confirmTextColor,
          callbackId
        );
      } catch (error) {
        hilog.error(DOMAIN, TAG, `showPicker error: ${error}`);
      }
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

    // App control APIs (only what's needed for AppRuntime)
    this.register('exitApp', (): void => {
      try {
        const context = getContext() as common.UIAbilityContext;
        context.terminateSelf();
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Failed to exit app: ${error}`);
      }
    });

    this.register('getScreenInfo', (callbackId: string): void => {
      LxAppDevice.getScreenInfo(callbackId);
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

    this.register('saveMedia', (path: string, mediaTypeStr: string): boolean => {
      const mediaType = Number(mediaTypeStr);
      if (!path || Number.isNaN(mediaType)) { return false; }
      LxAppMedia.saveMedia(path, mediaType).catch((err: Error) => {
        hilog.error(DOMAIN, TAG, `saveMedia async error: ${err}`);
      });
      return true;
    });

    this.register('openDocument', (filePath: string, mimeType: string = '', showMenu: string = '1'): boolean => {
      const showFlag = parseBoolean(showMenu, true);
      return LxAppDocument.openDocument(filePath, mimeType, showFlag);
    });

    this.register('scanCode', (payload: string): boolean => {
      return LxAppScan.scanCode(payload);
    });

    this.register('videoPlayerEvent', (componentId: string, event: string, payload?: string): boolean => {
      switch (event) {
        case 'seekDone': {
          const position = parseInt(payload ?? '', 10);
          if (isNaN(position)) return false;
          LxNativeVideoPlayer.onSeekDone(componentId, position);
          return true;
        }
        case 'buffering':
          LxNativeVideoPlayer.onBufferingChange(componentId, (payload ?? '') === '1');
          return true;
        case 'prepared':
          LxNativeVideoPlayer.onPrepared(componentId);
          return true;
        case 'stopped':
          LxNativeVideoPlayer.onStopped(componentId);
          return true;
        case 'playing':
          LxNativeVideoPlayer.onBufferingChange(componentId, false);
          LxNativeVideoPlayer.onPlaying(componentId);
          return true;
        case 'paused':
          LxNativeVideoPlayer.onPaused(componentId);
          return true;
        case 'waiting':
          LxNativeVideoPlayer.onBufferingChange(componentId, true);
          return true;
        case 'streamConfig': {
          if (!payload) return false;
          try {
            const config = JSON.parse(payload) as StreamVideoConfig;
            const width = Number(config?.width ?? 0);
            const height = Number(config?.height ?? 0);
            if (!Number.isFinite(width) || !Number.isFinite(height)) return false;
            LxNativeVideoPlayer.setStreamVideoSize(componentId, width, height);
            return true;
          } catch (error) {
            hilog.warn(DOMAIN, TAG, `videoPlayerEvent: streamConfig parse error=${error}`);
            return false;
          }
        }
        case 'enterFullscreen':
          LxNativeVideoPlayer.enterFullscreenStatic(componentId);
          return true;
        case 'exitFullscreen':
          LxNativeVideoPlayer.exitFullscreen(componentId);
          return true;
        default:
          hilog.warn(DOMAIN, TAG, `videoPlayerEvent: unknown event=${event}`);
          return false;
      }
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
      const id = String(callbackId ?? '').trim();
      if (!id) {
        hilog.warn(DOMAIN, TAG, 'requestLocationPermission: missing callbackId');
        return false;
      }

      (async () => {
        try {
          const granted: boolean = await PermissionManager.ensureLocationAccess();
          let payload: string;
          if (granted) {
            payload = JSON.stringify({ code: 'ok' });
          } else {
            payload = JSON.stringify({
              code: 'location_permission_denied',
              error: 'Location permission not granted'
            });
          }
          onCallback(id, granted, payload);
          hilog.info(DOMAIN, TAG, `requestLocationPermission result: ${granted}`);
        } catch (error) {
          const message = String(error);
          hilog.error(
            DOMAIN,
            TAG,
            `requestLocationPermission failed: ${message}`
          );
          onCallback(id, false, message);
        }
      })();

      return true;
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
      if (functionName === 'showToast' && rawArgs.length >= 6) {
        // Convert showToast arguments: title, icon, image, duration, mask, position
        // Accept both string and numeric forms for icon/position, and ms/seconds for duration
        const iconMap: Record<string, number> = { 'success': 0, 'error': 1, 'loading': 2, 'none': 3 };
        const positionMap: Record<string, number> = { 'top': 0, 'center': 1, 'bottom': 2 };

        const parseIcon = (v: Object): number => {
          const s = String(v ?? '').toLowerCase().trim();
          const mapped = iconMap[s];
          if (mapped !== undefined) { return mapped; }
          const n = Number(s);
          if (Number.isFinite(n) && n >= 0 && n <= 3) { return Math.floor(n); }
          return 3; // default none
        };

        const parsePosition = (v: Object): number => {
          const s = String(v ?? '').toLowerCase().trim();
          const mapped = positionMap[s];
          if (mapped !== undefined) { return mapped; }
          const n = Number(s);
          if (Number.isFinite(n) && n >= 0 && n <= 2) { return Math.floor(n); }
          return 1; // default center
        };

        const parseBool = (v: Object): boolean => {
          const s = String(v ?? '').toLowerCase().trim();
          if (s === 'true' || s === '1') { return true; }
          if (s === 'false' || s === '0' || s === '') { return false; }
          return !!v;
        };

        const parseDurationSeconds = (v: Object): number => {
          const num = Number(String(v ?? '').trim());
          if (!Number.isFinite(num) || num < 0) { return 1.5; }
          // Heuristic: treat values >= 50 as milliseconds and convert to seconds
          // (common web pattern passes 1500/2000 ms)
          const secs = num >= 50 ? (num / 1000) : num;
          return secs;
        };

        args = [
          String(rawArgs[0] ?? ''),                          // title
          parseIcon(rawArgs[1]),                              // icon
          String(rawArgs[2] ?? ''),                           // image
          parseDurationSeconds(rawArgs[3]),                   // duration (seconds)
          parseBool(rawArgs[4]),                              // mask
          parsePosition(rawArgs[5])                           // position
        ];
      } else if (functionName === 'showModal' && rawArgs.length >= 7) {
        // Handle showModal arguments - skip legacy editable/placeholder parameters if present
        // Expected: title, content, showCancel, cancelText, confirmText, confirmColor, callbackId
        // Legacy format may have 9 args: title, content, showCancel, cancelText, confirmText, editable, placeholder, confirmColor, callbackId
        if (rawArgs.length === 9) {
          // Legacy format with editable and placeholder - skip them
          args = [
            rawArgs[0], // title
            rawArgs[1], // content
            rawArgs[2], // showCancel
            rawArgs[3], // cancelText
            rawArgs[4], // confirmText
            rawArgs[7], // confirmColor (skip editable and placeholder)
            rawArgs[8]  // callbackId
          ];
        } else {
          // New format with 7 args
          args = rawArgs;
        }
      } else if (functionName === 'showPicker' && rawArgs.length >= 8) {
        // Handle showPicker arguments: columns, cancelText, cancelButtonColor, cancelTextColor, confirmText, confirmButtonColor, confirmTextColor, callbackId
        // All arguments are strings, no conversion needed
        args = rawArgs;
      } else if (functionName === 'showPopup' && rawArgs.length >= 5) {
        // appId, path, widthRatio, heightRatio, position as strings
        args = rawArgs;
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
