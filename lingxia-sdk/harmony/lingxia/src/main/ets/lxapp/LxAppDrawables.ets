/**
 * Custom drawable components for LingXia navigation buttons
 */

// Capsule button constants
const CAPSULE_BUTTON_TOTAL_WIDTH = 84.5;
const CAPSULE_BUTTON_HEIGHT = 32;
const CAPSULE_BUTTON_BUTTON_WIDTH = 38;
const CAPSULE_BUTTON_EDGE_PADDING = 4;
const CAPSULE_BUTTON_DIVIDER_WIDTH = 0.5;
const CAPSULE_BUTTON_DIVIDER_HEIGHT = 20;
const CAPSULE_BUTTON_BORDER_RADIUS = 16;
const CAPSULE_BUTTON_BORDER_WIDTH = 0.5;
const CAPSULE_BUTTON_Z_INDEX = 9999;

// Common colors
const COLOR_BLACK = '#000000';
const COLOR_TRANSPARENT = Color.Transparent;
const COLOR_LIGHT_BORDER = '#DDDDDD';
const COLOR_CAPSULE_BACKGROUND = 'rgba(255,255,255,0.9)';

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
    Image($rawfile('icons/icon_capsule_menu.svg'))
      .width(28)
      .height(20)
      .objectFit(ImageFit.Contain)
  }
}

/**
 * Close icon component for capsule button
 */
@Component
export struct CloseIcon {
  build() {
    Image($rawfile('icons/icon_capsule_close.svg'))
      .width(28)
      .height(20)
      .objectFit(ImageFit.Contain)
  }
}

/**
 * Capsule button component with more and close buttons
 * Used for non-home apps
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
      this.buildCapsuleRow()
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

      this.buildCapsuleRow(this.statusBarHeight + 8, 12)
    }
    .width('100%')
    .height(CAPSULE_BUTTON_HEIGHT) // Just the button height, not including margin
    .backgroundColor(COLOR_TRANSPARENT)
    .zIndex(CAPSULE_BUTTON_Z_INDEX) // High zIndex to ensure it's always on top
    .position({ x: 0, y: 0 }) // Position absolutely
  }

  @Builder
  buildCapsuleRow(marginTop: number = 0, marginRight: number = 0): void {
    Row() {
      Button() {
        ThreeDotsIcon()
      }
      .width(CAPSULE_BUTTON_BUTTON_WIDTH)
      .height(CAPSULE_BUTTON_HEIGHT)
      .padding(0)
      .backgroundColor(COLOR_TRANSPARENT)
      .onClick(() => {
        if (this.onMorePressed) {
          this.onMorePressed()
        }
      })

      Column()
        .width(CAPSULE_BUTTON_DIVIDER_WIDTH)
        .height(CAPSULE_BUTTON_DIVIDER_HEIGHT)
        .backgroundColor(COLOR_LIGHT_BORDER)

      Button() {
        CloseIcon()
      }
      .width(CAPSULE_BUTTON_BUTTON_WIDTH)
      .height(CAPSULE_BUTTON_HEIGHT)
      .padding(0)
      .backgroundColor(COLOR_TRANSPARENT)
      .onClick(() => {
        if (this.onClosePressed) {
          this.onClosePressed()
        }
      })
    }
    .padding({
      left: CAPSULE_BUTTON_EDGE_PADDING,
      right: CAPSULE_BUTTON_EDGE_PADDING
    })
    .backgroundColor(COLOR_CAPSULE_BACKGROUND)
    .borderRadius(CAPSULE_BUTTON_BORDER_RADIUS)
    .border({
      width: CAPSULE_BUTTON_BORDER_WIDTH,
      color: COLOR_LIGHT_BORDER
    })
    .margin({
      top: marginTop,
      right: marginRight
    })
  }
}
