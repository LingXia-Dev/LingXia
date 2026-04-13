import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxApp } from './LxApp';
import { LxAppContainer } from './LxAppContainer';
import { pauseAllComponentsByAppId } from './WebViewBridge';
import { onLxappOpened, onLxappClosed, getTabBar, TabBarItem, TabBarState, getCurrentLxapp, CurrentLxApp, onLxappEvent, UiEventType } from 'liblingxia.so';
import { ToastManager } from './APIs/LxAppToast';
import { ModalManager } from './APIs/LxAppModal';
import { PickerManager } from './APIs/LxAppPicker';
import { DatePickerManager } from './APIs/LxAppDatePicker';
import { ActionSheetManager } from './APIs/LxAppActionSheet';
import { PopupManager } from './APIs/LxAppPopup';
import { MediaPreviewOverlay, MediaPreviewOverlayManager } from './APIs/media/MediaPreviewOverlay';
import { CameraCaptureOverlay, CameraCaptureOverlayManager } from './APIs/media/CameraCaptureOverlay';
import { PdfSinglePageViewerOverlay, PdfSingleViewerOverlayManager } from './APIs/document/PdfSinglePageViewerOverlay';
import { LocalFileChooserOverlay, LocalFileChooserOverlayManager } from './APIs/file/LocalFileChooserOverlay';
import { BrowserOverlay, BrowserOverlayManager } from './APIs/LxAppBrowserOverlay';

const DOMAIN = 0xA000;
const TAG = 'LingXia.Navigation';

// Back handling
const KEYCODE_BACK: number = 2;
const BACK_PRESS_THROTTLE_MS: number = 200;

// Edge-swipe back gesture area (system gesture-back is disabled in LingxiaBaseAbility)
const EDGE_BACK_SWIPE_ZONE_WIDTH_VP: number = 40;
const EDGE_BACK_SWIPE_TRIGGER_DISTANCE_VP: number = 18;
const EDGE_BACK_SWIPE_MAX_VERTICAL_DRIFT_VP: number = 200;
const EDGE_BACK_SWIPE_VERTICAL_CANCEL_MARGIN_VP: number = 8;
const EDGE_BACK_SWIPE_LEFT_DRIFT_CANCEL_VP: number = 4;
const EDGE_BACK_SWIPE_TRIGGER_HORIZONTAL_BIAS_VP: number = 6;

/**
 * Animation type enum for page transitions
 */
export enum AnimationType {
  None = 0,      // No animation (used for Launch/Replace/SwitchTab semantics)
  Forward = 1,   // Forward animation (push-style)
  Backward = 2   // Backward animation (pop-style)
}

/**
 * LxApp Navigation with pure Rust stack management
 * - Completely relies on Rust stack for LxApp management
 * - Only manages UI state for current visible LxApp
 */
@Component
export struct LxAppNavigation {
  @State isReady: boolean = false
  @State homeAppId: string = ''
  @State homeAppPath: string = ''
  @State errorMessage: string = ''

  // Current modal LxApp state (only one modal app can be shown at a time)
  @State private currentModalAppId: string = ''
  @State private currentModalAppPath: string = ''
  @State private modalAppVisible: boolean = false

  // Navigation state for triggering path changes
  @State private navigationTrigger: number = 0

  private isBackHandling: boolean = false
  private lastBackHandledAt: number = 0

  private edgeBackSwipeTracking: boolean = false
  private edgeBackSwipeTriggered: boolean = false
  private edgeBackSwipeFingerId: number = -1
  private edgeBackSwipeStartX: number = 0
  private edgeBackSwipeStartY: number = 0
  private edgeBackSwipeTriggerDistancePx: number = 0
  private edgeBackSwipeMaxVerticalDriftPx: number = 0
  private edgeBackSwipeVerticalCancelMarginPx: number = 0
  private edgeBackSwipeHorizontalTriggerBiasPx: number = 0
  private edgeBackSwipeLeftCancelThresholdPx: number = 0

  aboutToAppear() {
    hilog.info(DOMAIN, TAG, 'LxAppNavigation starting with WebView preservation');
    LxApp.setNavigationInstance(this);
    this.initializeLxApp();
  }

