import { hilog } from '@kit.PerformanceAnalysisKit';
import { display } from '@kit.ArkUI';
import { webview } from '@kit.ArkWeb';
import { LxAppContainer } from '../LxAppContainer';
import { WebViewManager } from '../WebView';
import { onPageShow } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Popup';

export enum PopupPosition {
  Center = 0,
  Bottom = 1
}

interface PopupState {
  appId: string;
  path: string;
  widthVp: number;
  heightVp: number;
  position: PopupPosition;
  bottomInset: number;
}

interface PopupDimensions {
  widthVp: number;
  heightVp: number;
  bottomInset: number;
}

export interface PopupLayoutConfig {
  widthVp: number;
  heightVp: number;
  position: PopupPosition;
  bottomInset: number;
  appId: string;
  path: string;
}

export class LxAppPopup {
  private static managerRef: PopupManager | null = null;
  private static activeLayout: PopupLayoutConfig | null = null;

  public static setManagerRef(manager: PopupManager): void {
    LxAppPopup.managerRef = manager;
  }

  public static clearManagerRef(manager: PopupManager): void {
    if (LxAppPopup.managerRef === manager) {
      LxAppPopup.managerRef = null;
    }
  }

  public static showPopup(
    appId: string,
    path: string,
    widthRatioValue: string,
    heightRatioValue: string,
    positionValue: string
  ): boolean {
    if (!LxAppPopup.managerRef) {
      hilog.error(DOMAIN, TAG, 'showPopup failed: PopupManager not initialized');
      return false;
    }

    const widthRatio = parseRatio(widthRatioValue);
    const heightRatio = parseRatio(heightRatioValue);
    const position = normalizePosition(positionValue);

    const dimensions = computeDimensions(widthRatio, heightRatio, position);

    hilog.info(DOMAIN, TAG, `showPopup dimensions: width=${dimensions.widthVp}, height=${dimensions.heightVp}, position=${position}`);

    LxAppPopup.activeLayout = {
      appId: appId,
      path: path,
      widthVp: dimensions.widthVp,
      heightVp: dimensions.heightVp,
      position: position,
      bottomInset: dimensions.bottomInset
    };

    const success = LxAppPopup.managerRef.showPopup({
      appId: appId,
      path: path,
      widthVp: dimensions.widthVp,
      heightVp: dimensions.heightVp,
      position: position,
      bottomInset: dimensions.bottomInset
    });

    hilog.info(DOMAIN, TAG, `showPopup result=${success} for ${appId}:${path}`);
    if (success) {
      LxAppContainer.setOverlay(
        appId,
        path,
        true,
        dimensions.widthVp,
        dimensions.heightVp,
        position,
        dimensions.bottomInset
      );
      WebViewManager.notifyPopupState(appId, path, true);
      onPageShow(appId, path);
    } else {
      LxAppPopup.activeLayout = null;
    }
    return success;
  }

  public static hidePopup(appId: string): boolean {
    if (!LxAppPopup.managerRef) {
      hilog.warn(DOMAIN, TAG, 'hidePopup called without initialized manager');
      return true;
    }

    const previousLayout = LxAppPopup.activeLayout;
    const result = LxAppPopup.managerRef.hidePopup(appId);
    hilog.info(DOMAIN, TAG, `hidePopup result=${result} for ${appId}`);
    if (result) {
      if (previousLayout) {
        LxAppContainer.setOverlay(previousLayout.appId, previousLayout.path, false);
        WebViewManager.notifyPopupState(previousLayout.appId, previousLayout.path, false);
      }
      LxAppPopup.activeLayout = null;
    }
    return result;
  }

  public static getActiveLayout(): PopupLayoutConfig | null {
    return LxAppPopup.activeLayout;
  }
}

function parseRatio(value: string): number {
  const parsed = parseFloat(value);
  if (isNaN(parsed)) {
    return NaN;
  }
  return parsed;
}

function sanitizeFraction(value: number, fallback: number): number {
  if (isNaN(value)) {
    return fallback;
  }
  if (value <= 0) {
    return 0;
  }
  if (value >= 1) {
    return 1;
  }
  return value;
}

function normalizePosition(value: string): PopupPosition {
  const normalized = value.trim().toLowerCase();
  if (normalized === 'center') {
    return PopupPosition.Center;
  }
  if (normalized === 'bottom') {
    return PopupPosition.Bottom;
  }
  return PopupPosition.Bottom;
}

function defaultHeightRatio(position: PopupPosition, screenWidth: number, screenHeight: number): number {
  const minDp = Math.min(screenWidth, screenHeight);
  const maxDp = Math.max(screenWidth, screenHeight);
  const isTablet = minDp >= 600;

  if (position === PopupPosition.Bottom) {
    return isTablet ? 0.45 : 0.55;
  }

  if (isTablet) {
    return 0.5;
  }

  if (maxDp >= 900) {
    return 0.55;
  }

  if (maxDp >= 780) {
    return 0.58;
  }

  return 0.6;
}

function computeDimensions(
  widthRatio: number,
  heightRatio: number,
  position: PopupPosition
): PopupDimensions {
  const defaultDisplay = display.getDefaultDisplaySync();
  const screenWidth = defaultDisplay.width / defaultDisplay.densityPixels;
  const screenHeight = defaultDisplay.height / defaultDisplay.densityPixels;

  const sanitizedWidth = sanitizeFraction(widthRatio, 1.0);
  const defaultHeight = defaultHeightRatio(position, screenWidth, screenHeight);
  const sanitizedHeight = sanitizeFraction(heightRatio, defaultHeight);

  let widthVp: number;
  if (sanitizedWidth >= 0.999) {
    widthVp = screenWidth;
  } else {
    const widthCap = Math.max(screenWidth - 32, 0);
    const boundedWidth = Math.min(sanitizedWidth * screenWidth, widthCap);
    widthVp = Math.min(screenWidth, Math.max(160, boundedWidth));
  }

  let heightVp: number;
  if (sanitizedHeight >= 0.999) {
    heightVp = screenHeight;
  } else {
    const rawCap = position === PopupPosition.Bottom ? screenHeight - 24 : screenHeight - 32;
    const verticalCap = Math.max(rawCap, 0);
    const boundedHeight = Math.min(sanitizedHeight * screenHeight, verticalCap);
    heightVp = Math.min(screenHeight, Math.max(160, boundedHeight));
  }

  const bottomInset = 0;

  return {
    widthVp: widthVp,
    heightVp: heightVp,
    bottomInset: bottomInset
  };
}

@Component
export struct PopupManager {
  @State private isVisible: boolean = false;
  @State private appId: string = '';
  @State private path: string = '';

  aboutToAppear() {
    LxAppPopup.setManagerRef(this);
  }

  aboutToDisappear() {
    LxAppPopup.clearManagerRef(this);
  }

  public showPopup(state: PopupState): boolean {
    this.appId = state.appId;
    this.path = state.path;
    this.isVisible = true;
    return true;
  }

  public hidePopup(appId: string): boolean {
    if (!this.isVisible) {
      return true;
    }

    if (this.appId && this.appId !== appId) {
      hilog.warn(DOMAIN, TAG, `hidePopup mismatch: expected ${this.appId}, got ${appId}`);
    }

    this.isVisible = false;
    this.appId = '';
    this.path = '';

    return true;
  }

  build() {
    // Popup UI moved into LxAppContainer (same-tree overlay). Nothing to render here.
    Stack() {}
  }
}
