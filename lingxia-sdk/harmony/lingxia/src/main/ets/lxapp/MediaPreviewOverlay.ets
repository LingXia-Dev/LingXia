import { hilog } from '@kit.PerformanceAnalysisKit';
import { window } from '@kit.ArkUI';

export interface MediaPreviewItem {
  id: string;
  uri: string;
}

export interface MediaPreviewPageParams {
  items: MediaPreviewItem[];
  initialIndex?: number;
}

const DOMAIN = 0x0000;
const TAG = 'LingXia.MediaPreviewOverlay';

/**
 * Media preview overlay manager
 */
export class MediaPreviewOverlayManager {
  private static isShowing: boolean = false;
  private static overlayData: MediaPreviewPageParams | null = null;

  /**
   * Show media preview overlay
   */
  public static show(params: MediaPreviewPageParams): void {
    MediaPreviewOverlayManager.overlayData = params;
    MediaPreviewOverlayManager.isShowing = true;
    hilog.info(DOMAIN, TAG, `Showing media preview overlay with ${params.items.length} items`);
  }

  /**
   * Hide media preview overlay
   */
  public static hide(): void {
    MediaPreviewOverlayManager.isShowing = false;
    MediaPreviewOverlayManager.overlayData = null;
    hilog.info(DOMAIN, TAG, 'Hiding media preview overlay');
  }

  /**
   * Get current showing status
   */
  public static isOverlayShowing(): boolean {
    return MediaPreviewOverlayManager.isShowing;
  }

  /**
   * Get current overlay data
   */
  public static getOverlayData(): MediaPreviewPageParams | null {
    return MediaPreviewOverlayManager.overlayData;
  }
}

/**
 * Media preview overlay component
 * For integration by host application
 */
@Component
export struct MediaPreviewOverlay {
  @State private isShowing: boolean = MediaPreviewOverlayManager.isOverlayShowing();
  @State private overlayData: MediaPreviewPageParams | null = MediaPreviewOverlayManager.getOverlayData();

  aboutToAppear() {
    // Monitor state changes
    setInterval(() => {
      this.isShowing = MediaPreviewOverlayManager.isOverlayShowing();
      this.overlayData = MediaPreviewOverlayManager.getOverlayData();
    }, 100);
  }

  build() {
    if (this.isShowing && this.overlayData) {
      Stack() {
        // Semi-transparent background
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor('rgba(0, 0, 0, 0.9)')
          .onClick(() => {
            MediaPreviewOverlayManager.hide();
          })

        // Media preview content
        MediaPreviewContent({
          params: this.overlayData,
          onClose: () => MediaPreviewOverlayManager.hide()
        })
          .width('100%')
          .height('100%')
      }
      .width('100%')
      .height('100%')
      .position({ x: 0, y: 0 })
      .zIndex(9999)
    }
  }
}

/**
 * Media preview content component
 */
@Component
struct MediaPreviewContent {
  @Prop params: MediaPreviewPageParams;
  onClose: () => void = () => {};

  @State private currentIndex: number = 0;
  @State private statusBarHeight: number = 44;
  @State private isZoomed: boolean = false;

  aboutToAppear() {
    this.currentIndex = this.params.initialIndex || 0;
    this.initStatusBarHeight();
  }

  private async initStatusBarHeight(): Promise<void> {
    try {
      const windowClass: window.Window = await window.getLastWindow(getContext());
      const safeArea: window.AvoidArea = await windowClass.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      this.statusBarHeight = safeArea.topRect.height || 44;
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to get status bar height: ${error}`);
    }
  }

  build() {
    Stack({ alignContent: Alignment.Top }) {
      // Indicator
      if (this.params.items.length > 1) {
        Text(`${this.currentIndex + 1}/${this.params.items.length}`)
          .fontColor(Color.White)
          .fontSize(18)
          .fontWeight(FontWeight.Bold)
          .margin({ top: 60 })
          .width('100%')
          .textAlign(TextAlign.Center)
          .zIndex(10)
      }

      // Media display area (currently supports images only)
      if (this.params.items.length > 0) {
        Stack() {
          MediaViewer({
            item: this.params.items[this.currentIndex],
            onZoomStateChanged: (zoomed: boolean) => this.isZoomed = zoomed,
            onDismiss: this.onClose
          })
        }
        .width('100%')
        .height('100%')
        .gesture(
          PanGesture({ fingers: 1, direction: PanDirection.Horizontal })
            .onActionEnd((event: GestureEvent) => {
              if (!this.isZoomed) {
                const offsetX = event.offsetX;
                if (offsetX > 50 && this.currentIndex > 0) {
                  this.currentIndex--;
                } else if (offsetX < -50 && this.currentIndex < this.params.items.length - 1) {
                  this.currentIndex++;
                }
              }
            })
        )
      }
    }
  }
}

/**
 * Media viewer component (currently supports images only, extensible for video)
 */
@Component
struct MediaViewer {
  @Prop item: MediaPreviewItem;
  onZoomStateChanged: (zoomed: boolean) => void = () => {};
  onDismiss: () => void = () => {};

  @State private imageScale: number = 1;
  private readonly minScale: number = 1;
  private readonly maxScale: number = 3;
  private lastTapTime: number = 0;

  build() {
    Stack({ alignContent: Alignment.Center }) {
      Image(this.item.uri)
        .width('100%')
        .height('100%')
        .objectFit(ImageFit.Contain)
        .scale({ x: this.imageScale, y: this.imageScale })
        .gesture(
          GestureGroup(GestureMode.Exclusive,
            TapGesture({ count: 1 })
              .onAction(() => {
                if (this.imageScale <= this.minScale + 0.1) {
                  const now = Date.now();
                  const timeSinceLastTap = now - this.lastTapTime;

                  if (timeSinceLastTap > 300) {
                    this.lastTapTime = now;
                    this.onDismiss();
                  }
                }
              }),

            PinchGesture({ fingers: 2, distance: 1 })
              .onActionUpdate((event: GestureEvent) => {
                if (event.scale) {
                  this.imageScale = Math.min(this.maxScale, Math.max(this.minScale, event.scale));
                  this.onZoomStateChanged(this.imageScale > this.minScale + 0.1);
                }
              })
              .onActionEnd(() => {
                if (this.imageScale <= this.minScale + 0.1) {
                  this.resetZoom();
                }
              })
          )
        )
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Transparent)
  }

  private resetZoom(): void {
    animateTo({ duration: 200, curve: Curve.EaseOut }, () => {
      this.imageScale = this.minScale;
    });
    setTimeout(() => {
      this.onZoomStateChanged(false);
    }, 100);
  }
}