  private closeActiveOverlayIfAny(): boolean {
    // Keep ordering aligned with zIndex (topmost first).
    try {
      if (BrowserOverlayManager.isShowing()) {
        hilog.info(DOMAIN, TAG, 'BackPress: closing overlay BrowserOverlay');
        BrowserOverlayManager.hide();
        return true;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Error closing overlay BrowserOverlay: ${error}`);
    }

    try {
      if (PdfSingleViewerOverlayManager.isShowing()) {
        hilog.info(DOMAIN, TAG, 'BackPress: closing overlay PdfSinglePageViewerOverlay');
        PdfSingleViewerOverlayManager.hide();
        return true;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Error closing overlay PdfSinglePageViewerOverlay: ${error}`);
    }

    try {
      if (LocalFileChooserOverlayManager.isShowing()) {
        hilog.info(DOMAIN, TAG, 'BackPress: closing overlay LocalFileChooserOverlay');
        LocalFileChooserOverlayManager.cancel();
        return true;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Error closing overlay LocalFileChooserOverlay: ${error}`);
    }

    try {
      if (CameraCaptureOverlayManager.isShowing()) {
        hilog.info(DOMAIN, TAG, 'BackPress: closing overlay CameraCaptureOverlay');
        CameraCaptureOverlayManager.hide();
        return true;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Error closing overlay CameraCaptureOverlay: ${error}`);
    }

    try {
      if (MediaPreviewOverlayManager.isOverlayShowing()) {
        hilog.info(DOMAIN, TAG, 'BackPress: closing overlay MediaPreviewOverlay');
        MediaPreviewOverlayManager.closeOverlay();
        return true;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Error closing overlay MediaPreviewOverlay: ${error}`);
    }

    return false;
  }

  private handleSystemBackPress(): boolean {
    const now = Date.now();
    if (this.isBackHandling) {
      return true;
    }

    // Overlays should be closed before forwarding BackPress into the LxApp stack.
    if (this.closeActiveOverlayIfAny()) {
      return true;
    }

    if ((now - this.lastBackHandledAt) < BACK_PRESS_THROTTLE_MS) {
      return true;
    }

    this.isBackHandling = true;
    try {
      const current: CurrentLxApp = getCurrentLxapp();
      if (current && current.appid && current.appid.length > 0) {
        hilog.info(DOMAIN, TAG, `Forwarding system back press to ${current.appid}`);
        const handled: boolean = onLxappEvent(current.appid, UiEventType.BackPress, '');
        if (handled) {
          this.lastBackHandledAt = now;
        }
        return handled;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Error handling system back press: ${error}`);
    } finally {
      this.isBackHandling = false;
    }
    return false;
  }

  private resetEdgeBackSwipe(): void {
    this.edgeBackSwipeTracking = false;
    this.edgeBackSwipeTriggered = false;
    this.edgeBackSwipeFingerId = -1;
    this.edgeBackSwipeStartX = 0;
    this.edgeBackSwipeStartY = 0;
  }

  private updateEdgeBackSwipeMetrics(): void {
    if (this.edgeBackSwipeTriggerDistancePx > 0) {
      return;
    }
    this.edgeBackSwipeTriggerDistancePx = vp2px(EDGE_BACK_SWIPE_TRIGGER_DISTANCE_VP);
    this.edgeBackSwipeMaxVerticalDriftPx = vp2px(EDGE_BACK_SWIPE_MAX_VERTICAL_DRIFT_VP);
    this.edgeBackSwipeVerticalCancelMarginPx = vp2px(EDGE_BACK_SWIPE_VERTICAL_CANCEL_MARGIN_VP);
    this.edgeBackSwipeHorizontalTriggerBiasPx = vp2px(EDGE_BACK_SWIPE_TRIGGER_HORIZONTAL_BIAS_VP);
    this.edgeBackSwipeLeftCancelThresholdPx = vp2px(EDGE_BACK_SWIPE_LEFT_DRIFT_CANCEL_VP);
  }

