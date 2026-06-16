import { hilog } from '@kit.PerformanceAnalysisKit';
import { TabBarPosition, type TabBarItem, type TabBarState } from 'liblingxia.so';

// TabBar layout constants
const TAB_BAR_HORIZONTAL_HEIGHT = 72;    // Horizontal TabBar default height
const TAB_BAR_VERTICAL_WIDTH = 64;       // Vertical TabBar default width
const TAB_BAR_BORDER_WIDTH = 0.5;        // TabBar border width
const TAB_BAR_ITEM_SPACING = 16;         // TabBar item spacing
const TAB_BAR_ITEM_PADDING = 4;          // TabBar item internal padding

// TabBar padding configuration
const TAB_BAR_PADDING_HORIZONTAL_TOP = 8;
const TAB_BAR_PADDING_HORIZONTAL_BOTTOM = 12;
const TAB_BAR_PADDING_HORIZONTAL_LEFT = 16;
const TAB_BAR_PADDING_HORIZONTAL_RIGHT = 16;
const TAB_BAR_PADDING_VERTICAL_TOP = 20;     // includes status bar space
const TAB_BAR_PADDING_VERTICAL_BOTTOM = 8;
const TAB_BAR_PADDING_VERTICAL_LEFT = 8;
const TAB_BAR_PADDING_VERTICAL_RIGHT = 8;

// TabBar icon and text constants
const TAB_BAR_ITEM_ICON_SIZE = 24;                    // Tab icon size
const TAB_BAR_ITEM_ICON_CONTAINER_SIZE = 48;          // Tab icon container size
const TAB_BAR_ITEM_TEXT_SIZE_HORIZONTAL = 12;         // Tab text font size - horizontal layout
const TAB_BAR_ITEM_TEXT_SIZE_VERTICAL = 10;           // Tab text font size - vertical layout
const TAB_BAR_ITEM_ICON_TEXT_SPACING = 4;             // Icon to text spacing

// Badge configuration
const TAB_BAR_ITEM_BADGE_FONT_SIZE = 10;
const TAB_BAR_ITEM_BADGE_BORDER_RADIUS = 8;
const TAB_BAR_ITEM_BADGE_PADDING = 6;
const TAB_BAR_ITEM_BADGE_HEIGHT = 16;
const TAB_BAR_ITEM_BADGE_VERTICAL_PADDING = 2;
const TAB_BAR_ITEM_BADGE_POSITION_X = 32;  // Position at right of icon (icon center + icon_size/2 + offset)
const TAB_BAR_ITEM_BADGE_POSITION_Y = -8;

// Red dot configuration
const TAB_BAR_ITEM_RED_DOT_SIZE = 8;
const TAB_BAR_ITEM_RED_DOT_POSITION_X = 32;  // Position at right of icon (icon center + icon_size/2 + offset)
const TAB_BAR_ITEM_RED_DOT_POSITION_Y = -2;

// Text max lines
const TAB_BAR_ITEM_TEXT_MAX_LINES_VERTICAL = 2;
const TAB_BAR_ITEM_TEXT_MAX_LINES_HORIZONTAL = 1;

// Common colors
const COLOR_WHITE = '#ffffff';
const COLOR_BLACK = '#000000';
const COLOR_TRANSPARENT = Color.Transparent;
const COLOR_LIGHT_BORDER = '#DDDDDD';
const COLOR_BADGE_RED = '#FA5151';
const COLOR_RED_DOT = '#FA5151';
const COLOR_TEXT_SHADOW = 'rgba(255,255,255,0.8)';

// Text shadow configuration
const TEXT_SHADOW_RADIUS = 1;
const TEXT_SHADOW_OFFSET_X = 0;
const TEXT_SHADOW_OFFSET_Y = 1;

// System constants
const SYSTEM_SAFE_AREA_BOTTOM = 24;  // Safe area bottom extra spacing

const DOMAIN = 0x0000;
const TAG = 'LingXia.TabBar';

/**
 * TabBar component for LxApp
 * Uses cached static tabbar for performance while supporting dynamic updates
 */
@Component
export struct TabBar {
  @Prop tabbar: TabBarState;  // Cached static tabbar for performance
  @Prop appId: string = '';
  @Prop refreshKey: number = 0;  // Force refresh when data changes

