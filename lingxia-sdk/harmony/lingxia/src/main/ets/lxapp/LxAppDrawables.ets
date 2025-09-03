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

