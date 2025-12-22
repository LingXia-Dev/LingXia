/**
 * Custom drawable components for LingXia navigation buttons
 */

// Custom icon drawing constants
const CUSTOM_ICON_STROKE_WIDTH = 1.8;

// Capsule button constants
const CAPSULE_BUTTON_TOTAL_WIDTH = 102;
const CAPSULE_BUTTON_HEIGHT = 32;
const CAPSULE_BUTTON_BUTTON_WIDTH = 44;
const CAPSULE_BUTTON_BORDER_RADIUS = 16;
const CAPSULE_BUTTON_BORDER_WIDTH = 0.5;
const CAPSULE_BUTTON_Z_INDEX = 9999;

// Common colors
const COLOR_BLACK = '#000000';
const COLOR_TRANSPARENT = Color.Transparent;
const COLOR_LIGHT_BORDER = '#DDDDDD';

// System constants
const SYSTEM_STATUS_BAR_HEIGHT = 44;


/**
 * Navigation button component
 */
@Component
export struct NavigationButton {
  @Prop isBackButton: boolean = true
  onPressed?: () => void
  @State isPressed: boolean = false

  build() {
    Column() {
      Image($rawfile(this.isBackButton ? 'icons/icon_back.svg' : 'icons/icon_home.svg'))
        .width(20)
        .height(20)
        .objectFit(ImageFit.Contain)
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Transparent)
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .scale({
      x: this.isPressed ? 0.95 : 1.0,
      y: this.isPressed ? 0.95 : 1.0
    })
    .animation({
      duration: 100,
      curve: Curve.EaseInOut
    })
    .onTouch((event: TouchEvent) => {
      if (event.type === TouchType.Down) {
        this.isPressed = true
      } else if (event.type === TouchType.Up || event.type === TouchType.Cancel) {
        this.isPressed = false
      }
    })
    .onClick(() => {
      if (this.onPressed) {
        this.onPressed()
      }
    })
    .hitTestBehavior(HitTestMode.Block)
  }
}

/**
 * Three dots icon component for capsule button
 */
@Component
export struct ThreeDotsIcon {
  build() {
    Row() {
      // Three custom drawn dots - center dot larger, side dots smaller (matching Android)
      Circle()
        .width(3)
        .height(3)
        .fill(COLOR_BLACK)

      Circle()
        .width(5)
        .height(5)
        .fill(COLOR_BLACK)
        .margin({ left: 4 })

      Circle()
        .width(3)
        .height(3)
        .fill(COLOR_BLACK)
        .margin({ left: 4 })
    }
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }
}

/**
 * Close icon component for capsule button (outer circle LINE with inner dot, transparent middle)
 */
@Component
export struct CloseIcon {
  build() {
    Stack() {
      // Outer circle - use stroke instead of border to ensure circular shape
      Circle()
        .width(16)
        .height(16)
        .fillOpacity(0) // Transparent fill
        .stroke(COLOR_BLACK)
        .strokeWidth(CUSTOM_ICON_STROKE_WIDTH) // Thicker stroke matching Android

      // Inner dot (thick dot in center) - radius should be outer radius / 2.5
      Circle()
        .width(6.5)
        .height(6.5)
        .fill(COLOR_BLACK)
    }
  }
}

/**
 * Capsule button component with more and close buttons
 * Used for non-home LxApps
 */
@Component
export struct CapsuleButton {
  onMorePressed?: () => void
  onClosePressed?: () => void
  @Prop statusBarHeight: number = SYSTEM_STATUS_BAR_HEIGHT
  @Prop standalone: boolean = false

  build() {
    if (this.standalone) {
      this.buildStandaloneCapsule()
    } else {
      this.buildInlineCapsule()
    }
  }

  @Builder
  buildStandaloneCapsule(): void {
    Stack() {
      Row() {
        Button() {
          ThreeDotsIcon()
        }
        .width(CAPSULE_BUTTON_BUTTON_WIDTH)
        .height(CAPSULE_BUTTON_HEIGHT)
        .backgroundColor(COLOR_TRANSPARENT)
        .onClick(() => {
          if (this.onMorePressed) {
            this.onMorePressed()
          }
        })

        Column()
          .width(0.5)
          .height(16)
          .backgroundColor(COLOR_LIGHT_BORDER)

        Button() {
          CloseIcon()
        }
        .width(CAPSULE_BUTTON_BUTTON_WIDTH)
        .height(CAPSULE_BUTTON_HEIGHT)
        .backgroundColor(COLOR_TRANSPARENT)
        .onClick(() => {
          if (this.onClosePressed) {
            this.onClosePressed()
          }
        })
      }
      .backgroundColor('rgba(255,255,255,0.9)')
      .borderRadius(CAPSULE_BUTTON_BORDER_RADIUS)
      .border({
        width: CAPSULE_BUTTON_BORDER_WIDTH,
        color: COLOR_LIGHT_BORDER
      })
    }
    .position({ x: '100%', y: SYSTEM_STATUS_BAR_HEIGHT })
    .translate({ x: -CAPSULE_BUTTON_TOTAL_WIDTH, y: 8 })
    .width(CAPSULE_BUTTON_TOTAL_WIDTH)
    .height(CAPSULE_BUTTON_HEIGHT)
    .zIndex(CAPSULE_BUTTON_Z_INDEX)
  }

  @Builder
  buildInlineCapsule(): void {
    Row() {
      Blank()

      Row() {
        Button() {
          ThreeDotsIcon()
        }
        .width(CAPSULE_BUTTON_BUTTON_WIDTH)
        .height(CAPSULE_BUTTON_HEIGHT)
        .backgroundColor(COLOR_TRANSPARENT)
        .onClick(() => {
          if (this.onMorePressed) {
            this.onMorePressed()
          }
        })

        Column()
          .width(0.5)
          .height(16)
          .backgroundColor(COLOR_LIGHT_BORDER)

        Button() {
          CloseIcon()
        }
        .width(CAPSULE_BUTTON_BUTTON_WIDTH)
        .height(CAPSULE_BUTTON_HEIGHT)
        .backgroundColor(COLOR_TRANSPARENT)
        .enabled(true)
        .onClick(() => {
          if (this.onClosePressed) {
            this.onClosePressed()
          }
        })
      }
      .backgroundColor('rgba(255,255,255,0.9)')
      .borderRadius(CAPSULE_BUTTON_BORDER_RADIUS)
      .border({
        width: CAPSULE_BUTTON_BORDER_WIDTH,
        color: COLOR_LIGHT_BORDER
      })
      .margin({
        top: this.statusBarHeight + 8,
        right: 12
      })
    }
    .width('100%')
    .height(CAPSULE_BUTTON_HEIGHT) // Just the button height, not including margin
    .backgroundColor(COLOR_TRANSPARENT)
    .zIndex(CAPSULE_BUTTON_Z_INDEX) // High zIndex to ensure it's always on top
    .position({ x: 0, y: 0 }) // Position absolutely
  }
}