  private getTouchByFingerId(event: TouchEvent, fingerId: number): TouchObject | null {
    if (!event || !event.changedTouches) {
      return null;
    }
    for (let i = 0; i < event.changedTouches.length; i++) {
      const touch: TouchObject = event.changedTouches[i];
      if (touch.id === fingerId) {
        return touch;
      }
    }
    return null;
  }

  private handleEdgeBackSwipeTouch(event: TouchEvent): void {
    if (!event) {
      return;
    }

    if (event.type === TouchType.Cancel) {
      this.resetEdgeBackSwipe();
      return;
    }

    if (!event.changedTouches || event.changedTouches.length === 0) {
      return;
    }

    const firstTouch: TouchObject = event.changedTouches[0];

    if (event.type === TouchType.Down) {
      this.updateEdgeBackSwipeMetrics();
      this.edgeBackSwipeTracking = true;
      this.edgeBackSwipeTriggered = false;
      this.edgeBackSwipeFingerId = firstTouch.id;
      this.edgeBackSwipeStartX = firstTouch.windowX;
      this.edgeBackSwipeStartY = firstTouch.windowY;
      return;
    }

    if (!this.edgeBackSwipeTracking || this.edgeBackSwipeTriggered) {
      return;
    }

    const trackingTouch: TouchObject | null = this.getTouchByFingerId(event, this.edgeBackSwipeFingerId);

    if (event.type === TouchType.Move) {
      if (!trackingTouch) {
        return;
      }
      const deltaX: number = trackingTouch.windowX - this.edgeBackSwipeStartX;
      const deltaY: number = trackingTouch.windowY - this.edgeBackSwipeStartY;

      // Swiping left (or drifting left) should not be considered for back.
      if (deltaX < -this.edgeBackSwipeLeftCancelThresholdPx) {
        this.resetEdgeBackSwipe();
        return;
      }

      // If user is clearly doing vertical scroll, abandon tracking.
      if (
        Math.abs(deltaY) > this.edgeBackSwipeMaxVerticalDriftPx &&
        Math.abs(deltaY) > (Math.abs(deltaX) + this.edgeBackSwipeVerticalCancelMarginPx)
      ) {
        this.resetEdgeBackSwipe();
        return;
      }

      if (
        deltaX >= this.edgeBackSwipeTriggerDistancePx &&
        Math.abs(deltaY) <= this.edgeBackSwipeMaxVerticalDriftPx &&
        deltaX > (Math.abs(deltaY) + this.edgeBackSwipeHorizontalTriggerBiasPx)
      ) {
        this.edgeBackSwipeTriggered = true;
        event.stopPropagation();
        this.handleSystemBackPress();
        this.resetEdgeBackSwipe();
      }
      return;
    }

    if (event.type === TouchType.Up) {
      if (trackingTouch) {
        this.resetEdgeBackSwipe();
      }
    }
  }

  private handleKeyEventDispatch(event: KeyEvent): boolean {
    if (event && event.keyCode === KEYCODE_BACK && event.type === KeyType.Down) {
      return this.handleSystemBackPress();
    }
    return false;
  }

  @Builder
  private buildEdgeBackSwipeZone(): void {
    Column()
      .width(EDGE_BACK_SWIPE_ZONE_WIDTH_VP)
      .height('100%')
      .position({ x: 0, y: 0 })
      .backgroundColor(Color.Transparent)
      .zIndex(20000)
      // Allow taps/clicks (e.g. navbar back button, PDF viewer back button) to pass through.
      .hitTestBehavior(HitTestMode.Transparent)
      .onTouch((event: TouchEvent) => {
        this.handleEdgeBackSwipeTouch(event);
      })
  }

  private async initializeLxApp(): Promise<void> {
    try {
      const instance = LxApp.getInstance();
      const homeAppId = instance.getHomeLxAppId();

      if (homeAppId) {
        this.homeAppId = homeAppId;
        this.isReady = true;
        hilog.info(DOMAIN, TAG, `LxApp Navigation ready: ${this.homeAppId}`);

        // Auto-open home lxapp now that navigation instance is ready.
        hilog.info(DOMAIN, TAG, 'Auto-opening home lxapp');
        const sessionId: number = LxApp.getRuntimeSessionId(homeAppId);
        if (!Number.isFinite(sessionId) || sessionId <= 0) {
          this.errorMessage = `Invalid home app session: ${homeAppId}`;
          hilog.error(DOMAIN, TAG, this.errorMessage);
          return;
        }
        this.openLxAppWithSession(homeAppId, "", sessionId);
      } else {
        this.errorMessage = 'LxApp not initialized. Please call LxApp.initialize() in EntryAbility first.';
        hilog.error(DOMAIN, TAG, 'LxApp not initialized');
      }
    } catch (error) {
      this.errorMessage = `Initialization error: ${error}`;
      hilog.error(DOMAIN, TAG, `Initialization error: ${error}`);
    }
  }

