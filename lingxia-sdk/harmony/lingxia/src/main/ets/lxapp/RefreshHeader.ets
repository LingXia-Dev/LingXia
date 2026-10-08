import { accessibility } from '@kit.AccessibilityKit';

/**
 * Pull-to-refresh header: a ring that grows with the pull, then a spinner
 * while refreshing. With the system's reduce-animations setting on it holds
 * a still ring.
 */
@Component
export struct RefreshHeader {
  @Watch('onRefreshingChange') @Prop isRefreshing: boolean = false
  // Share of the trigger distance pulled so far, 0..1.
  @Prop pullProgress: number = 0
  // `#RRGGBB` from the host theme; empty keeps the system colour.
  @Prop spinnerColor: string = ''
  @State private animated: boolean = true

  aboutToAppear() {
    this.onRefreshingChange()
  }

  onRefreshingChange() {
    if (this.isRefreshing) {
      this.animated = !this.animationsReduced()
    }
  }

  /** The accessibility "reduce animations" setting is on. */
  private animationsReduced(): boolean {
    try {
      return accessibility.isAnimationReduceEnabledSync();
    } catch (_error) {
      return false;
    }
  }

  private color(): ResourceColor {
    return this.spinnerColor.length > 0 ? this.spinnerColor : $r('sys.color.ohos_id_color_text_secondary');
  }

  build() {
    Stack() {
      if (this.isRefreshing && this.animated) {
        LoadingProgress()
          .width(32)
          .height(32)
          .color(this.color())
      } else {
        Progress({
          value: this.isRefreshing ? 75 : 75 * Math.min(1, Math.max(0, this.pullProgress)),
          total: 100,
          type: ProgressType.Ring
        })
          .width(24)
          .height(24)
          .color(this.color())
          .backgroundColor(Color.Transparent)
          .style({ strokeWidth: 2.5 })
          .opacity(this.isRefreshing || this.pullProgress >= 1 ? 1 : 0.4 + 0.5 * this.pullProgress)
      }
    }
    .width('100%')
    .height(64)
    .alignContent(Alignment.Center)
  }
}
