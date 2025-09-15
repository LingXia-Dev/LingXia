/**
 * Custom drawable components for LingXia navigation buttons
 */

// Custom icon drawing constants
const CUSTOM_ICON_CANVAS_SIZE = 36;
const CUSTOM_ICON_STROKE_WIDTH = 1.8; // Thinner
const CUSTOM_ICON_BACK_ARROW_SIZE = 6; // Much smaller arrow opening
const CUSTOM_ICON_HOME_ICON_SIZE = 16; // Smaller home icon

// Capsule button constants
const CAPSULE_BUTTON_TOTAL_WIDTH = 102;
const CAPSULE_BUTTON_HEIGHT = 32;
const CAPSULE_BUTTON_BUTTON_WIDTH = 44;
const CAPSULE_BUTTON_BORDER_RADIUS = 16;
const CAPSULE_BUTTON_BORDER_WIDTH = 0.5;
const CAPSULE_BUTTON_Z_INDEX = 9999;

// Common colors
const COLOR_WHITE = '#ffffff';
const COLOR_BLACK = '#000000';
const COLOR_TRANSPARENT = Color.Transparent;
const COLOR_LIGHT_BORDER = '#DDDDDD';
const COLOR_ICON_STROKE = '#666666';

// Animation constants
const ANIMATION_DURATION_FAST = 100;
const ANIMATION_BUTTON_PRESS_SCALE = 0.95;
const ANIMATION_CURVE_EASE_IN_OUT = Curve.EaseInOut;  // Smooth ease in/out curve

// System constants
const SYSTEM_STATUS_BAR_HEIGHT = 44;

/**
 * Custom back arrow component using Canvas for precise drawing
 */
@Component
export struct BackArrowIcon {
  @Prop color: ResourceColor = COLOR_BLACK
  @Prop iconSize: number = CUSTOM_ICON_BACK_ARROW_SIZE
  @State private refreshKey: number = 0

  build() {
    Canvas(this.canvasContext)
      .width(CUSTOM_ICON_CANVAS_SIZE)
      .height(CUSTOM_ICON_CANVAS_SIZE)
      .onReady(() => {
        this.drawBackArrow()
      })
      .onAreaChange(() => {
        // Force redraw when area changes
        this.refreshKey++
        this.drawBackArrow()
      })
  }

  private canvasContext = new CanvasRenderingContext2D(new RenderingContextSettings(true))

  private drawBackArrow() {
    const ctx = this.canvasContext

    // Clear canvas
    ctx.clearRect(0, 0, CUSTOM_ICON_CANVAS_SIZE, CUSTOM_ICON_CANVAS_SIZE)

    // Set arrow style
    ctx.strokeStyle = COLOR_ICON_STROKE
    ctx.lineWidth = CUSTOM_ICON_STROKE_WIDTH
    ctx.lineCap = 'round'
    ctx.lineJoin = 'round'

    const centerX = CUSTOM_ICON_CANVAS_SIZE / 2 - 4 // Move more to the left
    const centerY = CUSTOM_ICON_CANVAS_SIZE / 2
    const chevronSize = CUSTOM_ICON_CANVAS_SIZE * 0.4
    const tipX = centerX - chevronSize * 0.3 //  tip position
    const baseX = centerX + chevronSize * 0.2 // base position
    const offsetY = chevronSize * 0.45 // vertical offset

    ctx.beginPath()
    // Arrow pointing left: <
    ctx.moveTo(baseX, centerY - offsetY)  // Top arm start
    ctx.lineTo(tipX, centerY)             // To tip
    ctx.moveTo(tipX, centerY)             // From tip
    ctx.lineTo(baseX, centerY + offsetY)  // Bottom arm end

    ctx.stroke()
  }
}

/**
 * Custom home icon component - simple and clean outline
 * Back to the working original design with hollow effect
 */
@Component
export struct HomeIcon {
  @Prop color: ResourceColor = COLOR_BLACK
  @Prop iconSize: number = CUSTOM_ICON_HOME_ICON_SIZE

  build() {
    // Use Canvas for precise home icon drawing with larger size for clarity
    Canvas(this.canvasContext)
      .width(CUSTOM_ICON_CANVAS_SIZE)
      .height(CUSTOM_ICON_CANVAS_SIZE)
      .onReady(() => {
        this.drawHomeIcon()
      })
  }

  private canvasContext = new CanvasRenderingContext2D(new RenderingContextSettings(true))

  private drawHomeIcon() {
    const ctx = this.canvasContext

    // Clear canvas
    ctx.clearRect(0, 0, CUSTOM_ICON_CANVAS_SIZE, CUSTOM_ICON_CANVAS_SIZE)

    // Draw house outline only (no background circle)
    ctx.strokeStyle = COLOR_ICON_STROKE // Black outline
    ctx.lineWidth = CUSTOM_ICON_STROKE_WIDTH
    ctx.lineCap = 'round'
    ctx.lineJoin = 'round'

    // Draw house outline centered in 36x36 canvas
    ctx.beginPath()
    // Start from roof peak
    ctx.moveTo(18, 6)    // peak (centered)
    // Right roof slope
    ctx.lineTo(28, 14)   // right roof corner
    // Right wall down
    ctx.lineTo(28, 28)   // right bottom
    // Bottom right to door right
    ctx.lineTo(21, 28)   // door right bottom
    // Door right side up
    ctx.lineTo(21, 21)   // door right top
    // Door top
    ctx.lineTo(15, 21)   // door left top
    // Door left side down
    ctx.lineTo(15, 28)   // door left bottom
    // Bottom left
    ctx.lineTo(8, 28)    // left bottom
    // Left wall up
    ctx.lineTo(8, 14)    // left roof corner
    // Left roof slope back to peak
    ctx.lineTo(18, 6)    // back to peak
    ctx.closePath()

    // Draw only the outline (no fill)
    ctx.stroke()
  }
}

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
      if (this.isBackButton) {
        BackArrowIcon({
          color: COLOR_BLACK, // Fixed color
          iconSize: CUSTOM_ICON_BACK_ARROW_SIZE
        })
      } else {
        HomeIcon({
          color: COLOR_BLACK, // Fixed color
          iconSize: CUSTOM_ICON_HOME_ICON_SIZE
       })
      }
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
    .hitTestBehavior(HitTestMode.Block) // Ensure this component blocks and handles clicks
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

