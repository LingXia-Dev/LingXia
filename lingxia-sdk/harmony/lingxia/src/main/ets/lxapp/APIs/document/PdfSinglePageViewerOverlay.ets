import { hilog } from '@kit.PerformanceAnalysisKit';
import { pdfService } from '@kit.PDFKit';
import { image } from '@kit.ImageKit';

const DOMAIN = 0x0000;
const TAG = 'LingXia.PdfSingleViewer';

interface PdfSingleOverlayState {
  filePath: string;
  title: string;
  showMenu: boolean;
}

interface PdfSingleOverlayOptions {
  title?: string;
  showMenu?: boolean;
  onShare?: () => void;
}

export class PdfSingleViewerOverlayManager {
  private static current: PdfSingleOverlayState | null = null;
  private static shareHandler: (() => void) | null = null;

  public static show(filePath: string, options: PdfSingleOverlayOptions = {}): boolean {
    if (!filePath) { return false; }
    const title = options.title ?? PdfSingleViewerOverlayManager.deriveTitle(filePath);
    const showMenu = options.showMenu !== undefined ? options.showMenu : true;
    PdfSingleViewerOverlayManager.current = { filePath, title, showMenu };
    PdfSingleViewerOverlayManager.shareHandler = showMenu ? (options.onShare ?? null) : null;
    return true;
  }

  public static hide(): void {
    PdfSingleViewerOverlayManager.current = null;
    PdfSingleViewerOverlayManager.shareHandler = null;
  }

  public static share(): void {
    try { PdfSingleViewerOverlayManager.shareHandler?.(); } catch (e) {
      hilog.error(DOMAIN, TAG, `share handler error: ${e}`);
    }
  }

  public static isShowing(): boolean { return PdfSingleViewerOverlayManager.current !== null; }
  public static getState(): PdfSingleOverlayState | null { return PdfSingleViewerOverlayManager.current; }

  private static deriveTitle(path: string): string {
    try {
      const decoded = decodeURIComponent(path);
      const parts = decoded.split('/');
      return parts.length > 0 ? parts[parts.length - 1] : decoded;
    } catch (_) {
      const parts = path.split('/');
      return parts.length > 0 ? parts[parts.length - 1] : path;
    }
  }
}

@Component
export struct PdfSinglePageViewerOverlay {
  @State private isVisible: boolean = PdfSingleViewerOverlayManager.isShowing();
  @State private overlayState: PdfSingleOverlayState | null = PdfSingleViewerOverlayManager.getState();
  private intervalId: number = -1;

  aboutToAppear() {
    this.intervalId = setInterval(() => {
      this.isVisible = PdfSingleViewerOverlayManager.isShowing();
      this.overlayState = PdfSingleViewerOverlayManager.getState();
    }, 120);
  }

  aboutToDisappear() {
    if (this.intervalId !== -1) {
      clearInterval(this.intervalId);
      this.intervalId = -1;
    }
  }

  build() {
    if (this.isVisible && this.overlayState) {
      Stack() {
        // Mask background (click to close)
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor('rgba(0, 0, 0, 0.45)')
          .onClick(() => PdfSingleViewerOverlayManager.hide())

        // Foreground viewer content
        PdfSinglePageContent({
          filePath: (this.overlayState as PdfSingleOverlayState).filePath,
          title: (this.overlayState as PdfSingleOverlayState).title,
          showMenu: (this.overlayState as PdfSingleOverlayState).showMenu,
          onClose: () => PdfSingleViewerOverlayManager.hide(),
          onShare: () => PdfSingleViewerOverlayManager.share()
        })
        .width('100%')
        .height('100%')
      }
      .width('100%')
      .height('100%')
      .position({ x: 0, y: 0 })
      .zIndex(10007)
    }
  }
}

@Component
struct PdfSinglePageContent {
  @Prop filePath: string;
  @Prop title: string;
  @Prop showMenu: boolean;
  onClose: () => void = () => {};
  onShare: () => void = () => {};

  private document: pdfService.PdfDocument | null = null;
  @State private pageCount: number = 0;
  @State private currentPage: number = 1; // Current visible page (1-based)
  @State private pagePixelMaps: image.PixelMap[] = []; // All page images
  @State private showPageIndicator: boolean = false; // Show/hide page indicator
  @State private indicatorOpacity: number = 0; // Indicator opacity for animation
  private hideIndicatorTimer: number = -1; // Timer for auto-hiding indicator
  private scroller: Scroller = new Scroller(); // Scroller controller