  // Callback functions
  onTabSelected?: (appId: string, index: number, item: TabBarItem) => void;

  aboutToAppear() {
    hilog.info(DOMAIN, TAG, `TabBar created: ${this.tabbar.items.length} items, vertical: ${this.isVertical()}, transparent: ${this.isTransparent()}, refreshKey: ${this.refreshKey}`);
  }

  build() {
    Stack() {
      if (this.isVertical()) {
        this.buildVerticalTabBar();
      } else {
        this.buildHorizontalTabBar();
      }
    }
    .width('100%')
    .height('100%')
    .hitTestBehavior(HitTestMode.Transparent)
  }

  @Builder
  private buildVerticalTabBar() {
    Column() {
      this.buildCenteredVerticalLayout()
    }
    .width(this.getTabBarSize())
    .height('100%')
    .backgroundColor(this.getBackgroundColor())
    .justifyContent(FlexAlign.SpaceEvenly)
    .alignItems(HorizontalAlign.Center)
    .padding({
      top: TAB_BAR_PADDING_VERTICAL_TOP,
      bottom: TAB_BAR_PADDING_VERTICAL_BOTTOM,
      left: TAB_BAR_PADDING_VERTICAL_LEFT,
      right: TAB_BAR_PADDING_VERTICAL_RIGHT
    })
      .border(this.getVerticalBorder())
      .linearGradient(this.getVerticalGradient())
      .hitTestBehavior(HitTestMode.Transparent)
      .zIndex(1)
      .position({
        top: 0,
        bottom: 0,
        left: this.tabbar.position === TabBarPosition.Left ? 0 : undefined,
        right: this.tabbar.position === TabBarPosition.Right ? 0 : undefined
      })
  }

  @Builder
  private buildHorizontalTabBar() {
    Row() {
      this.buildCenteredHorizontalLayout()
    }
    .width('100%')
    .height(this.getTabBarSize())
    .backgroundColor(this.getBackgroundColor())
    .justifyContent(FlexAlign.SpaceEvenly)
    .alignItems(VerticalAlign.Center)
    .padding(this.getHorizontalPadding())
      .border(this.getHorizontalBorder())
      .linearGradient(this.getHorizontalGradient())
      .hitTestBehavior(HitTestMode.Transparent)
      .zIndex(1)
      .position({
        bottom: this.tabbar.position === TabBarPosition.Bottom ? 0 : undefined,
        left: 0,
        right: 0
      })
  }

  @Builder
  buildTabItem(item: TabBarItem, index: number) {
    Column() {
      // Tab icon with badge/red dot overlay
      Stack() {
        // Tab icon - support full path icons from native
        if (item.iconPath || item.selectedIconPath) {
          Image(this.getIconUrl(item, index))
            .width(TAB_BAR_ITEM_ICON_SIZE)
            .height(TAB_BAR_ITEM_ICON_SIZE)
            .fillColor(this.tabbar.selectedIndex === index ? this.getSelectedColor() : this.getNormalColor())
            .objectFit(ImageFit.Contain)
            .onError((error: ImageError): void => {
              const currentIconPath: string | undefined = this.tabbar.selectedIndex === index ?
                (item.selectedIconPath || item.iconPath) :
                item.iconPath;
              hilog.error(DOMAIN, TAG, `Tab ${index} icon load error: ${JSON.stringify(error)}, path: ${currentIconPath}`);
            })
        }

        // Badge or red dot
        if (item.badge) {
          Text(item.badge)
            .fontSize(TAB_BAR_ITEM_BADGE_FONT_SIZE)
            .fontColor(COLOR_WHITE)
            .backgroundColor(COLOR_BADGE_RED)
            .borderRadius(TAB_BAR_ITEM_BADGE_BORDER_RADIUS)
            .padding({
              left: TAB_BAR_ITEM_BADGE_PADDING,
              right: TAB_BAR_ITEM_BADGE_PADDING,
              top: TAB_BAR_ITEM_BADGE_VERTICAL_PADDING,
              bottom: TAB_BAR_ITEM_BADGE_VERTICAL_PADDING
            })
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.None })
            .textAlign(TextAlign.Center)
            .position({ x: TAB_BAR_ITEM_BADGE_POSITION_X, y: TAB_BAR_ITEM_BADGE_POSITION_Y })
            .zIndex(1)
            .height(TAB_BAR_ITEM_BADGE_HEIGHT)
        } else if (item.hasRedDot) {
          // Red dot indicator
          Circle()
            .width(TAB_BAR_ITEM_RED_DOT_SIZE)
            .height(TAB_BAR_ITEM_RED_DOT_SIZE)
            .fill(COLOR_RED_DOT)
            .position({ x: TAB_BAR_ITEM_RED_DOT_POSITION_X, y: TAB_BAR_ITEM_RED_DOT_POSITION_Y })
            .zIndex(1)
        }
      }
      .width(TAB_BAR_ITEM_ICON_CONTAINER_SIZE)
      .height(TAB_BAR_ITEM_ICON_SIZE)

