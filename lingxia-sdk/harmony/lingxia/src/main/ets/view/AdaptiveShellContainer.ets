import {
  AdaptiveSizeClass,
  GpuCanvasOrigin,
  GpuCanvasSize,
  HingeRect,
  SIDEBAR_EXPANDED_VP,
  SIDEBAR_RAIL_VP,
  observeFoldStatus,
  readLocalHinge,
  resolveSizeClass,
  resolveSwitcherForm,
} from './GpuFoldAdapter';

/**
 * Responsive host shell container for HarmonyOS phones, foldables, tri-folds, and tablets.
 *
 * Uses zero device-class hardcoding: adapts strictly by live container width
 * (`compact < 600 vp`, `regular >= 600 vp` with ±24 vp hysteresis), expands the
 * sidebar only when `freeWidth >= 820 vp` (otherwise 56 vp rail, no host Pins on mobile),
 * snaps docked multi-tab aside panels to vertical fold creases, and collapses hidden
 * asides to a right-edge `‹ N` ear.
 *
 * Not yet mounted by the HarmonyOS host: pages still present one main at a time.
 */
@Component
export struct AdaptiveShellContainer {
  @Prop mainCount: number = 1;
  @Prop asideTabs: string[] = [];
  @Prop activeAsideTab: string = '';
  @Prop densityPixels: number = 3;
  @BuilderParam sidebarContent: () => void = this.emptySlot;
  @BuilderParam mainContent: () => void = this.emptySlot;
  @BuilderParam asideContent: () => void = this.emptySlot;
  onSelectAsideTab?: (id: string) => void;
  onHingeChange?: (hinge: HingeRect | null) => void;

  @State private windowWidthVp: number = 390;
  @State private windowHeightVp: number = 844;
  @State private sizeClass: AdaptiveSizeClass = 'compact';
  @State private asideCollapsed: boolean = false;
  @State private asideMaximized: boolean = false;
  @State private hinge: HingeRect | null = null;

  private unsubscribeFold?: () => void;

  @Builder
  private emptySlot(): void {}

  aboutToAppear(): void {
    this.refreshHinge();
    this.unsubscribeFold = observeFoldStatus(() => {
      this.refreshHinge();
    });
  }

  aboutToDisappear(): void {
    this.unsubscribeFold?.();
    this.unsubscribeFold = undefined;
  }

  private refreshHinge(): void {
    const origin: GpuCanvasOrigin = { x: 0, y: 0 };
    const size: GpuCanvasSize = {
      width: this.windowWidthVp * this.densityPixels,
      height: this.windowHeightVp * this.densityPixels,
    };
    const next = readLocalHinge(origin, size, this.densityPixels);
    this.hinge = next;
    this.onHingeChange?.(next);
  }

  private computeAsideWidthVp(): number {
    if (this.sizeClass !== 'regular' || this.asideTabs.length === 0 || this.asideCollapsed) {
      return 0;
    }
    if (this.asideMaximized) {
      return Math.max(360, this.windowWidthVp - (this.mainCount > 1 ? SIDEBAR_RAIL_VP : 0));
    }
    if (this.hinge && this.hinge.orientation === 'vertical') {
      return Math.max(240, this.windowWidthVp - (this.hinge.x + this.hinge.width));
    }
    return Math.min(360, this.windowWidthVp * 0.45);
  }

  private computeSidebarWidthVp(asideWidthVp: number): number {
    const freeWidthVp = Math.max(0, this.windowWidthVp - asideWidthVp);
    const form = resolveSwitcherForm(this.sizeClass, this.mainCount, freeWidthVp);
    if (form === 'sidebar') return SIDEBAR_EXPANDED_VP;
    if (form === 'rail') return SIDEBAR_RAIL_VP;
    return 0;
  }

  build() {
    Stack({ alignContent: Alignment.End }) {
      Row() {
        if (this.computeSidebarWidthVp(this.computeAsideWidthVp()) > 0) {
          Column() {
            this.sidebarContent();
          }
          .width(this.computeSidebarWidthVp(this.computeAsideWidthVp()))
          .height('100%')
        }

        Column() {
          this.mainContent();
        }
        .layoutWeight(1)
        .height('100%')

        if (this.computeAsideWidthVp() > 0) {
          Column() {
            Row({ space: 6 }) {
              Row({ space: 6 }) {
                ForEach(this.asideTabs, (tabId: string) => {
                  Text(tabId)
                    .fontSize(12)
                    .fontWeight(tabId === this.activeAsideTab ? FontWeight.Bold : FontWeight.Normal)
                    .padding({ left: 8, right: 8, top: 4, bottom: 4 })
                    .onClick(() => this.onSelectAsideTab?.(tabId))
                }, (tabId: string) => tabId)
              }
              .layoutWeight(1)

              Text('⤢')
                .fontSize(13)
                .padding({ left: 6, right: 6, top: 4, bottom: 4 })
                .onClick(() => {
                  this.asideMaximized = !this.asideMaximized;
                })

              Text('⇥')
                .fontSize(13)
                .padding({ left: 6, right: 6, top: 4, bottom: 4 })
                .onClick(() => {
                  this.asideCollapsed = true;
                })
            }
            .height(36)
            .width('100%')
            .padding({ left: 8, right: 8 })

            Column() {
              this.asideContent();
            }
            .layoutWeight(1)
            .width('100%')
          }
          .width(this.computeAsideWidthVp())
          .height('100%')
        }
      }
      .width('100%')
      .height('100%')

      if (this.asideTabs.length > 0 && (this.sizeClass === 'compact' || this.asideCollapsed)) {
        Button(`‹ ${this.asideTabs.length}`)
          .width(28)
          .height(56)
          .fontSize(11)
          .onClick(() => {
            this.asideCollapsed = false;
          })
      }
    }
    .width('100%')
    .height('100%')
    .onAreaChange((_oldArea, newArea) => {
      const w = Number(newArea.width);
      const h = Number(newArea.height);
      if (w > 0 && h > 0) {
        this.windowWidthVp = w;
        this.windowHeightVp = h;
        this.sizeClass = resolveSizeClass(this.sizeClass, w);
        this.refreshHinge();
      }
    })
  }
}