  // Zoom state for each page (simplified - no pan, just zoom)
  @State private pageScales: number[] = []; // Scale for each page
  private readonly minScale: number = 1.0;
  private readonly maxScale: number = 4.0;

  aboutToAppear() {
    this.loadDocument();
  }

  aboutToDisappear() {
    this.releaseDocument();
    if (this.hideIndicatorTimer !== -1) {
      clearTimeout(this.hideIndicatorTimer);
    }
  }

  private loadDocument(): void {
    try {
      const doc = new pdfService.PdfDocument();
      doc.loadDocument(this.filePath, '');
      this.document = doc;
      this.pageCount = doc.getPageCount();
      hilog.info(DOMAIN, TAG, `PDF loaded: pageCount=${this.pageCount}`);
      this.loadAllPages();
    } catch (e) {
      hilog.error(DOMAIN, TAG, `loadDocument failed: ${e}`);
      this.onClose();
    }
  }

  private loadAllPages(): void {
    if (!this.document) { return; }
    const maps: image.PixelMap[] = [];
    const scales: number[] = [];
    for (let i = 0; i < this.pageCount; i++) {
      try {
        const page = this.document.getPage(i);
        const map = page.getPagePixelMap();
        maps.push(map);
        scales.push(1.0); // Initial scale
      } catch (e) {
        hilog.error(DOMAIN, TAG, `loadPage ${i} failed: ${e}`);
      }
    }
    this.pagePixelMaps = maps;
    this.pageScales = scales;
    hilog.info(DOMAIN, TAG, `Loaded ${maps.length} pages`);
  }

  private releaseDocument(): void {
    try { this.document?.releaseDocument(); } catch (_) {}
    this.document = null;
    // Release all page pixel maps
    for (const map of this.pagePixelMaps) {
      try { map?.release(); } catch (_) {}
    }
    this.pagePixelMaps = [];
    this.pageScales = [];
  }

  private displayPageIndicator(): void {
    this.showPageIndicator = true;
    this.indicatorOpacity = 1;

    // Clear existing timer
    if (this.hideIndicatorTimer !== -1) {
      clearTimeout(this.hideIndicatorTimer);
    }

    // Auto-hide after 1.5 seconds
    this.hideIndicatorTimer = setTimeout(() => {
      this.indicatorOpacity = 0;
      setTimeout(() => {
        this.showPageIndicator = false;
      }, 300); // Wait for fade-out animation
    }, 1500);
  }

  private onScrollIndex(start: number, end: number): void {
    // Show indicator when scrolling
    this.displayPageIndicator();

    // Update current page based on the last visible item
    // This ensures we show the correct page number when scrolling to the end
    this.currentPage = Math.min(end + 1, this.pageCount); // Convert 0-based index to 1-based page number
  }

  build() {
    // Full-screen layout with Stack to overlay navigation on top
    Stack({ alignContent: Alignment.Top })
      .width('100%')
      .height('100%')
      .backgroundColor('#F5F5F5')
      .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
    {
      // PDF content area - continuous scroll with List
      if (this.pagePixelMaps.length > 0) {
        List({ scroller: this.scroller }) {
          ForEach(this.pagePixelMaps, (pixelMap: image.PixelMap, index: number) => {
            ListItem() {
              this.renderZoomablePage(pixelMap, index)
            }
          }, (pixelMap: image.PixelMap, index: number) => `page_${index}`)
        }
        .width('100%')
        .height('100%')
        .padding({ top: 92 })
        .backgroundColor('#F5F5F5')
        .edgeEffect(EdgeEffect.Spring)
        .scrollBar(BarState.Off)
        .onScrollIndex((start: number, end: number) => {
          this.onScrollIndex(start, end);
        })
      } else {
        // Loading state
        Column()
          .width('100%')
          .height('100%')
          .justifyContent(FlexAlign.Center)
          .alignItems(HorizontalAlign.Center)
          .backgroundColor('#F5F5F5')
        {
          LoadingProgress().color('#666666')
        }
      }

      // Navigation bar overlay (on top of content)
      this.renderNavigationBar()

      // Page indicator overlay (top left, below nav bar) - iOS style
      if (this.showPageIndicator && this.pageCount > 0) {
        Text(`${this.currentPage} / ${this.pageCount}`)
          .fontColor('#FFFFFF')
          .fontSize(14)
          .fontWeight(FontWeight.Medium)
          .padding({ left: 12, right: 12, top: 6, bottom: 6 })
          .backgroundColor('rgba(0, 0, 0, 0.5)') // Semi-transparent black
          .borderRadius(12)
          .position({ x: 10, y: 98 })
          .zIndex(150)
          .opacity(this.indicatorOpacity)
          .animation({
            duration: 300,
            curve: Curve.EaseInOut
          })
          .hitTestBehavior(HitTestMode.None) // Allow gestures to pass through
      }
    }
  }

