import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { window } from '@kit.ArkUI';

const DOMAIN = 0x0000;
const TAG = 'LingXia.BrowserOverlay';

/**
 * Browser overlay manager - controls in-app browser visibility
 */
export class BrowserOverlayManager {
  private static showing: boolean = false;
  private static currentUrl: string = '';
  private static listeners: Set<() => void> = new Set();

  public static show(url: string): boolean {
    if (!url) {
      hilog.error(DOMAIN, TAG, 'show failed: empty URL');
      return false;
    }
    hilog.info(DOMAIN, TAG, `Showing browser overlay for URL: ${url}`);
    BrowserOverlayManager.currentUrl = url;
    BrowserOverlayManager.showing = true;
    BrowserOverlayManager.notifyListeners();
    return true;
  }

  public static hide(): void {
    hilog.info(DOMAIN, TAG, 'Hiding browser overlay');
    BrowserOverlayManager.showing = false;
    BrowserOverlayManager.currentUrl = '';
    BrowserOverlayManager.notifyListeners();
  }

  public static isShowing(): boolean {
    return BrowserOverlayManager.showing;
  }

  public static getUrl(): string {
    return BrowserOverlayManager.currentUrl;
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
}

/**
 * In-app browser overlay component — Safari-style layout
 * Top: pill-shaped address bar with refresh icon (below status bar)
 * Bottom: back / forward / close toolbar
 */
@Component
export struct BrowserOverlay {
  @State private isVisible: boolean = false;
  @State private url: string = '';
  @State private displayHost: string = '';
  @State private canGoBack: boolean = false;
  @State private canGoForward: boolean = false;
  @State private isLoading: boolean = false;
  @State private statusBarHeight: number = 44;

  private controller: webview.WebviewController = new webview.WebviewController();
  private listener: (() => void) | null = null;

  aboutToAppear() {
    this.listener = () => {
      this.isVisible = BrowserOverlayManager.isShowing();
      const newUrl = BrowserOverlayManager.getUrl();
      if (newUrl && newUrl !== this.url) {
        this.url = newUrl;
        this.displayHost = this.extractHost(newUrl);
        this.canGoBack = false;
        this.canGoForward = false;
        this.isLoading = true;
      }
    };
    BrowserOverlayManager.registerListener(this.listener!);
    this.initStatusBarHeight();
  }

  aboutToDisappear() {
    if (this.listener) {
      BrowserOverlayManager.unregisterListener(this.listener!);
      this.listener = null;
    }
  }

  private async initStatusBarHeight(): Promise<void> {
    try {
      const windowClass: window.Window = await window.getLastWindow(getContext());
      const safeArea: window.AvoidArea = await windowClass.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      this.statusBarHeight = safeArea.topRect.height || 44;
      hilog.info(DOMAIN, TAG, `Status bar height: ${this.statusBarHeight}`);
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to get status bar height: ${error}`);
    }
  }

  private extractHost(urlString: string): string {
    try {
      const match = urlString.match(/^https?:\/\/([^\/\?#]+)/);
      if (match && match[1]) {
        return match[1];
      }
    } catch (_e) {
      // ignore
    }
    return urlString;
  }

  build() {
    if (this.isVisible && this.url) {
      Column() {
        // === Top: Status bar spacer + Address Bar ===
        Column() {
          // Spacer for status bar
          Row()
            .width('100%')
            .height(px2vp(this.statusBarHeight))

          // Address bar pill
          Row() {
            // URL text
            Text(this.displayHost)
              .fontSize(14)
              .fontColor('#333333')
              .maxLines(1)
              .textOverflow({ overflow: TextOverflow.Ellipsis })
              .layoutWeight(1)

            // Refresh button
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
          .width('100%')
          .height(44)
          .padding({ left: 16, right: 14 })
          .backgroundColor('#F0F0F0')
          .borderRadius(22)
          .alignItems(VerticalAlign.Center)
        }
        .width('100%')
        .padding({ left: 16, right: 16, bottom: 8 })
        .backgroundColor(Color.White)

        // === Middle: Web Content ===
        Web({ src: this.url, controller: this.controller })
          .width('100%')
          .layoutWeight(1)
          .javaScriptAccess(true)
          .domStorageAccess(true)
          .mixedMode(MixedMode.Compatible)
          .onPageBegin(() => {
            this.isLoading = true;
          })
          .onPageEnd(() => {
            this.isLoading = false;
            this.updateState();
          })
          .onProgressChange((event) => {
            if (event && event.newProgress > 30) {
              this.updateState();
            }
          })

        // === Bottom: Navigation Toolbar ===
        Column() {
          // Top border
          Divider()
            .color('#E0E0E0')
            .strokeWidth(0.5)

          Row() {
            // Back button
            Image($rawfile('icons/icon_back.svg'))
              .width(22)
              .height(22)
              .fillColor(this.canGoBack ? '#333333' : '#CCCCCC')
              .padding(11)
              .width(52)
              .height(44)
              .enabled(this.canGoBack)
              .onClick(() => {
                try {
                  this.controller.backward();
                } catch (error) {
                  hilog.warn(DOMAIN, TAG, `backward failed: ${error}`);
                }
              })

            // Forward button
            Image($rawfile('icons/icon_forward.svg'))
              .width(22)
              .height(22)
              .fillColor(this.canGoForward ? '#333333' : '#CCCCCC')
              .padding(11)
              .width(52)
              .height(44)
              .enabled(this.canGoForward)
              .onClick(() => {
                try {
                  this.controller.forward();
                } catch (error) {
                  hilog.warn(DOMAIN, TAG, `forward failed: ${error}`);
                }
              })

            Blank()

            // Close button
            Image($rawfile('icons/icon_close_x.svg'))
              .width(22)
              .height(22)
              .fillColor('#333333')
              .padding(11)
              .width(52)
              .height(44)
              .onClick(() => {
                BrowserOverlayManager.hide();
              })
          }
          .width('100%')
          .height(48)
          .padding({ left: 12, right: 12 })
          .alignItems(VerticalAlign.Center)
        }
        .width('100%')
        .backgroundColor(Color.White)
      }
      .width('100%')
      .height('100%')
      .backgroundColor(Color.White)
      .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
    }
  }

  private updateState(): void {
    try {
      const currentUrl = this.controller.getUrl();
      if (currentUrl) {
        this.displayHost = this.extractHost(currentUrl);
      }
      this.canGoBack = this.controller.accessBackward();
      this.canGoForward = this.controller.accessForward();
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `updateState failed: ${error}`);
    }
  }
}
