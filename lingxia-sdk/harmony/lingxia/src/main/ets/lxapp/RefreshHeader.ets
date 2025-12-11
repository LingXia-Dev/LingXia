@Component
export struct RefreshHeader {
  @Watch('onRefreshingChange') @Prop isRefreshing: boolean
  @State loadingStep: number = 0
  private intervalId: number = -1

  aboutToAppear() {
    if (this.isRefreshing) {
      this.startAnimation()
    }
  }

  aboutToDisappear() {
    this.stopAnimation()
  }

  onRefreshingChange() {
    if (this.isRefreshing) {
      this.startAnimation()
    } else {
      this.stopAnimation()
    }
  }

  startAnimation() {
    this.stopAnimation()
    this.intervalId = setInterval(() => {
      animateTo({ duration: 200, curve: Curve.EaseInOut }, () => {
        this.loadingStep = (this.loadingStep + 1) % 3
      })
    }, 200)
  }

  stopAnimation() {
    if (this.intervalId !== -1) {
      clearInterval(this.intervalId)
      this.intervalId = -1
    }
    this.loadingStep = 0
  }

  build() {
    Row() {
      ForEach([0, 1, 2], (index: number) => {
        Circle()
          .width(8)
          .height(8)
          .fill('#888888')
          .margin({ left: 4, right: 4 })
          .opacity(this.getOpacity(index))
          .scale(this.getScale(index))
      })
    }
    .width('100%')
    .height(64)
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }

  getOpacity(index: number): number {
    if (!this.isRefreshing) return 0.4;
    return this.loadingStep === index ? 1.0 : 0.3
  }

  getScale(index: number): ScaleOptions {
    if (!this.isRefreshing) return { x: 1, y: 1 };
    return this.loadingStep === index ? { x: 1.2, y: 1.2 } : { x: 1, y: 1 }
  }
}