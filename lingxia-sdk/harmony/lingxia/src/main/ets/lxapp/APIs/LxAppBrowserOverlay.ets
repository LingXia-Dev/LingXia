import { hilog } from '@kit.PerformanceAnalysisKit';
import { webview } from '@kit.ArkWeb';
import { window } from '@kit.ArkUI';
import { submitBrowserAddress } from './BrowserAddressInput';

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
  @State private addressText: string = '';
  @State private canGoBack: boolean = false;
  @State private canGoForward: boolean = false;
  @State private statusBarHeight: number = 44;
  @State private bottomSafeAreaHeight: number = 0;
  @State private isEditingAddress: boolean = false;

  private controller: webview.WebviewController = new webview.WebviewController();
  private listener: (() => void) | null = null;

  aboutToAppear() {
    this.listener = () => {
      this.isVisible = BrowserOverlayManager.isShowing();
      const newUrl = BrowserOverlayManager.getUrl();
      if (newUrl && newUrl !== this.url) {
        this.url = newUrl;
        this.addressText = newUrl;
        this.canGoBack = false;
        this.canGoForward = false;
      }
    };
    BrowserOverlayManager.registerListener(this.listener!);
    this.initSafeAreaInsets();
  }

  aboutToDisappear() {
    if (this.listener) {
      BrowserOverlayManager.unregisterListener(this.listener!);
      this.listener = null;
    }
  }

  private async initSafeAreaInsets(): Promise<void> {
    try {
      const windowClass: window.Window = await window.getLastWindow(getContext());
      const safeArea: window.AvoidArea = await windowClass.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      this.statusBarHeight = safeArea.topRect.height || 44;
      this.bottomSafeAreaHeight = safeArea.bottomRect.height || 0;
      hilog.info(DOMAIN, TAG, `Safe area top=${this.statusBarHeight} bottom=${this.bottomSafeAreaHeight}`);
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to get safe area insets: ${error}`);
    }
  }

  private submitAddress(raw: string): void {
    const result = submitBrowserAddress(raw, this.url);
    if (!result) {
      return;
    }

    this.addressText = result.displayText;
    this.url = result.url;
    this.isEditingAddress = false;
    try {
      this.controller.loadUrl(result.url);
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `loadUrl failed: ${error}`);
    }
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
            TextInput({ text: this.addressText, placeholder: 'Enter URL' })
              .layoutWeight(1)
              .fontSize(14)
              .fontColor('#333333')
              .backgroundColor(Color.Transparent)
              .border({ width: 0 })
              .padding({ left: 0, right: 0, top: 0, bottom: 0 })
              .caretColor('#333333')
              .enterKeyType(EnterKeyType.Go)
              .onChange((value: string) => {
                this.addressText = value;
              })
              .onFocus(() => {
                this.isEditingAddress = true;
              })
              .onBlur(() => {
                this.isEditingAddress = false;
              })
              .onSubmit((_enterKey) => {
                this.submitAddress(this.addressText);
              })

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
        .padding({ left: 24, right: 24, bottom: 12 })
        .backgroundColor(Color.White)

        // === Middle: Web Content + Bottom Toolbar Overlay ===
        Stack({ alignContent: Alignment.Bottom }) {
          Web({ src: this.url, controller: this.controller })
            .width('100%')
            .height('100%')
            .javaScriptAccess(true)
            .domStorageAccess(true)
            .mixedMode(MixedMode.Compatible)
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
            .height(44)
            .padding({ left: 8, right: 8 })
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
        .layoutWeight(1)
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
      if (currentUrl && !this.isEditingAddress) {
        this.addressText = currentUrl;
      }
      this.canGoBack = this.controller.accessBackward();
      this.canGoForward = this.controller.accessForward();
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `updateState failed: ${error}`);
    }
  }
}
