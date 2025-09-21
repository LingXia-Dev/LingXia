import { AbilityConstant, UIAbility, Want } from '@kit.AbilityKit';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { window } from '@kit.ArkUI';
import { LxApp } from './LxApp';
import { getCurrentLxapp, onUiEvent, UiEventType } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Ability';

interface LingxiaWindowKeyEvent {
  readonly keyCode?: number;
  stopPropagation?: () => void;
  preventDefault?: () => void;
  handled?: boolean;
}

interface WindowWithOn {
  on?: (type: string, callback: (event: LingxiaWindowKeyEvent) => void) => void;
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

const BACK_KEY_CODES: number[] = [2, 4]; // Known back key codes (KEY_BACK, KEYCODE_BACK)

export class LingxiaBaseAbility extends UIAbility {
  private isBackHandling: boolean = false;
  private lastBackHandledAt: number = 0;

  private handleBackPressInternal(): boolean {
    const now = Date.now();
    if (this.isBackHandling || (now - this.lastBackHandledAt) < 200) {
      return true;
    }

    this.isBackHandling = true;
    try {
      const current: Record<string, string> = getCurrentLxapp();
      if (current && current.appid && current.appid.length > 0) {
        hilog.info(DOMAIN, TAG, '%{public}s', `Forwarding back press to ${current.appid}`);
        onUiEvent(current.appid, UiEventType.BackPress, '');
        this.lastBackHandledAt = now;
        return true;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, '%{public}s', `Error handling back press: ${error}`);
    } finally {
      this.isBackHandling = false;
    }
    return false;
  }

  protected registerWindowBackInterceptor(windowStage: window.WindowStage): void {
    try {
      const mainWindow = windowStage.getMainWindowSync();
      if (!mainWindow) {
        hilog.warn(DOMAIN, TAG, '%{public}s', 'Main window not available for back interceptor');
        return;
      }

      const wrapper = new WindowKeyEventTargetWrapper(mainWindow);
      const registered = wrapper.register('keyEvent', (event: LingxiaWindowKeyEvent) => {
        if (!event) {
          return;
        }

        if (event.keyCode !== undefined && BACK_KEY_CODES.indexOf(event.keyCode) !== -1) {
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

  onWindowStageCreate(windowStage: window.WindowStage): void {
    super.onWindowStageCreate(windowStage);
    this.registerWindowBackInterceptor(windowStage);
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
  }

  onBackground(): void {
    super.onBackground();
    hilog.info(DOMAIN, TAG, '%{public}s', 'Ability onBackground');
  }

  onBackPressed(): boolean {
    hilog.info(DOMAIN, TAG, '%{public}s', 'Ability onBackPressed');

    if (this.handleBackPressInternal()) {
      return true;
    }

    return super.onBackPressed();
  }
}