  public openLxApp(appId: string, path: string): void {
    const sessionId: number = LxApp.getRuntimeSessionId(appId);
    this.openLxAppWithSession(appId, path, sessionId);
  }

  public openLxAppWithSession(appId: string, path: string, sessionId: number): void {
    if (!Number.isFinite(sessionId) || sessionId <= 0) {
      hilog.error(DOMAIN, TAG, `openLxAppWithSession requires valid sessionId for ${appId}`);
      return;
    }
    hilog.info(DOMAIN, TAG, `Opening lxapp: ${appId}:${path} (session=${sessionId})`);

    try {
      const resolvedPath: string = onLxappOpened(appId, path, sessionId);
      if (!resolvedPath || resolvedPath.length === 0) {
        hilog.warn(DOMAIN, TAG, `onLxappOpened rejected open request (stale session?): ${appId} (session=${sessionId})`);
        return;
      }

      // UI layer distinction: Home LxApp vs Modal LxApp
      if (appId === this.homeAppId) {
        hilog.info(DOMAIN, TAG, `Showing home LxApp: ${appId} with path: ${resolvedPath}`);
        this.modalAppVisible = false;
        this.homeAppPath = resolvedPath;
        // Trigger navigation to show the home app
        this.navigationTrigger++;
        return;
      }

      // For modal apps, show modal over home
      hilog.info(DOMAIN, TAG, `Showing modal LxApp: ${appId}:${resolvedPath}`);
      this.currentModalAppId = appId;
      this.currentModalAppPath = resolvedPath;
      this.modalAppVisible = true;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to open LxApp ${appId}: ${error}`);
    }
  }

  public closeLxApp(appId: string): void {
    const sessionId: number = LxApp.getRuntimeSessionId(appId);
    this.closeLxAppWithSession(appId, sessionId);
  }

  public closeLxAppWithSession(appId: string, sessionId: number): void {
    if (!Number.isFinite(sessionId) || sessionId <= 0) {
      hilog.warn(DOMAIN, TAG, `Ignoring close without valid session: ${appId}`);
      return;
    }
    hilog.info(DOMAIN, TAG, `Closing lxapp: ${appId} (session=${sessionId})`);

    try {
      // Call native close handler first - this removes the app from Rust stack
      const accepted: boolean = onLxappClosed(appId, sessionId);
      if (!accepted) {
        hilog.warn(DOMAIN, TAG, `Ignoring stale close callback: ${appId} (session=${sessionId})`);
        return;
      }

      // Pause native components (video players, etc.) after close is accepted.
      pauseAllComponentsByAppId(appId);

      // UI cleanup: hide the closed app
      if (appId === this.homeAppId) {
        hilog.info(DOMAIN, TAG, `Hiding home LxApp: ${appId}`);
        // Home app should never be closed, but if it happens, don't hide the container
      } else if (appId === this.currentModalAppId) {
        // For modal apps, only hide the container so that the underlying WebViews
        // remain attached (hidden) and can be reused on the next open. Actual
        // WebView destruction is driven by Rust via destroyWebViewController.
        hilog.info(DOMAIN, TAG, `Closing modal LxApp and hiding container: ${appId}`);
        this.modalAppVisible = false;
      }

      // Get next LxApp from Rust stack and open it
      const currentLxApp: CurrentLxApp = getCurrentLxapp();
      if (currentLxApp.appid && currentLxApp.appid.length > 0) {
        this.openLxAppWithSession(currentLxApp.appid, currentLxApp.path, currentLxApp.session_id);
      } else {
        hilog.info(DOMAIN, TAG, `No more LxApps in Rust stack - showing empty state`);
        // No apps left in stack - hide modal but keep home visible
        this.modalAppVisible = false;
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to close LxApp ${appId}: ${error}`);
    }
  }

  /**
   * Navigate to page with animation type behavior
   * @param appId - LxApp ID
   * @param path - Page path to navigate to
   * @param animationType - Type of animation
   * @returns boolean - True if navigation was successful, false otherwise
   */
  public navigate(appId: string, path: string, animationType: AnimationType): boolean {
    const typeName = AnimationType[animationType];
    hilog.info(DOMAIN, TAG, `navigate: ${appId}:${path} (${typeName})`);

    try {
      // Execute navigation with animation type information
      return this.navigateToPage(appId, path, animationType);
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Navigation failed: ${error}`);
      return false;
    }
  }

  /**
   * Required by NavigationInstance interface
   * Tabbar visibility is handled by navigation type in LxAppContainer
   */
  public isTabBarHidden(appId: string): boolean {
    return false;
  }

  /**
   * Check if a path is a tabbar item - public for reuse
   */
  public isTabBarItem(appId: string, path: string): boolean {
    try {
      const tabBarState: TabBarState | null = getTabBar(appId);
      if (!tabBarState || !tabBarState.items || tabBarState.items.length === 0) {
        return false;
      }

      const tabIndex: number = tabBarState.items.findIndex((item: TabBarItem): boolean =>
        item.pagePath === path || item.pagePath.includes(path)
      );

      return tabIndex >= 0;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Error checking if path is tabbar item: ${error}`);
      return false;
    }
  }

  /**
   * Navigate to a specific page within an LxApp
   * @param appId - LxApp ID
   * @param path - Page path to navigate to
   * @param animationType - Animation type for animation control
   * @returns boolean - True if navigation was successful, false otherwise
   */
  private navigateToPage(appId: string, path: string, animationType: AnimationType = AnimationType.None): boolean {
    hilog.info(DOMAIN, TAG, `Navigating to page: ${appId}:${path}`);

    // For home app, update the path and trigger navigation
    if (appId === this.homeAppId) {
      hilog.info(DOMAIN, TAG, `Navigating page in home app ${appId} to ${path} with type: ${AnimationType[animationType]}`);
      this.homeAppPath = path;
      // Pass animation type to container for animation control
      LxAppContainer.setAnimationType(appId, animationType);
      // Increment trigger to force UI update
      this.navigationTrigger++;
      hilog.info(DOMAIN, TAG, `Updated homeAppPath to: ${path}, trigger: ${this.navigationTrigger}`);
      return true;
    }

    // For modal apps, check if it's the current modal app
    if (appId === this.currentModalAppId && this.modalAppVisible) {
      hilog.info(DOMAIN, TAG, `Navigating page in modal app ${appId} to ${path} with type: ${AnimationType[animationType]}`);
      this.currentModalAppPath = path;
      // Pass animation type to container for animation control
      LxAppContainer.setAnimationType(appId, animationType);
      // Trigger UI update by updating the path
      this.currentModalAppPath = path;
      hilog.info(DOMAIN, TAG, `Updated modal app ${appId} path to: ${path}`);
      return true;
    }

    hilog.warn(DOMAIN, TAG, `LxApp ${appId} not found or not currently visible for page navigation`);
    return false;
  }

  /**
   * Build modal LxApp container with common styling
   */
  @Builder
  buildModalLxApp(): void {
    LxAppContainer({
      appId: this.currentModalAppId,
      initialPath: this.currentModalAppPath,
      externalPath: this.currentModalAppPath,
      navigationMode: true,
      closeCallback: 'navigation',
      navigationTrigger: 0
    })
    .width('100%')
    .height('100%')
    .zIndex(1000)
    .backgroundColor(Color.White)
    .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
    .visibility(this.modalAppVisible ? Visibility.Visible : Visibility.Hidden)
    .onAppear(() => {
      hilog.info(DOMAIN, TAG, `Modal LxApp appeared: ${this.currentModalAppId}`);
    })
  }

  build() {
    Stack() {
      // Always show home LxApp container to preserve WebViews, but hide when modal is visible
      if (this.isReady && this.homeAppId && this.homeAppPath) {
        LxAppContainer({
          appId: this.homeAppId,
          initialPath: this.homeAppPath,
          externalPath: this.homeAppPath,
          navigationMode: false,
          closeCallback: undefined,
          navigationTrigger: this.navigationTrigger
        })
        .zIndex(0)
        .visibility(this.modalAppVisible ? Visibility.Hidden : Visibility.Visible)
      } else if (this.errorMessage) {
        this.buildErrorPage()
      } else {
        this.buildLoadingPage()
      }

      // Always show modal LxApp container to preserve WebViews, but hide when not visible
      if (this.currentModalAppId) {
        this.buildModalLxApp();
      }

      ToastManager()
        .zIndex(10000)

      ModalManager()
        .zIndex(10001)

      ActionSheetManager()
        .zIndex(10002)

      PickerManager()
        .zIndex(10003)

      DatePickerManager()
        .zIndex(10003)

      PopupManager()
        .zIndex(10004)

      // Integrated media preview overlay
      MediaPreviewOverlay()
        .zIndex(10005)

      // Integrated camera capture overlay
      CameraCaptureOverlay()
        .zIndex(10006)

      // Integrated PDF single-page viewer (closed, safe-area aware)
      PdfSinglePageViewerOverlay()
        .zIndex(10007)

      LocalFileChooserOverlay()
        .zIndex(10008)

      // In-app browser overlay
      BrowserOverlay()
        .zIndex(10009)

      // Custom edge-swipe back gesture zone
      this.buildEdgeBackSwipeZone()
    }
    .width('100%')
    .height('100%')
    .focusable(true)
    .onKeyEventDispatch((event: KeyEvent): boolean => this.handleKeyEventDispatch(event))
  }

  @Builder
  buildFixedCapsuleButton(appId: string): void {
    if (appId !== LxApp.getInstance().getHomeLxAppId()) {
      Stack() {
        Row() {
          Button() {
            this.buildThreeDots()
          }
          .width(44)
          .height(36)
          .backgroundColor(Color.Transparent)
          .onClick(() => {
            hilog.info(DOMAIN, TAG, 'Capsule more button pressed');
          })

          Column()
            .width(0.5)
            .height(18)
            .backgroundColor('#DDDDDD')

          Button() {
            this.buildCloseIcon()
          }
          .width(44)
          .height(36)
          .backgroundColor(Color.Transparent)
          .onClick(() => {
            hilog.info(DOMAIN, TAG, `Capsule close button pressed for appId: ${appId}`);
            this.closeLxApp(appId);
          })
        }
        .backgroundColor('rgba(255,255,255,0.9)')
        .borderRadius(20)
        .border({ width: 0.5, color: '#DDDDDD' })
        .margin({ top: 52, right: 12 })
      }
      .width('100%')
      .height('100%')
      .backgroundColor(Color.Transparent)
      .zIndex(9999)
      .alignContent(Alignment.TopEnd)
    }
  }

  /**
   * Build three dots icon
   */
  @Builder
  buildThreeDots(): void {
    Row() {
      Circle().width(3).height(3).fill('#000000')
      Circle().width(5).height(5).fill('#000000').margin({ left: 4 })
      Circle().width(3).height(3).fill('#000000').margin({ left: 4 })
    }
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }

  /**
   * Build close icon
   */
  @Builder
  buildCloseIcon(): void {
    Stack() {
      Circle()
        .width(16)
        .height(16)
        .fillOpacity(0)
        .stroke('#000000')
        .strokeWidth(2.5)
      Circle()
        .width(6.5)
        .height(6.5)
        .fill('#000000')
    }
  }

  @Builder
  buildErrorPage() {
    Column() {
      Text('⚠️')
        .fontSize(32)
    }
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .width('100%')
    .height('100%')
  }

  /**
   * Build clean loading page
   */
  @Builder
  buildLoadingPage() {
    Column() {
      LoadingProgress()
        .width(24)
        .height(24)
        .color('#1677FF')
    }
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .width('100%')
    .height('100%')
  }
}
