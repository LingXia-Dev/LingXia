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

    // Render at 2x resolution for better zoom quality
    const renderScale = 2.0;

    for (let i = 0; i < this.pageCount; i++) {
      try {
        const page = this.document.getPage(i);

        // Try high-resolution rendering using getBox for dimensions
        try {
          const box: pdfService.PdfRect = page.getBox(pdfService.BoxType.BOX_MEDIA);
          // PdfRect uses left, right, top, bottom
          const pageWidth: number = Math.abs(box.right - box.left);
          const pageHeight: number = Math.abs(box.bottom - box.top);

          // Create scale matrix with target dimensions (2x resolution)
          const scaleMatrix = new pdfService.PdfMatrix();
          scaleMatrix.width = pageWidth * renderScale;
          scaleMatrix.height = pageHeight * renderScale;
          scaleMatrix.rotate = 0;

          // Use getCustomPagePixelMap with scale matrix for higher resolution
          // Parameters: matrix, isGray (false for color), drawAnnotations (true)
          const map = page.getCustomPagePixelMap(scaleMatrix, false, true);
          maps.push(map);
          hilog.info(DOMAIN, TAG, `Page ${i} loaded at ${renderScale}x (${pageWidth}x${pageHeight})`);
        } catch (highResErr) {
          // Fallback to standard resolution
          hilog.warn(DOMAIN, TAG, `Page ${i} high-res failed, using standard: ${highResErr}`);
          const map = page.getPagePixelMap();
          maps.push(map);
        }
      } catch (e) {
        hilog.error(DOMAIN, TAG, `loadPage ${i} failed: ${e}`);
      }
    }
    this.pagePixelMaps = maps;
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
      // Zoomable content container - applies scale/translate to entire content
      Column() {
        // PDF content area - continuous scroll with List
        if (this.pagePixelMaps.length > 0) {
          List({ scroller: this.scroller }) {
            ForEach(this.pagePixelMaps, (pixelMap: image.PixelMap, index: number) => {
              ListItem() {
                this.renderPage(pixelMap, index)
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
  private renderPage(pixelMap: image.PixelMap, index: number) {
    // Simple page rendering - zoom is handled at container level
    Column() {
      Image(pixelMap)
        .width('100%')
        .objectFit(ImageFit.Contain)
        .backgroundColor('#FFFFFF')
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
