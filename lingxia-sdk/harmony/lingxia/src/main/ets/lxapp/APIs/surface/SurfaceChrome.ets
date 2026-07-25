/** Shared close control for in-tree and sub-window surface presentations. */
@Component
export struct SurfaceCloseButton {
  @Prop surfaceRightVp: number = 0;
  @Prop surfaceTopVp: number = 0;
  onPressed?: () => void;
  @State private isPressed: boolean = false;

  build() {
    Stack() {
      Stack() {
        Column()
          .width('12vp')
          .height('1.5vp')
          .borderRadius('1vp')
          .backgroundColor('rgba(255,255,255,0.96)')
          .rotate({ angle: 45 })
        Column()
          .width('12vp')
          .height('1.5vp')
          .borderRadius('1vp')
          .backgroundColor('rgba(255,255,255,0.96)')
          .rotate({ angle: -45 })
      }
      .width('28vp')
      .height('28vp')
      .borderRadius('14vp')
      .backgroundColor('rgba(0,0,0,0.72)')
      .scale({ x: this.isPressed ? 0.92 : 1, y: this.isPressed ? 0.92 : 1 })
      .opacity(this.isPressed ? 0.78 : 1)
      .animation({ duration: 100, curve: Curve.EaseInOut })
    }
    .width('48vp')
    .height('48vp')
    .backgroundColor(Color.Transparent)
    .position({ x: this.surfaceRightVp - 50, y: this.surfaceTopVp + 2 })
    .zIndex(3)
    .hitTestBehavior(HitTestMode.Block)
    .onTouch((event: TouchEvent) => {
      if (event.type === TouchType.Down) {
        this.isPressed = true;
      } else if (event.type === TouchType.Up || event.type === TouchType.Cancel) {
        this.isPressed = false;
      }
    })
    .onClick(() => {
      this.isPressed = false;
      if (this.onPressed) {
        this.onPressed();
      }
    })
  }
}