      // Tab text - optional, only show if provided
      if (item.text) {
        Text(item.text)
          .fontSize(this.isVertical() ? TAB_BAR_ITEM_TEXT_SIZE_VERTICAL : TAB_BAR_ITEM_TEXT_SIZE_HORIZONTAL)
          .fontColor(this.tabbar.selectedIndex === index ? this.getSelectedColor() : this.getNormalColor())
          .margin({ top: TAB_BAR_ITEM_ICON_TEXT_SPACING })
          .fontWeight(this.isTransparent() ? FontWeight.Medium : FontWeight.Normal)
          .maxLines(this.isVertical() ? TAB_BAR_ITEM_TEXT_MAX_LINES_VERTICAL : TAB_BAR_ITEM_TEXT_MAX_LINES_HORIZONTAL)
          .textAlign(TextAlign.Center)
          // Add text shadow for better readability on transparent background
          .textShadow(this.isTransparent() ? {
            radius: TEXT_SHADOW_RADIUS,
            color: COLOR_TEXT_SHADOW,
            offsetX: TEXT_SHADOW_OFFSET_X,
            offsetY: TEXT_SHADOW_OFFSET_Y
          } : undefined)
      }
    }
    // Each item gets equal weight to distribute space
    .layoutWeight(1)
    .width(this.isVertical() ? '100%' : undefined)
    .height(this.isVertical() ? undefined : '100%')
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .margin({ top: TAB_BAR_ITEM_PADDING, bottom: TAB_BAR_ITEM_PADDING })
    .onClick(() => {
      hilog.info(DOMAIN, TAG, `Tab clicked: ${index}, current: ${this.tabbar.selectedIndex}, appId: ${this.appId}, path: ${item.pagePath}`);

      // Always trigger callback - let parent decide if action is needed
      if (this.onTabSelected) {
        this.onTabSelected(this.appId, index, item);
      }
    })
  }

  /**
   * Build centered vertical layout (original behavior)
   */
  @Builder
  buildCenteredVerticalLayout() {
    ForEach(this.tabbar.items, (item: TabBarItem, index: number) => {
      this.buildTabItem(item, index)
    })
  }

  /**
   * Build centered horizontal layout (original behavior)
   */
  @Builder
  buildCenteredHorizontalLayout() {
    ForEach(this.tabbar.items, (item: TabBarItem, index: number) => {
      this.buildTabItem(item, index)
    })
  }

  /**
   * Get icon URL with proper file:// prefix for absolute paths
   */
  private getIconUrl(item: TabBarItem, index: number): string {
    const iconPath: string | undefined = this.tabbar.selectedIndex === index ?
      (item.selectedIconPath || item.iconPath) :
      item.iconPath;

    if (!iconPath) {
      return '';
    }

    // Add file:// prefix for absolute paths
    const fileUrl: string = iconPath.startsWith('/') ? `file://${iconPath}` : iconPath;
    return fileUrl;
  }

  /**
   * Get normal text/icon color
   */
  private getNormalColor(): ResourceColor {
    return this.toResourceColor(this.tabbar.color ?? 0xFF666666);
  }

  /**
   * Get selected text/icon color
   */
  private getSelectedColor(): ResourceColor {
    return this.toResourceColor(this.tabbar.selectedColor ?? 0xFF1677FF);
  }

  /**
   * Get background color - supports transparent for better integration
   */
  private getBackgroundColor(): ResourceColor {
    const bgColor: number = this.tabbar.backgroundColor ?? 0x00000000;
    return this.toResourceColor(bgColor);
  }

  /**
   * Get border color
   */
  private getBorderColor(): ResourceColor {
    return this.toResourceColor(this.tabbar.borderStyle ?? 0xFFF0F0F0);
  }

  /**
   * Get TabBar size (height for horizontal, width for vertical)
   */
  private getTabBarSize(): number {
    // Use configured dimension (Rust provides default value, but TypeScript interface might be undefined)
    return this.tabbar.dimension ?? (this.isVertical() ? TAB_BAR_VERTICAL_WIDTH : TAB_BAR_HORIZONTAL_HEIGHT);
  }

  /**
   * Check if TabBar is vertical layout (left/right position)
   */
  private isVertical(): boolean {
    return this.tabbar.position === TabBarPosition.Left || this.tabbar.position === TabBarPosition.Right;
  }

  /**
   * Check if TabBar is transparent using bit operations
   */
  private isTransparent(): boolean {
    const bgColor: number = this.tabbar.backgroundColor ?? 0x00000000;
    return ((bgColor >>> 24) & 0xFF) === 0;
  }

  /**
   * Get padding for horizontal layout
   */
  private getHorizontalPadding(): Padding {
    if (this.isTransparent()) {
      // Overlay mode: keep the bottom safe area at zero to avoid stretching the tappable area and showing a white block
      return {
        top: TAB_BAR_PADDING_HORIZONTAL_TOP,
        bottom: 0,
        left: TAB_BAR_PADDING_HORIZONTAL_LEFT,
        right: TAB_BAR_PADDING_HORIZONTAL_RIGHT
      };
    } else {
      return {
        top: TAB_BAR_PADDING_HORIZONTAL_TOP,
        bottom: TAB_BAR_PADDING_HORIZONTAL_BOTTOM,
        left: TAB_BAR_PADDING_HORIZONTAL_LEFT,
        right: TAB_BAR_PADDING_HORIZONTAL_RIGHT
      };
    }
  }

  /**
   * Convert ARGB color value to ResourceColor
   */
  private toResourceColor(colorValue: number): ResourceColor {
    if (((colorValue >>> 24) & 0xFF) === 0) {
      return COLOR_TRANSPARENT;
    }

    const alpha = (colorValue >>> 24) & 0xFF;
    const red = (colorValue >>> 16) & 0xFF;
    const green = (colorValue >>> 8) & 0xFF;
    const blue = colorValue & 0xFF;

    if (alpha === 255) {
      return `#${red.toString(16).padStart(2, '0')}${green.toString(16).padStart(2, '0')}${blue.toString(16).padStart(2, '0')}`;
    } else {
      return `rgba(${red}, ${green}, ${blue}, ${alpha / 255})`;
    }
  }

  /**
   * Get border for horizontal layout
   */
  private getHorizontalBorder(): BorderOptions {
    return {
      width: { top: this.isTransparent() ? 0 : TAB_BAR_BORDER_WIDTH },
      color: this.getBorderColor()
    };
  }

  /**
   * Get border for vertical layout
   */
  private getVerticalBorder(): BorderOptions {
    const position: number = this.tabbar.position;
    if (position === TabBarPosition.Left) {
      return {
        width: { right: this.isTransparent() ? 0 : TAB_BAR_BORDER_WIDTH },
        color: this.getBorderColor()
      };
    } else if (position === TabBarPosition.Right) {
      return {
        width: { left: this.isTransparent() ? 0 : TAB_BAR_BORDER_WIDTH },
        color: this.getBorderColor()
      };
    }
    return {};
  }

  /**
   * Get gradient for horizontal layout
   */
  private getHorizontalGradient(): LinearGradient | undefined {
    if (!this.isTransparent()) return undefined;

    return undefined;
  }

  /**
   * Get gradient for vertical layout
   */
  private getVerticalGradient(): LinearGradient | undefined {
    if (!this.isTransparent()) return undefined;

    return undefined;
  }
}
