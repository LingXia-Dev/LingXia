/**
 * Custom drawable components for LingXia navigation buttons
 */

import { noteCapsuleFrame } from './capsuleGeometry';

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
const CAPSULE_TRAILING_MARGIN = 12;
// Keep in sync with NavigationBar.NAVIGATION_BAR_CONTENT_HEIGHT.
const NAV_BAR_CONTENT_HEIGHT = 44;

// Capsule centerY matches the navbar title centerY:
// top = statusBar + (navBarContentHeight - capsuleHeight) / 2
export function calculateCapsuleTop(statusBarHeight: number): number {
  return statusBarHeight + (NAV_BAR_CONTENT_HEIGHT - CAPSULE_BUTTON_HEIGHT) / 2;
}

// Common colors
const COLOR_TRANSPARENT = Color.Transparent;

// System constants
const SYSTEM_STATUS_BAR_HEIGHT = 44;

function argbColor(colorValue: number): ResourceColor {
  const alpha = (colorValue >>> 24) & 0xFF;
  const red = (colorValue >>> 16) & 0xFF;
  const green = (colorValue >>> 8) & 0xFF;
  const blue = colorValue & 0xFF;
  if (alpha === 255) {
    return `#${red.toString(16).padStart(2, '0')}${green.toString(16).padStart(2, '0')}${blue.toString(16).padStart(2, '0')}`;
  }
  return `rgba(${red}, ${green}, ${blue}, ${alpha / 255})`;
}


/**
 * Navigation button component
 */
@Component
export struct NavigationButton {
  @Prop isBackButton: boolean = true
  @Prop color: number = 0xFF000000
  onPressed?: () => void
  @State isPressed: boolean = false

  build() {
    Column() {
      Image($rawfile(this.isBackButton ? 'icons/icon_back.svg' : 'icons/icon_home.svg'))
        .width(20)
        .height(20)
        .objectFit(ImageFit.Contain)
        .fillColor(argbColor(this.color))
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
  @Prop color: number = 0xFF000000

  build() {
    Image($rawfile('icons/icon_capsule_menu.svg'))
      .width(28)
      .height(20)
      .objectFit(ImageFit.Contain)
      .fillColor(argbColor(this.color))
  }
}

/**
 * Close icon component for capsule button
 */
@Component
export struct CloseIcon {
  @Prop color: number = 0xFF000000

  build() {
    Image($rawfile('icons/icon_capsule_close.svg'))
      .width(28)
      .height(20)
      .objectFit(ImageFit.Contain)
      .fillColor(argbColor(this.color))
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
  @Prop capsuleBackgroundColor: number = 0xFFFFFFFF
  @Prop capsuleForegroundColor: number = 0xFF000000
  @Prop capsuleDividerColor: number = 0xFFD1D1D6
  @Prop capsuleInteractionColor: number = 0xFFE5E5EA
  @State morePressed: boolean = false
  @State closePressed: boolean = false

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
    .position({ x: '100%', y: calculateCapsuleTop(this.statusBarHeight) })
    .translate({ x: -(CAPSULE_BUTTON_TOTAL_WIDTH + CAPSULE_TRAILING_MARGIN), y: 0 })
    .width(CAPSULE_BUTTON_TOTAL_WIDTH)
    .height(CAPSULE_BUTTON_HEIGHT)
    .zIndex(CAPSULE_BUTTON_Z_INDEX)
  }

  @Builder
  buildInlineCapsule(): void {
    Row() {
      Blank()

      this.buildCapsuleRow(calculateCapsuleTop(this.statusBarHeight), CAPSULE_TRAILING_MARGIN)
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
        ThreeDotsIcon({ color: this.capsuleForegroundColor })
      }
      .width(CAPSULE_BUTTON_BUTTON_WIDTH)
      .height(CAPSULE_BUTTON_HEIGHT)
      .padding(0)
      .backgroundColor(this.morePressed ? argbColor(this.capsuleInteractionColor) : COLOR_TRANSPARENT)
      .onTouch((event: TouchEvent) => {
        this.morePressed = event.type === TouchType.Down
      })
      .onClick(() => {
        if (this.onMorePressed) {
          this.onMorePressed()
        }
      })

      Column()
        .width(CAPSULE_BUTTON_DIVIDER_WIDTH)
        .height(CAPSULE_BUTTON_DIVIDER_HEIGHT)
        .backgroundColor(argbColor(this.capsuleDividerColor))

      Button() {
        CloseIcon({ color: this.capsuleForegroundColor })
      }
      .width(CAPSULE_BUTTON_BUTTON_WIDTH)
      .height(CAPSULE_BUTTON_HEIGHT)
      .padding(0)
      .backgroundColor(this.closePressed ? argbColor(this.capsuleInteractionColor) : COLOR_TRANSPARENT)
      .onTouch((event: TouchEvent) => {
        this.closePressed = event.type === TouchType.Down
      })
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
    .backgroundColor(argbColor(this.capsuleBackgroundColor))
    .borderRadius(CAPSULE_BUTTON_BORDER_RADIUS)
    .border({
      width: CAPSULE_BUTTON_BORDER_WIDTH,
      color: argbColor(this.capsuleDividerColor)
    })
    .margin({
      top: marginTop,
      right: marginRight
    })
    .onAreaChange((_, area) => {
      noteCapsuleFrame(
        Number((area.globalPosition?.x as number) ?? 0),
        Number((area.globalPosition?.y as number) ?? 0),
        Number(area.width),
        Number(area.height)
      )
    })
  }
}
