import { hilog } from '@kit.PerformanceAnalysisKit';
import { pdfService } from '@kit.PDFKit';
import { image } from '@kit.ImageKit';
import { LxLog } from '../../../app/LxLog';

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
      LxLog.e(TAG, `share handler error: ${e}`);
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
  @State private pageIndices: number[] = [];
  @State private pageCacheVersion: number = 0;
  @State private showPageIndicator: boolean = false; // Show/hide page indicator
  @State private indicatorOpacity: number = 0; // Indicator opacity for animation
  private hideIndicatorTimer: number = -1; // Timer for auto-hiding indicator
  private scroller: Scroller = new Scroller(); // Scroller controller
  private pagePixelMapCache: Map<number, image.PixelMap> = new Map();
  private readonly prefetchRadius: number = 2;
  private readonly releaseRadius: number = 4;
  private estimatedPageHeight: number = 842;

  // Whole-screen zoom state
  @State private contentScale: number = 1.0;
  @State private contentTranslateX: number = 0;
  @State private contentTranslateY: number = 0;
  private baseScale: number = 1.0; // Scale at pinch start
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
      this.pageIndices = Array.from({ length: this.pageCount }, (_: Object, index: number) => index);
      this.resolveEstimatedPageHeight();
      hilog.info(DOMAIN, TAG, `PDF loaded: pageCount=${this.pageCount}`);
      this.ensureVisiblePagesLoaded(0, Math.min(2, this.pageCount - 1));
    } catch (e) {
      LxLog.e(TAG, `loadDocument failed: ${e}`);
      this.onClose();
    }
  }

  private resolveEstimatedPageHeight(): void {
    if (!this.document || this.pageCount <= 0) {
      return;
    }
    try {
      const page = this.document.getPage(0);
      const box: pdfService.PdfRect = page.getBox(pdfService.BoxType.BOX_MEDIA);
      const pageWidth: number = Math.abs(box.right - box.left);
      const pageHeight: number = Math.abs(box.bottom - box.top);
      if (pageWidth > 0 && pageHeight > 0) {
        this.estimatedPageHeight = Math.round((pageHeight / pageWidth) * 360);
      }
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `resolveEstimatedPageHeight failed: ${error}`);
    }
  }

  private loadPage(index: number): void {
    if (!this.document || index < 0 || index >= this.pageCount || this.pagePixelMapCache.has(index)) {
      return;
    }
    try {
      const page = this.document.getPage(index);
      const renderScale = 2.0;
      try {
        const box: pdfService.PdfRect = page.getBox(pdfService.BoxType.BOX_MEDIA);
        const pageWidth: number = Math.abs(box.right - box.left);
        const pageHeight: number = Math.abs(box.bottom - box.top);
        const scaleMatrix = new pdfService.PdfMatrix();
        scaleMatrix.width = pageWidth * renderScale;
        scaleMatrix.height = pageHeight * renderScale;
        scaleMatrix.rotate = 0;
        const map = page.getCustomPagePixelMap(scaleMatrix, false, true);
        this.pagePixelMapCache.set(index, map);
      } catch (highResErr) {
        hilog.warn(DOMAIN, TAG, `Page ${index} high-res failed, using standard: ${highResErr}`);
        const map = page.getPagePixelMap();
        this.pagePixelMapCache.set(index, map);
      }
      this.pageCacheVersion += 1;
    } catch (error) {
      LxLog.e(TAG, `loadPage ${index} failed: ${error}`);
    }
  }

  private ensureVisiblePagesLoaded(start: number, end: number): void {
    if (this.pageCount <= 0) {
      return;
    }
    const from = Math.max(0, start - this.prefetchRadius);
    const to = Math.min(this.pageCount - 1, end + this.prefetchRadius);
    for (let index = from; index <= to; index++) {
      this.loadPage(index);
    }
    this.releaseDistantPages(from, to);
  }

  private releaseDistantPages(visibleStart: number, visibleEnd: number): void {
    const keepFrom = Math.max(0, visibleStart - this.releaseRadius);
    const keepTo = Math.min(this.pageCount - 1, visibleEnd + this.releaseRadius);
    let released: boolean = false;
    this.pagePixelMapCache.forEach((map: image.PixelMap, index: number) => {
      if (index < keepFrom || index > keepTo) {
        try { map.release(); } catch (_) {}
        this.pagePixelMapCache.delete(index);
        released = true;
      }
    });
    if (released) {
      this.pageCacheVersion += 1;
    }
  }

  private hasLoadedPage(index: number): boolean {
    return this.pagePixelMapCache.has(index);
  }

  private getLoadedPage(index: number): image.PixelMap {
    return this.pagePixelMapCache.get(index) as image.PixelMap;
  }

  private releaseDocument(): void {
    try { this.document?.releaseDocument(); } catch (_) {}
    this.document = null;
    this.pagePixelMapCache.forEach((map: image.PixelMap) => {
      try { map?.release(); } catch (_) {}
    });
    this.pagePixelMapCache.clear();
    this.pageIndices = [];
    this.pageCacheVersion = 0;
  }

  private resetZoom(): void {
    animateTo({ duration: 200, curve: Curve.EaseOut }, () => {
      this.contentScale = this.minScale;
      this.contentTranslateX = 0;
      this.contentTranslateY = 0;
    });
  }

  private isZoomed(): boolean {
    return this.contentScale > this.minScale + 0.1;
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

    const visibleStart = Math.max(0, Math.min(start, this.pageCount - 1));
    const visibleEnd = Math.min(this.pageCount - 1, visibleStart + 2);

    // Some Harmony builds may report a very large `end` index here.
    // Keep the preload window bounded so we never regress back to
    // whole-document eager rendering.
    this.currentPage = visibleStart + 1;
    this.ensureVisiblePagesLoaded(visibleStart, visibleEnd);
  }

  build() {
    // Full-screen layout with Stack to overlay navigation on top
    Stack({ alignContent: Alignment.Top })
      .width('100%')
      .height('100%')
      .backgroundColor('#F5F5F5')
      .expandSafeArea([SafeAreaType.SYSTEM], [SafeAreaEdge.TOP, SafeAreaEdge.BOTTOM])
    {
      // Zoomable content container - applies scale/translate to entire content
      Column() {
        // PDF content area - continuous scroll with List
        if (this.pageIndices.length > 0) {
          List({ scroller: this.scroller }) {
            ForEach(this.pageIndices, (index: number) => {
              ListItem() {
                this.renderPage(index)
              }
            }, (index: number) => `page_${index}`)
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
      }
      .width('100%')
      .height('100%')
      .scale({ x: this.contentScale, y: this.contentScale })
      .translate({ x: this.contentTranslateX, y: this.contentTranslateY })
      .gesture(
        GestureGroup(GestureMode.Exclusive,
          // Double tap to toggle zoom (2x) - zoom entire screen
          TapGesture({ count: 2 })
            .onAction(() => {
              if (this.isZoomed()) {
                this.resetZoom();
              } else {
                animateTo({ duration: 200, curve: Curve.EaseOut }, () => {
                  this.contentScale = 2.0;
                });
              }
            }),
          // Pinch gesture for zooming entire screen content
          PinchGesture({ fingers: 2, distance: 1 })
            .onActionStart(() => {
              this.baseScale = this.contentScale;
            })
            .onActionUpdate((event: GestureEvent) => {
              if (event && event.scale) {
                this.contentScale = Math.min(this.maxScale, Math.max(this.minScale, this.baseScale * event.scale));
              }
            })
            .onActionEnd(() => {
              if (this.contentScale <= this.minScale + 0.1) {
                this.resetZoom();
              }
            }),
          // Pan gesture when zoomed (for moving around the zoomed content)
          PanGesture()
            .onActionUpdate((event: GestureEvent) => {
              if (this.isZoomed() && event) {
                this.contentTranslateX += event.offsetX;
                this.contentTranslateY += event.offsetY;
              }
            })
        )
      )

      // Navigation bar overlay (on top of content, not affected by zoom)
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
  private renderPage(index: number) {
    Column() {
      if (this.hasLoadedPage(index)) {
        Image(this.getLoadedPage(index))
          .width('100%')
          .objectFit(ImageFit.Contain)
          .backgroundColor('#FFFFFF')
      } else {
        Column() {
          LoadingProgress()
            .color('#9CA3AF')
        }
        .width('100%')
        .height(this.estimatedPageHeight)
        .justifyContent(FlexAlign.Center)
        .alignItems(HorizontalAlign.Center)
        .backgroundColor('#FFFFFF')
        .onAppear(() => {
          this.loadPage(index);
        })
      }
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
        .backgroundColor('#F8F8F8')

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
      Image($rawfile('icons/icon_back.svg'))
        .width(20)
        .height(20)
        .fillColor('#333333')
        .margin({ left: 4 })
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
        Image($rawfile('icons/icon_share.svg'))
          .width(20)
          .height(20)
          .fillColor('#333333')
          .margin({ right: 4 })
          .onClick(() => {
            hilog.info(DOMAIN, TAG, 'Share button clicked');
            this.onShare();
          })
      } else {
        Row().width(20).height(20).margin({ right: 4 })
      }
      }
    }
  }
}
