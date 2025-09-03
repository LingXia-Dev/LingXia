/**
 * Custom drawable components for LingXia navigation buttons
 */

/**
 * Custom back arrow component using Canvas for precise drawing
 */
@Component
export struct BackArrowIcon {
  @Prop color: ResourceColor = Color.Black
  @Prop iconSize: number = 20

  build() {
    Canvas(this.canvasContext)
      .width(36)
      .height(36)
      .onReady(() => {
        this.drawBackArrow()
      })
  }

  private canvasContext = new CanvasRenderingContext2D(new RenderingContextSettings(true))

  private drawBackArrow() {
    const ctx = this.canvasContext

    // Clear canvas
    ctx.clearRect(0, 0, 36, 36)

    // Set arrow style
    ctx.strokeStyle = 'rgba(0, 0, 0, 0.8)' // Black arrow
    ctx.lineWidth = 2.5
    ctx.lineCap = 'round'
    ctx.lineJoin = 'round'

    // Draw back arrow centered in canvas
    const centerX = 18
    const centerY = 18
    const arrowSize = 8 // Larger arrow for better visibility

    ctx.beginPath()
    // Arrow pointing left: >
    ctx.moveTo(centerX + arrowSize/2, centerY - arrowSize)  // Top right
    ctx.lineTo(centerX - arrowSize/2, centerY)             // Center left (tip)
    ctx.lineTo(centerX + arrowSize/2, centerY + arrowSize)  // Bottom right

    ctx.stroke()
  }
}

/**
 * Custom home icon component - simple and clean outline
 * Back to the working original design with hollow effect
 */
@Component
export struct HomeIcon {
  @Prop color: ResourceColor = Color.Black
  @Prop iconSize: number = 18
  private readonly strokeWidth: number = 1.5

  build() {
    // Use Canvas for precise home icon drawing with larger size for clarity
    Canvas(this.canvasContext)
      .width(36)
      .height(36)
      .onReady(() => {
        this.drawHomeIcon()
      })
  }

  private canvasContext = new CanvasRenderingContext2D(new RenderingContextSettings(true))

  private drawHomeIcon() {
    const ctx = this.canvasContext

    // Clear canvas
    ctx.clearRect(0, 0, 36, 36)

    // Draw house outline only (no background circle)
    ctx.strokeStyle = 'rgba(0, 0, 0, 0.8)' // Black outline
    ctx.lineWidth = 2.5
    ctx.lineCap = 'round'
    ctx.lineJoin = 'round'

    // Draw house outline centered in canvas
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
 * Navigation button component with frosted glass effect
 * Matches the styling from iOS and Android implementations
 */
@Component
export struct NavigationButton {
  @Prop isBackButton: boolean = true
  @Prop color: ResourceColor = Color.Black
  @Prop bgColor: ResourceColor = Color.Transparent
  onPressed?: () => void
  @State isPressed: boolean = false

  build() {
    Button() {
      Stack() {
        if (this.isBackButton) {
          BackArrowIcon({ color: this.color, iconSize: 24 })
        } else {
          HomeIcon({
            color: this.color,
            iconSize: 24
         })
        }
      }
      .width('100%')
      .height('100%')
      .alignContent(Alignment.Center)
    }
    .backgroundColor(this.bgColor)
    .borderRadius('50%') // Use percentage for perfect circle
    .padding(0) // Remove default button padding
    .scale({ x: this.isPressed ? 0.95 : 1.0, y: this.isPressed ? 0.95 : 1.0 })
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
        .fill('#000000')

      Circle()
        .width(5)
        .height(5)
        .fill('#000000')
        .margin({ left: 4 })

      Circle()
        .width(3)
        .height(3)
        .fill('#000000')
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
        .stroke('#000000')
        .strokeWidth(2.5) // Thicker stroke matching Android

      // Inner dot (thick dot in center) - radius should be outer radius / 2.5
      Circle()
        .width(6.5)
        .height(6.5)
        .fill('#000000')
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
  @Prop statusBarHeight: number = 44
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
        .width(44)
        .height(32)
        .backgroundColor(Color.Transparent)
        .onClick(() => {
          if (this.onMorePressed) {
            this.onMorePressed()
          }
        })

        Column()
          .width(0.5)
          .height(16)
          .backgroundColor('#DDDDDD')

        Button() {
          CloseIcon()
        }
        .width(44)
        .height(32)
        .backgroundColor(Color.Transparent)
        .onClick(() => {
          if (this.onClosePressed) {
            this.onClosePressed()
          }
        })
      }
      .backgroundColor('rgba(255,255,255,0.9)')
      .borderRadius(16)
      .border({ width: 0.5, color: '#DDDDDD' })
    }
    .position({ x: '100%', y: 44 })
    .translate({ x: -102, y: 6 })
    .width(102) // 44+0.5+44+padding
    .height(32)
    .zIndex(9999)
  }

  @Builder
  buildInlineCapsule(): void {
    Row() {
      Blank()

      Row() {
        Button() {
          ThreeDotsIcon()
        }
        .width(44)
        .height(32)
        .backgroundColor(Color.Transparent)
        .onClick(() => {
          if (this.onMorePressed) {
            this.onMorePressed()
          }
        })

        Column()
          .width(0.5)
          .height(16)
          .backgroundColor('#DDDDDD')

        Button() {
          CloseIcon()
        }
        .width(44)
        .height(32)
        .backgroundColor(Color.Transparent)
        .enabled(true)
        .onClick(() => {
          if (this.onClosePressed) {
            this.onClosePressed()
          }
        })
      }
      .backgroundColor('rgba(255,255,255,0.9)')
      .borderRadius(16)
      .border({ width: 0.5, color: '#DDDDDD' })
      .margin({
        top: this.statusBarHeight + 8,
        right: 12
      })
    }
    .width('100%')
    .height(32) // Just the button height, not including margin
    .backgroundColor(Color.Transparent)
    .zIndex(9999) // High zIndex to ensure it's always on top
    .position({ x: 0, y: 0 }) // Position absolutely
  }
}

