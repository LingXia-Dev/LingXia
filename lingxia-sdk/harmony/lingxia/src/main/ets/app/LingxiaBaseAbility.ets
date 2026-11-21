import { AbilityConstant, UIAbility, Want } from '@kit.AbilityKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { window, display } from '@kit.ArkUI';
import { Lingxia } from './Lingxia';
import { LxLog } from './LxLog';
import {
  getCurrentLxapp,
  onAppShow,
  onAppHide,
  onUserCaptureScreen,
  onDeviceOrientationChanged,
  CurrentLxApp
} from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Ability';
const DEVICE_ORIENTATION_PORTRAIT: string = 'portrait';
const DEVICE_ORIENTATION_LANDSCAPE: string = 'landscape';

interface LingxiaWindowSizeChangeEvent {
  readonly width?: number;
  readonly height?: number;
}

interface WindowWithGestureBackControl {
  setGestureBackEnabled?: (enabled: boolean) => Promise<void>;
  isGestureBackEnabled?: () => boolean;
}

interface WindowWithSizeOn {
  on?: (type: string, callback: (event: LingxiaWindowSizeChangeEvent) => void) => void;
  off?: (type: string, callback: (event: LingxiaWindowSizeChangeEvent) => void) => void;
}

class WindowSizeEventTargetWrapper {
  private readonly window: window.Window;
  private readonly windowWithOn: WindowWithSizeOn;

  constructor(windowInstance: window.Window) {
    this.window = windowInstance;
    this.windowWithOn = windowInstance as WindowWithSizeOn;
  }

  public register(type: string, callback: (event: LingxiaWindowSizeChangeEvent) => void): boolean {
    const listener = this.windowWithOn.on;
    if (typeof listener === 'function') {
      const typedListener = listener as (eventType: string, cb: (event: LingxiaWindowSizeChangeEvent) => void) => void;
      typedListener(type, callback);
      return true;
    }
    return false;
  }

  public unregister(type: string, callback: (event: LingxiaWindowSizeChangeEvent) => void): void {
    const listener = this.windowWithOn.off;
    if (typeof listener === 'function') {
      const typedListener = listener as (eventType: string, cb: (event: LingxiaWindowSizeChangeEvent) => void) => void;
      typedListener(type, callback);
    }
  }
}

export class LingxiaBaseAbility extends UIAbility {
  private static hostAddonInstalled: boolean = false;
  private mainWindow: window.Window | null = null;
  private hasEnteredBackground: boolean = false;
  private gestureBackConfigInProgress: boolean = false;
  private screenshotListenerRegistered: boolean = false;
  private sizeChangeListenerRegistered: boolean = false;
  private screenshotListener: (() => void) | null = null;
  private sizeChangeListener: ((event: LingxiaWindowSizeChangeEvent) => void) | null = null;
  private lastReportedDeviceOrientation: string = '';

  /**
   * Override to install native host addon.
   */
  protected registerHostAddon(): void {
  }

  private disableSystemGestureBack(): void {
    if (!this.mainWindow || this.gestureBackConfigInProgress) {
      return;
    }

    this.gestureBackConfigInProgress = true;
    try {
      const gestureWindow: WindowWithGestureBackControl = this.mainWindow as WindowWithGestureBackControl;

      if (typeof gestureWindow.isGestureBackEnabled === 'function') {
        try {
          if (!gestureWindow.isGestureBackEnabled()) {
            this.gestureBackConfigInProgress = false;
            return;
          }
        } catch {
          // Ignore and attempt to set anyway.
        }
      }

      if (typeof gestureWindow.setGestureBackEnabled === 'function') {
        gestureWindow.setGestureBackEnabled(false)
          .then(() => {
            hilog.info(DOMAIN, TAG, '%{public}s', 'System gesture back disabled');
          })
          .catch((error: Error) => {
            const message = error?.message ?? String(error);
            hilog.warn(DOMAIN, TAG, '%{public}s', `Failed to disable system gesture back: ${message}`);
          })
          .finally(() => {
            this.gestureBackConfigInProgress = false;
          });
        return;
      }

      hilog.warn(DOMAIN, TAG, '%{public}s', 'Window does not expose setGestureBackEnabled');
    } catch {
      hilog.warn(DOMAIN, TAG, '%{public}s', 'Error configuring gesture back');
    }

    this.gestureBackConfigInProgress = false;
  }

