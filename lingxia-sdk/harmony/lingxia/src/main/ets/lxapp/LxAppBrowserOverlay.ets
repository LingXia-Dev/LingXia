import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { window } from '@kit.ArkUI';
import { LxApp } from './LxApp';
import { requestWebFileChooser } from './WebFileChooserBridge';
import {
  WebViewManager,
  notifyWebViewControllerAttached
} from './WebViewBridge';
import {
  resolveBrowserNavigationPolicy,
  type BrowserNavigationPolicyResponse
} from './BrowserNavigationPolicy';
import {
  browserTabClose,
  browserTabPathForId,
  getBuiltinBrowserAppId
} from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.BrowserOverlay';
const ATTACH_RETRY_DELAY_MS = 100;
const ATTACH_MAX_RETRIES = 8;

interface BrowserUrlDisplay {
  text: string;
  iconPath: Resource;
  iconColor: string;
}

function browserUrlDisplay(rawUrl: string): BrowserUrlDisplay {
  const trimmed = String(rawUrl ?? '').trim();
  if (!trimmed) {
    return {
      text: '',
      iconPath: $rawfile('icons/icon_lock.svg'),
      iconColor: '#666666',
    };
  }
  const fallback: BrowserUrlDisplay = {
    text: 'Web page',
    iconPath: $rawfile('icons/icon_warning.svg'),
    iconColor: '#A15C00',
  };
  const schemeEnd = trimmed.indexOf(':');
  if (schemeEnd <= 0) {
    return fallback;
  }
  const scheme = trimmed.substring(0, schemeEnd).toLowerCase();
  if (scheme === 'http' || scheme === 'https') {
    const rest = trimmed.substring(schemeEnd + 1);
    if (!rest.startsWith('//')) {
      return fallback;
    }
    const authority = rest.substring(2).split(/[/?#]/)[0] ?? '';
    const hostPort = authority.split('@').pop() ?? authority;
    const host = hostPort.startsWith('[')
      ? (hostPort.split(']')[0] + ']')
      : hostPort.split(':')[0];
    if (!host) {
      return fallback;
    }
    if (scheme === 'http') {
      return {
        text: host,
        iconPath: $rawfile('icons/icon_warning.svg'),
        iconColor: '#A15C00',
      };
    }
    return {
      text: host,
      iconPath: $rawfile('icons/icon_lock.svg'),
      iconColor: '#666666',
    };
  }
  return fallback;
}

/**
 * Browser overlay manager - single UI instance (no multi-open UI for now)
 */
export class BrowserOverlayManager {
  private static showing: boolean = false;
  private static currentTabId: string = '';
  private static initialUrl: string = '';
  private static listeners: Set<() => void> = new Set();

  public static show(tabId: string, initialUrl: string = ''): boolean {
    const normalizedTabId = String(tabId ?? '').trim().toLowerCase();
    if (!normalizedTabId) {
      hilog.error(DOMAIN, TAG, 'show failed: empty tabId');
      return false;
    }

    // Single-instance UI rule: new tab request closes previous tab first.
    if (BrowserOverlayManager.currentTabId && BrowserOverlayManager.currentTabId !== normalizedTabId) {
      BrowserOverlayManager.closeCurrentTab();
    }

    BrowserOverlayManager.currentTabId = normalizedTabId;
    BrowserOverlayManager.initialUrl = String(initialUrl ?? '').trim();
    BrowserOverlayManager.showing = true;
    BrowserOverlayManager.notifyListeners();
    return true;
  }

  public static hide(): void {
    BrowserOverlayManager.closeCurrentTab();
    BrowserOverlayManager.showing = false;
    BrowserOverlayManager.currentTabId = '';
    BrowserOverlayManager.initialUrl = '';
    BrowserOverlayManager.notifyListeners();
  }

  public static isShowing(): boolean {
    return BrowserOverlayManager.showing;
  }

  public static getTabId(): string {
    return BrowserOverlayManager.currentTabId;
  }

  public static getInitialUrl(): string {
    return BrowserOverlayManager.initialUrl;
  }

  public static registerListener(listener: () => void): void {
    BrowserOverlayManager.listeners.add(listener);
  }

  public static unregisterListener(listener: () => void): void {
    BrowserOverlayManager.listeners.delete(listener);
  }

  private static notifyListeners(): void {
    BrowserOverlayManager.listeners.forEach((listener: () => void) => {
      try {
        listener();
      } catch (error) {
        hilog.error(DOMAIN, TAG, `Listener error: ${error}`);
      }
    });
  }

  private static closeCurrentTab(): void {
    const tabId = BrowserOverlayManager.currentTabId;
    if (!tabId) {
      return;
    }
    try {
      const closed: boolean = Boolean(browserTabClose(tabId));
      if (!closed) {
        hilog.warn(DOMAIN, TAG, `browserTabClose returned false for tabId=${tabId}`);
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `browserTabClose failed for tabId=${tabId}: ${error}`);
    }
  }
}

/**
 * In-app browser overlay component
 * Bottom floating toolbar: back / forward / address+refresh / close
 */
@Component
export struct BrowserOverlay {
  @State private isVisible: boolean = false;
  @State private tabId: string = '';
  @State private url: string = '';
  @State private addressText: string = '';
  @State private controllerReady: boolean = false;
  @State private canGoBack: boolean = false;
  @State private canGoForward: boolean = false;
  @State private bottomSafeAreaHeight: number = 0;
  @State private topSafeAreaHeight: number = 0;
  @State private addressIconPath: Resource = $rawfile('icons/icon_lock.svg');
  @State private addressIconColor: string = '#666666';

  private controller: webview.WebviewController = new webview.WebviewController();
  private listener: (() => void) | null = null;
  private attachRetryTimerId: number | null = null;
  private attachRetryCount: number = 0;
  private controllerWebTag: string = '';
  private controllerAcked: boolean = false;

  aboutToAppear() {
    this.listener = () => {
      const visible = BrowserOverlayManager.isShowing();
      this.isVisible = visible;
      if (!visible) {
        this.clearAttachRetryTimer();
        this.resetLocalState();
        return;
      }

      const nextTabId = BrowserOverlayManager.getTabId();
      const nextInitialUrl = BrowserOverlayManager.getInitialUrl();

      if (nextTabId !== this.tabId) {
        this.clearAttachRetryTimer();
        this.attachRetryCount = 0;
        this.tabId = nextTabId;
        this.url = nextInitialUrl;
        this.updateAddressDisplay(nextInitialUrl);
        this.canGoBack = false;
        this.canGoForward = false;
        this.controllerReady = false;
        this.attachManagedControllerWithRetry();
        return;
      }

      if (nextInitialUrl && !this.url) {
        this.url = nextInitialUrl;
        this.updateAddressDisplay(nextInitialUrl);
      }

      if (this.controllerReady) {
        this.updateState();
      }
    };

    BrowserOverlayManager.registerListener(this.listener!);
    this.initSafeAreaInsets();
    this.listener();
  }

  aboutToDisappear() {
    this.clearAttachRetryTimer();
    if (this.listener) {
      BrowserOverlayManager.unregisterListener(this.listener!);
      this.listener = null;
    }
  }

  private async initSafeAreaInsets(): Promise<void> {
    try {
      const windowClass: window.Window = await window.getLastWindow(getContext());
      const safeArea: window.AvoidArea = await windowClass.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      this.bottomSafeAreaHeight = safeArea.bottomRect.height || 0;
      this.topSafeAreaHeight = safeArea.topRect.height || 0;
      hilog.info(DOMAIN, TAG, `Safe area top=${this.topSafeAreaHeight} bottom=${this.bottomSafeAreaHeight}`);
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to get safe area insets: ${error}`);
    }
  }

  private attachManagedController(): boolean {
    if (!this.tabId) {
      return false;
    }

    let appId = '';
    let path = '';
    try {
      appId = String(getBuiltinBrowserAppId() ?? '').trim();
      path = String(browserTabPathForId(this.tabId) ?? '').trim();
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to resolve browser app/path for tabId=${this.tabId}: ${error}`);
      return false;
    }

    if (!appId || !path) {
      hilog.error(DOMAIN, TAG, `Invalid browser app/path for tabId=${this.tabId} appId=${appId} path=${path}`);
      return false;
    }

    const managedController = WebViewManager.findWebview(appId, path);
    if (!managedController) {
      return false;
    }

    this.controller = managedController;
    let resolvedWebTag = '';
    const infos = WebViewManager.getAllWebViewInfos();
    for (let i = 0; i < infos.length; i++) {
      if (infos[i].controller === managedController) {
        resolvedWebTag = infos[i].webtag;
        break;
      }
    }
    if (!resolvedWebTag) {
      hilog.warn(DOMAIN, TAG, `attachManagedController: cannot resolve webtag for tabId=${this.tabId}`);
      return false;
    }
    this.controllerWebTag = resolvedWebTag;
    this.controllerAcked = false;
    return true;
  }

  private attachManagedControllerWithRetry(): void {
    if (!this.isVisible || !this.tabId) {
      return;
    }

    if (this.attachManagedController()) {
      this.controllerReady = true;
      this.attachRetryCount = 0;
      this.updateState();
      return;
    }

    this.controllerReady = false;
    if (this.attachRetryCount >= ATTACH_MAX_RETRIES) {
      hilog.error(DOMAIN, TAG, `show failed: managed WebView not found for tabId=${this.tabId}`);
      BrowserOverlayManager.hide();
      return;
    }

    this.attachRetryCount += 1;
    const expectedTabId = this.tabId;
    this.clearAttachRetryTimer();
    this.attachRetryTimerId = setTimeout(() => {
      this.attachRetryTimerId = null;
      if (!this.isVisible || this.tabId !== expectedTabId) {
        return;
      }
      this.attachManagedControllerWithRetry();
    }, ATTACH_RETRY_DELAY_MS);
  }

  private clearAttachRetryTimer(): void {
    if (this.attachRetryTimerId !== null) {
      clearTimeout(this.attachRetryTimerId);
      this.attachRetryTimerId = null;
    }
  }

  private resetLocalState(): void {
    this.tabId = '';
    this.url = '';
    this.addressText = '';
    this.controllerReady = false;
    this.canGoBack = false;
    this.canGoForward = false;
    this.attachRetryCount = 0;
    this.controllerWebTag = '';
    this.controllerAcked = false;
    this.addressIconPath = $rawfile('icons/icon_lock.svg');
    this.addressIconColor = '#666666';
  }

  build() {
    if (this.isVisible && this.tabId && this.controllerReady) {
      Stack({ alignContent: Alignment.Bottom }) {
        Web({ src: '', controller: this.controller })
          .width('100%')
          .height('100%')
          .javaScriptAccess(true)
          .domStorageAccess(true)
          .mixedMode(MixedMode.Compatible)
          .onAppear(() => {
            if (!this.controllerAcked && this.controllerWebTag) {
              notifyWebViewControllerAttached(this.controllerWebTag);
              this.controllerAcked = true;
            }
          })
          .onLoadIntercept((event) => {
            if (!event || !event.data) {
              return false;
            }
            const request = event.data;
            const targetUrl = request.getRequestUrl();
            if (!targetUrl) {
              return false;
            }

            let hasUserGesture = false;
            let isMainFrame = true;
            try {
              hasUserGesture = request.isRequestGesture();
            } catch {}
            try {
              isMainFrame = request.isMainFrame();
            } catch {}

            const policy: BrowserNavigationPolicyResponse | null = resolveBrowserNavigationPolicy(
              targetUrl,
              hasUserGesture,
              isMainFrame
            );
            if (!policy) {
              return true;
            }
            if (policy.decision === 'in_webview') {
              return false;
            }
            if (policy.decision === 'open_external') {
              LxApp.launchWithUrl(targetUrl);
            }
            return true;
          })
          .onShowFileSelector((event) => {
            if (!event) {
              return false;
            }
            return requestWebFileChooser(
              this.controllerWebTag,
              (selectedPaths: Array<string>) => {
                event.result.handleFileList(selectedPaths);
              },
              event.fileSelector.getAcceptType() ?? [],
              event.fileSelector.getMode(),
              event.fileSelector.isCapture(),
              this.url
            );
          })
          .onPageEnd(() => {
            this.updateState();
          })
          .onProgressChange((event) => {
            if (event && event.newProgress > 30) {
              this.updateState();
            }
          })

        Column() {
          Row() {
            Image($rawfile('icons/icon_back.svg'))
              .width(20)
              .height(20)
              .fillColor(this.canGoBack ? '#333333' : '#CCCCCC')
              .padding(6)
              .width(32)
              .height(32)
              .translate({ x: 6, y: 0 })
              .enabled(this.canGoBack)
              .onClick(() => {
                try {
                  this.controller.backward();
                } catch (error) {
                  hilog.warn(DOMAIN, TAG, `backward failed: ${error}`);
                }
              })

            Image($rawfile('icons/icon_forward.svg'))
              .width(20)
              .height(20)
              .fillColor(this.canGoForward ? '#333333' : '#CCCCCC')
              .padding(6)
              .width(32)
              .height(32)
              .translate({ x: -6, y: 0 })
              .enabled(this.canGoForward)
              .onClick(() => {
                try {
                  this.controller.forward();
                } catch (error) {
                  hilog.warn(DOMAIN, TAG, `forward failed: ${error}`);
                }
              })

            Row() {
              Image(this.addressIconPath)
                .width(16)
                .height(16)
                .fillColor(this.addressIconColor)
                .margin({ right: 6 })

              Text(this.addressText)
                .layoutWeight(1)
                .fontSize(14)
                .fontColor('#333333')
                .maxLines(1)
                .textOverflow({ overflow: TextOverflow.Ellipsis })

              Image($rawfile('icons/icon_browser_refresh.svg'))
                .width(18)
                .height(18)
                .fillColor('#666666')
                .margin({ left: 8 })
                .onClick(() => {
                  try {
                    this.controller.refresh();
                  } catch (error) {
                    hilog.warn(DOMAIN, TAG, `refresh failed: ${error}`);
                  }
                })
            }
            .layoutWeight(1)
            .height(38)
            .margin({ left: 4, right: 4 })
            .padding({ left: 12, right: 8 })
            .backgroundColor('#F0F0F0')
            .borderRadius(19)
            .alignItems(VerticalAlign.Center)

            Image($rawfile('icons/icon_close_x.svg'))
              .width(22)
              .height(22)
              .fillColor('#333333')
              .padding(11)
              .width(44)
              .height(44)
              .onClick(() => {
                BrowserOverlayManager.hide();
              })
          }
          .width('100%')
          .height(52)
          .padding({ left: 6, right: 6 })
          .alignItems(VerticalAlign.Center)
          .backgroundColor('#F2FFFFFF')
          .borderRadius(16)
          .border({ width: 0.5, color: '#14000000' })
          .shadow({
            radius: 18,
            color: '#22000000',
            offsetX: 0,
            offsetY: 4
          })
        }
        .width('100%')
        .padding({
          left: 24,
          right: 24,
          bottom: px2vp(this.bottomSafeAreaHeight) + 8
        })
      }
      .width('100%')
      .height('100%')
      .padding({ top: px2vp(this.topSafeAreaHeight) })
      .backgroundColor(Color.White)
      .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
    }
  }

  private updateState(): void {
    try {
      const currentUrl = this.controller.getUrl();
      if (currentUrl) {
        this.url = currentUrl;
        this.updateAddressDisplay(currentUrl);
      }
      this.canGoBack = this.controller.accessBackward();
      this.canGoForward = this.controller.accessForward();
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `updateState failed: ${error}`);
    }
  }

  private updateAddressDisplay(rawUrl: string): void {
    const display = browserUrlDisplay(rawUrl);
    this.addressText = display.text;
    this.addressIconPath = display.iconPath;
    this.addressIconColor = display.iconColor;
  }
}
