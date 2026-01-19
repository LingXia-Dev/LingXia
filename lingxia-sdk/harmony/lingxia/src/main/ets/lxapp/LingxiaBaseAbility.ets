import { AbilityConstant, UIAbility, Want } from '@kit.AbilityKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { window } from '@kit.ArkUI';
import { LxApp } from './LxApp';
import { getCurrentLxapp, onUiEvent, UiEventType, onAppShow, onAppHide, onUserCaptureScreen, CurrentLxApp } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Ability';
const KEYCODE_BACK: number = 2;
const BACK_PRESS_THROTTLE_MS: number = 200;

interface LingxiaWindowKeyEvent {
  readonly keyCode?: number;
  stopPropagation?: () => void;
  preventDefault?: () => void;
  handled?: boolean;
}

interface WindowWithOn {
  on?: (type: string, callback: (event: LingxiaWindowKeyEvent) => void) => void;
}

interface WindowWithGestureBackControl {
  setGestureBackEnabled?: (enabled: boolean) => Promise<void>;
  isGestureBackEnabled?: () => boolean;
}

class WindowKeyEventTargetWrapper {
  private readonly window: window.Window;
  private readonly windowWithOn: WindowWithOn;

  constructor(windowInstance: window.Window) {
    this.window = windowInstance;
    this.windowWithOn = windowInstance as WindowWithOn;
  }

  public register(type: string, callback: (event: LingxiaWindowKeyEvent) => void): boolean {
    const listener = this.windowWithOn.on;
    if (typeof listener === 'function') {
      const typedListener = listener as (eventType: string, cb: (event: LingxiaWindowKeyEvent) => void) => void;
      typedListener(type, callback);
      return true;
    }
    return false;
  }
}

export class LingxiaBaseAbility extends UIAbility {
  private isBackHandling: boolean = false;
  private lastBackHandledAt: number = 0;
  private static extensionsRegistered: boolean = false;
  private mainWindow: window.Window | null = null;
  private hasEnteredBackground: boolean = false;
  private gestureBackConfigInProgress: boolean = false;

  /**
   * Override to register custom native extensions.
   */
  protected registerExtensions(): void {
  }

  private handleBackPressInternal(): boolean {
    const now = Date.now();
    if (this.isBackHandling || (now - this.lastBackHandledAt) < BACK_PRESS_THROTTLE_MS) {
      return true;
    }

    this.isBackHandling = true;
    try {
      const current: CurrentLxApp = getCurrentLxapp();
      if (current && current.appid && current.appid.length > 0) {
        hilog.info(DOMAIN, TAG, '%{public}s', `Forwarding back press to ${current.appid}`);
        const handled: boolean = onUiEvent(current.appid, UiEventType.BackPress, '');
        if (handled) {
          this.lastBackHandledAt = now;
        }
        return handled;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, '%{public}s', `Error handling back press: ${error}`);
    } finally {
      this.isBackHandling = false;
    }
    return false;
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

  protected registerWindowBackInterceptor(windowStage: window.WindowStage): void {
    try {
      const mainWindow = windowStage.getMainWindowSync();
      if (!mainWindow) {
        hilog.warn(DOMAIN, TAG, '%{public}s', 'Main window not available for back interceptor');
        return;
      }

      // Store reference for screenshot listener
      this.mainWindow = mainWindow;

      // Disable system edge-swipe back gesture (it backgrounds the UIAbility by default).
      // We implement our own in-app back handling and keep the app in foreground.
      this.disableSystemGestureBack();

      const wrapper = new WindowKeyEventTargetWrapper(mainWindow);
      const registered = wrapper.register('keyEvent', (event: LingxiaWindowKeyEvent) => {
        if (!event) {
          return;
        }

        if (event.keyCode !== undefined && event.keyCode === KEYCODE_BACK) {
          if (this.handleBackPressInternal()) {
            if (typeof event.stopPropagation === 'function') {
              event.stopPropagation();
            }
            if (typeof event.preventDefault === 'function') {
              event.preventDefault();
            }
            event.handled = true;
          }
        }
      });

      if (!registered) {
        hilog.warn(DOMAIN, TAG, '%{public}s', 'Window does not expose keyEvent listener');
        return;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, '%{public}s', `Failed to register back interceptor: ${error}`);
    }
  }

  protected registerScreenshotListener(): void {
    if (!this.mainWindow) {
      hilog.warn(DOMAIN, TAG, '%{public}s', 'Main window not available for screenshot listener');
      return;
    }

    try {
      this.mainWindow.on('screenshot', () => {
        hilog.info(DOMAIN, TAG, '%{public}s', 'Screenshot detected');
        try {
          const current: CurrentLxApp = getCurrentLxapp();
          if (current && current.appid && current.appid.length > 0) {
            onUserCaptureScreen(current.appid);
          }
        } catch (error) {
          hilog.error(DOMAIN, TAG, '%{public}s', `Error in screenshot callback: ${error}`);
        }
      });
      hilog.info(DOMAIN, TAG, '%{public}s', 'Screenshot listener registered');
    } catch (error) {
      hilog.error(DOMAIN, TAG, '%{public}s', `Failed to register screenshot listener: ${error}`);
    }
  }

  onWindowStageCreate(windowStage: window.WindowStage): void {
    super.onWindowStageCreate(windowStage);

    // Register extensions (once only)
    if (!LingxiaBaseAbility.extensionsRegistered) {
      this.registerExtensions();
      LingxiaBaseAbility.extensionsRegistered = true;
    }
    this.registerWindowBackInterceptor(windowStage);
    this.registerScreenshotListener();
  }

  onCreate(want: Want, launchParam: AbilityConstant.LaunchParam): void {
    super.onCreate(want, launchParam);
    LxApp.handleWant(want);
  }

  onNewWant(want: Want, launchParam: AbilityConstant.LaunchParam): void {
    super.onNewWant(want, launchParam);
    LxApp.handleWant(want);
  }

  onDestroy(): void {
    super.onDestroy();
    hilog.info(DOMAIN, TAG, '%{public}s', 'Ability onDestroy');
  }

  onForeground(): void {
    super.onForeground();
    hilog.info(DOMAIN, TAG, '%{public}s', 'Ability onForeground');

    this.disableSystemGestureBack();

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
      hilog.error(DOMAIN, TAG, '%{public}s', `Error in onForeground: ${error}`);
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
      hilog.error(DOMAIN, TAG, '%{public}s', `Error in onBackground: ${error}`);
    }
  }

  // Note: UIAbility.onBackPressed return value controls whether the UIAbility is
  // moved to background or destroyed. In-app back is handled by key events and
  // the custom edge-swipe gesture in LxAppNavigation.
}