  private attachMainWindow(windowStage: window.WindowStage): void {
    try {
      const mainWindow = windowStage.getMainWindowSync();
      if (!mainWindow) {
        hilog.warn(DOMAIN, TAG, '%{public}s', 'Main window not available');
        return;
      }
      this.mainWindow = mainWindow;
      // The system edge-swipe back would background the app; LxAppNavigation pops pages itself.
      this.disableSystemGestureBack();
    } catch (error) {
      LxLog.e(TAG, `Failed to attach main window: ${error}`);
    }
  }

  protected registerScreenshotListener(): void {
    if (this.screenshotListenerRegistered) {
      return;
    }
    if (!this.mainWindow) {
      hilog.warn(DOMAIN, TAG, '%{public}s', 'Main window not available for screenshot listener');
      return;
    }

    try {
      const listener = () => {
        hilog.info(DOMAIN, TAG, '%{public}s', 'Screenshot detected');
        try {
          const current: CurrentLxApp = getCurrentLxapp();
          if (current && current.appid && current.appid.length > 0) {
            onUserCaptureScreen(current.appid);
          }
        } catch (error) {
          LxLog.e(TAG, `Error in screenshot callback: ${error}`);
        }
      };
      this.mainWindow.on('screenshot', listener);
      this.screenshotListener = listener;
      this.screenshotListenerRegistered = true;
      hilog.info(DOMAIN, TAG, '%{public}s', 'Screenshot listener registered');
    } catch (error) {
      LxLog.e(TAG, `Failed to register screenshot listener: ${error}`);
    }
  }

  private resolveOrientationFromSize(width: number, height: number): string | null {
    if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) {
      return null;
    }

