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

// TabBar grouping constants
const TAB_BAR_GROUP_DEFAULT = 0;  // Default group (no grouping)
const TAB_BAR_GROUP_START = 1;    // Start group
const TAB_BAR_GROUP_END = 2;      // End group

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
const TAB_BAR_ITEM_BADGE_POSITION_X = 16;
const TAB_BAR_ITEM_BADGE_POSITION_Y = -8;

// Red dot configuration
const TAB_BAR_ITEM_RED_DOT_SIZE = 8;
const TAB_BAR_ITEM_RED_DOT_POSITION_X = 16;
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
const COLOR_RED_DOT = '#ff0000';
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
  @Prop selectedIndex: number = 0;  // Use @Prop instead of @State to sync with parent
  @Prop refreshKey: number = 0;  // Force refresh when data changes

  // Callback functions
  onTabSelected?: (appId: string, index: number, item: TabBarItem) => void;

  aboutToAppear() {
    hilog.info(DOMAIN, TAG, `TabBar created: ${this.tabbar.items.length} items, vertical: ${this.isVertical()}, transparent: ${this.isTransparent()}, refreshKey: ${this.refreshKey}`);
  }

  build() {
    if (this.isVertical()) {
      // Vertical layout (left/right position)
      Column() {
        if (this.hasGroupField()) {
          this.buildGroupedVerticalLayout()
        } else {
          this.buildCenteredVerticalLayout()
        }
      }
      .width(this.getTabBarSize())
      .height('100%')
      .backgroundColor(this.getBackgroundColor())
      .justifyContent(this.hasGroupField() ? FlexAlign.Start : FlexAlign.SpaceEvenly)
      .alignItems(HorizontalAlign.Center)
      .padding({
        top: TAB_BAR_PADDING_VERTICAL_TOP,
        bottom: TAB_BAR_PADDING_VERTICAL_BOTTOM,
        left: TAB_BAR_PADDING_VERTICAL_LEFT,
        right: TAB_BAR_PADDING_VERTICAL_RIGHT
      })
      .border(this.getVerticalBorder())
      .linearGradient(this.getVerticalGradient())
      .zIndex(this.isTransparent() ? 1 : 0)
      .position(this.isTransparent() ? {
        top: 0,
        bottom: 0,
        left: this.tabbar.position === TabBarPosition.Left ? 0 : undefined,
        right: this.tabbar.position === TabBarPosition.Right ? 0 : undefined
      } : {})
    } else {
      // Horizontal layout (bottom position)
      Row() {
        if (this.hasGroupField()) {
          this.buildGroupedHorizontalLayout()
        } else {
          this.buildCenteredHorizontalLayout()
        }
      }
      .width('100%')
      .height(this.getTabBarSize())
      .backgroundColor(this.getBackgroundColor())
      .justifyContent(this.hasGroupField() ? FlexAlign.Start : FlexAlign.SpaceEvenly)
      .alignItems(VerticalAlign.Center)
      .padding(this.getHorizontalPadding())
      .border(this.getHorizontalBorder())
      .linearGradient(this.getHorizontalGradient())
      .position(this.isTransparent() ? {
        bottom: this.tabbar.position === TabBarPosition.Bottom ? 0 : undefined,
        left: 0,
        right: 0
      } : {})
    }
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
            .fillColor(this.selectedIndex === index ? this.getSelectedColor() : this.getNormalColor())
            .objectFit(ImageFit.Contain)
            .onError((error: ImageError): void => {
              const currentIconPath: string | undefined = this.selectedIndex === index ?
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
        } else if (item.redDot) {
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
          .fontColor(this.selectedIndex === index ? this.getSelectedColor() : this.getNormalColor())
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
    // In grouped mode, layoutWeight is handled by Blank spacer
    // In centered mode, each item gets equal weight to distribute space
    .layoutWeight(this.hasGroupField() ? 0 : 1)
    .width(this.isVertical() ? '100%' : undefined)
    .height(this.isVertical() ? undefined : '100%')
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .margin({ top: TAB_BAR_ITEM_PADDING, bottom: TAB_BAR_ITEM_PADDING })
    .onClick(() => {
      hilog.info(DOMAIN, TAG, `Tab clicked: ${index}, current: ${this.selectedIndex}, appId: ${this.appId}, path: ${item.pagePath}`);

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
   * Build grouped vertical layout: distribute items to start/end positions
   */
  @Builder
  buildGroupedVerticalLayout() {
    // Start items
    ForEach(this.getStartItems(), (item: TabBarItem, index: number) => {
      this.buildTabItem(item, this.tabbar.items.indexOf(item))
      if (index < this.getStartItems().length - 1) {
        Blank().height(TAB_BAR_ITEM_SPACING) // Adaptive spacer
      }
    })

    // Flexible spacer to push end items to the bottom
    Blank()
      .layoutWeight(1)

    // End items
    ForEach(this.getEndItems(), (item: TabBarItem, index: number) => {
      this.buildTabItem(item, this.tabbar.items.indexOf(item))
      if (index < this.getEndItems().length - 1) {
        Blank().height(TAB_BAR_ITEM_SPACING) // Adaptive spacer
      }
    })
  }

  /**
   * Build grouped horizontal layout: distribute items to start/end positions
   */
  @Builder
  buildGroupedHorizontalLayout() {
    // Start items
    ForEach(this.getStartItems(), (item: TabBarItem, index: number) => {
      this.buildTabItem(item, this.tabbar.items.indexOf(item))
      if (index < this.getStartItems().length - 1) {
        Blank().width(TAB_BAR_ITEM_SPACING) // Adaptive spacer
      }
    })

    // Flexible spacer to push end items to the right
    Blank()
      .layoutWeight(1)

    // End items
    ForEach(this.getEndItems(), (item: TabBarItem, index: number) => {
      this.buildTabItem(item, this.tabbar.items.indexOf(item))
      if (index < this.getEndItems().length - 1) {
        Blank().width(TAB_BAR_ITEM_SPACING) // Adaptive spacer
      }
    })
  }

  /**
   * Check if any item has group field (grouped mode vs centered mode)
   */
  private hasGroupField(): boolean {
    return this.tabbar.items.some((item: TabBarItem): boolean => (item.group ?? 0) !== 0);
  }

  /**
   * Get start items (group 0 or 1)
   */
  private getStartItems(): TabBarItem[] {
    return this.tabbar.items.filter((item: TabBarItem): boolean => {
      const group: number = item.group ?? 0;
      return group !== TAB_BAR_GROUP_END; // 0 (no group) or 1 (start) → all treated as start
    });
  }

  /**
   * Get end items (group 2)
   */
  private getEndItems(): TabBarItem[] {
    return this.tabbar.items.filter((item: TabBarItem): boolean => {
      const group: number = item.group ?? 0;
      return group === TAB_BAR_GROUP_END; // end
    });
  }

  /**
   * Get icon URL with proper file:// prefix for absolute paths
   */
  private getIconUrl(item: TabBarItem, index: number): string {
    const iconPath: string | undefined = this.selectedIndex === index ?
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
    const bgColor: number = this.tabbar.backgroundColor ?? 0xFFFFFFFF;
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
    const bgColor: number = this.tabbar.backgroundColor ?? 0xFFFFFFFF;
    return ((bgColor >>> 24) & 0xFF) === 0;
  }

  /**
   * Get padding for horizontal layout
   */
  private getHorizontalPadding(): Padding {
    const safeAreaBottom = this.isTransparent() ? SYSTEM_SAFE_AREA_BOTTOM : 0;
    return {
      top: TAB_BAR_PADDING_HORIZONTAL_TOP,
      bottom: TAB_BAR_PADDING_HORIZONTAL_BOTTOM + safeAreaBottom,
      left: TAB_BAR_PADDING_HORIZONTAL_LEFT,
      right: TAB_BAR_PADDING_HORIZONTAL_RIGHT
    };
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

/**
 * TabBar controller for programmatic control
 */
export class TabBarController {
  private static instance: TabBarController | null = null;
  private currenttabbar: TabBarState | null = null;
  private appId: string = '';
  private selectedIndex: number = 0;

  private constructor() {
    hilog.info(DOMAIN, TAG, 'TabBarController instance created');
  }

  /**
   * Get TabBarController singleton instance
   */
  public static getInstance(): TabBarController {
    if (!TabBarController.instance) {
      TabBarController.instance = new TabBarController();
    }
    return TabBarController.instance;
  }

  /**
   * Update tab bar tabbaruration
   */
  public updatetabbar(appId: string, tabbar: TabBarState): void {
    hilog.info(DOMAIN, TAG, `Updating TabBar tabbar for appId: ${appId}`);
    this.appId = appId;
    this.currenttabbar = tabbar;
  }

  /**
   * Switch to tab by page path
   */
  public switchToTab(pagePath: string): boolean {
    if (!this.currenttabbar) {
      return false;
    }

    const index: number = this.currenttabbar.items.findIndex((item: TabBarItem): boolean => item.pagePath === pagePath);
    if (index >= 0) {
      this.selectedIndex = index;
      hilog.info(DOMAIN, TAG, `Switched to tab ${index} with path: ${pagePath}`);
      return true;
    }
    return false;
  }
}