  @Builder
  private renderZoomablePage(pixelMap: image.PixelMap, index: number) {
    Column() {
      Image(pixelMap)
        .width('100%')
        .objectFit(ImageFit.Contain)
        .backgroundColor('#FFFFFF')
        .scale({
          x: this.pageScales[index] ?? 1.0,
          y: this.pageScales[index] ?? 1.0
        })
        .gesture(
          GestureGroup(GestureMode.Exclusive,
            // Double tap to toggle zoom (2x)
            TapGesture({ count: 2 })
              .onAction(() => {
                const currentScale = this.pageScales[index] ?? 1.0;
                if (currentScale > this.minScale + 0.1) {
                  // Zoomed in, reset to min
                  this.pageScales[index] = this.minScale;
                } else {
                  // Not zoomed, zoom to 2x
                  this.pageScales[index] = 2.0;
                }
              }),
            // Pinch gesture for zooming (like MediaPreviewOverlay)
            PinchGesture({ fingers: 2, distance: 1 })
              .onActionUpdate((event: GestureEvent) => {
                if (event && event.scale) {
                  // event.scale is the scale from gesture start (not cumulative)
                  this.pageScales[index] = Math.min(this.maxScale, Math.max(this.minScale, event.scale));
                }
              })
              .onActionEnd(() => {
                // Reset to minScale if user zoomed out below threshold
                if (this.pageScales[index] <= this.minScale + 0.1) {
                  this.pageScales[index] = this.minScale;
                }
              })
          )
        )
    }
    .width('100%')
    .margin({ bottom: 8 })
  }

  @Builder
  private renderNavigationBar() {
    // Navigation bar with minimal status bar spacer
    Column()
      .width('100%')
      .zIndex(100)
    {
      // Small transparent spacer (status bar is 105px, we only need small gap)
      Row()
        .width('100%')
        .height(48)
        .backgroundColor(Color.Transparent)

      // Actual navigation bar
      Row()
        .width('100%')
        .height(44)
        .hitTestBehavior(HitTestMode.Block)
        .backgroundColor('#F8F8F8')
        .padding({ left: 8, right: 8, top: 4, bottom: 4 })
        .shadow({
          radius: 2,
          color: 'rgba(0, 0, 0, 0.03)',
          offsetX: 0,
          offsetY: 1
        })
      {
      // Back button
      Button({ type: ButtonType.Normal }) {
        Text('←')
          .fontSize(18)
          .fontColor('#333333')
          .fontWeight(FontWeight.Medium)
      }
      .width(32)
      .height(32)
      .backgroundColor('#EEEEEE')
      .borderRadius(16)
      .onClick(() => {
        hilog.info(DOMAIN, TAG, 'Back button clicked');
        this.onClose();
      })

      // Title
      Text(this.title)
        .layoutWeight(1)
        .fontSize(13)
        .fontColor('#333333')
        .fontWeight(FontWeight.Medium)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .textAlign(TextAlign.Center)
        .margin({ left: 6, right: 6 })

      // Share button
      if (this.showMenu) {
        Button({ type: ButtonType.Normal }) {
          Text('分享')
            .fontSize(11)
            .fontColor('#333333')
            .fontWeight(FontWeight.Medium)
        }
        .width(40)
        .height(32)
        .backgroundColor('#EEEEEE')
        .borderRadius(16)
        .onClick(() => {
          hilog.info(DOMAIN, TAG, 'Share button clicked');
          this.onShare();
        })
      } else {
        Row().width(40).height(32)
      }
      }
    }
  }
}