    if (width >= height) {
      return DEVICE_ORIENTATION_LANDSCAPE;
    }
    return DEVICE_ORIENTATION_PORTRAIT;
  }

  private reportDeviceOrientation(orientationValue: string): void {
    if (orientationValue !== DEVICE_ORIENTATION_PORTRAIT && orientationValue !== DEVICE_ORIENTATION_LANDSCAPE) {
      return;
    }

    if (this.lastReportedDeviceOrientation === orientationValue) {
      return;
    }

    try {
      const current: CurrentLxApp = getCurrentLxapp();
      if (!current || !current.appid || current.appid.length === 0) {
        return;
      }

      const sessionId: number = Math.trunc(Number(current.sessionId));
      if (!Number.isFinite(sessionId) || sessionId <= 0) {
        return;
      }

      const accepted: boolean = onDeviceOrientationChanged(current.appid, sessionId, orientationValue);
      if (accepted) {
        this.lastReportedDeviceOrientation = orientationValue;
      }
    } catch (error) {
      LxLog.e(TAG, `Failed to report device orientation: ${error}`);
    }
  }

  private syncDeviceOrientationFromDisplay(): void {
    try {
      const defaultDisplay = display.getDefaultDisplaySync();
      const orientationValue = this.resolveOrientationFromSize(defaultDisplay.width, defaultDisplay.height);
      if (orientationValue) {
        this.reportDeviceOrientation(orientationValue);
      }
    } catch (error) {
      hilog.warn(DOMAIN, TAG, '%{public}s', `Failed to read display orientation: ${error}`);
    }
  }

  private registerWindowSizeListener(windowStage: window.WindowStage): void {
    if (this.sizeChangeListenerRegistered) {
      return;
    }

    try {
      const mainWindow = windowStage.getMainWindowSync();
      if (!mainWindow) {
        hilog.warn(DOMAIN, TAG, '%{public}s', 'Main window not available for orientation listener');
        return;
      }

      const wrapper = new WindowSizeEventTargetWrapper(mainWindow);
      const listener = (event: LingxiaWindowSizeChangeEvent) => {
        const width = Number(event?.width);
        const height = Number(event?.height);
        const orientationValue = this.resolveOrientationFromSize(width, height);
        if (orientationValue) {
          this.reportDeviceOrientation(orientationValue);
        }
      };
      const registered = wrapper.register('windowSizeChange', listener);

      if (!registered) {
        hilog.warn(DOMAIN, TAG, '%{public}s', 'Window does not expose windowSizeChange listener');
        return;
      }

      this.sizeChangeListener = listener;
      this.sizeChangeListenerRegistered = true;
      this.syncDeviceOrientationFromDisplay();
    } catch (error) {
      LxLog.e(TAG, `Failed to register orientation listener: ${error}`);
    }
  }

  onWindowStageCreate(windowStage: window.WindowStage): void {
    super.onWindowStageCreate(windowStage);
    this.unregisterWindowListeners();

    // Register native host addon (once only)
    if (!LingxiaBaseAbility.hostAddonInstalled) {
      this.registerHostAddon();
      LingxiaBaseAbility.hostAddonInstalled = true;
    }
    this.attachMainWindow(windowStage);
    this.registerScreenshotListener();
    this.registerWindowSizeListener(windowStage);
  }

  private unregisterWindowListeners(): void {
    const mainWindow = this.mainWindow;
    if (mainWindow) {
      if (this.screenshotListener) {
        try {
          mainWindow.off('screenshot', this.screenshotListener);
        } catch (error) {
          LxLog.e(TAG, `Failed to unregister screenshot listener: ${error}`);
        }
      }
      if (this.sizeChangeListener) {
        try {
          new WindowSizeEventTargetWrapper(mainWindow).unregister('windowSizeChange', this.sizeChangeListener);
        } catch (error) {
          LxLog.e(TAG, `Failed to unregister orientation listener: ${error}`);
        }
      }
    }

    this.screenshotListener = null;
    this.sizeChangeListener = null;
    this.screenshotListenerRegistered = false;
    this.sizeChangeListenerRegistered = false;
    this.mainWindow = null;
  }

  onWindowStageDestroy(): void {
    this.unregisterWindowListeners();
    super.onWindowStageDestroy();
  }

  /**
   * Override to return false and take inbound https links over natively: the
   * Want then reaches this ability and stops there.
   */
  protected deliverAppLinks(): boolean {
    return true;
  }

  onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): void {
    super.onCreate(want, launchParam);
    if (this.deliverAppLinks()) {
      Lingxia.handleWant(want);
    }
    Lingxia.handleNotificationWant(want);
  }

  onNewWant(want: Want, launchParam: AbilityConstant.LaunchParam): void {
    super.onNewWant(want, launchParam);
    if (this.deliverAppLinks()) {
      Lingxia.handleWant(want);
    }
    Lingxia.handleNotificationWant(want);
  }

  onDestroy(): void {
    this.unregisterWindowListeners();
    super.onDestroy();
    hilog.info(DOMAIN, TAG, '%{public}s', 'Ability onDestroy');
  }

  onForeground(): void {
    super.onForeground();
    hilog.info(DOMAIN, TAG, '%{public}s', 'Ability onForeground');

    this.disableSystemGestureBack();
    this.syncDeviceOrientationFromDisplay();

    // Notify native layer that app entered foreground
    try {
      const current: CurrentLxApp = getCurrentLxapp();
      if (current && current.appid && current.appid.length > 0) {
        if (this.hasEnteredBackground) {
          onAppShow(current.appid);
          this.hasEnteredBackground = false;
        }
      }
    } catch (error) {
      LxLog.e(TAG, `Error in onForeground: ${error}`);
    }
  }

  onBackground(): void {
    super.onBackground();
    hilog.info(DOMAIN, TAG, '%{public}s', 'Ability onBackground');

    // Notify native layer that app entered background
    try {
      const current: CurrentLxApp = getCurrentLxapp();
      if (current && current.appid && current.appid.length > 0) {
        onAppHide(current.appid);
      }
      this.hasEnteredBackground = true;
    } catch (error) {
      LxLog.e(TAG, `Error in onBackground: ${error}`);
    }
  }

  // System Back reaches the entry page's onBackPress (see handleSystemBack); only an unhandled
  // one gets here, where the runtime's default moves the app to the background.
}
