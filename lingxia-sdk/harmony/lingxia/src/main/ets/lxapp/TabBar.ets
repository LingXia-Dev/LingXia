import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxLog } from '../app/LxLog';
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

// Circle drawn behind the icon of a selected item that ships only one icon,
// standing in for the selected artwork it does not have.
const TAB_BAR_ACTIVE_INDICATOR_SIZE = 36;
const TAB_BAR_ACTIVE_INDICATOR_OPACITY = 0.2;

// Overflow ("more") slot and its popup
const TAB_BAR_MORE_DOT_SIZE = 4;
const TAB_BAR_MORE_DOT_SPACING = 3;
const TAB_BAR_OVERFLOW_COLUMNS = 5;
const TAB_BAR_OVERFLOW_CELL_WIDTH = 64;
const TAB_BAR_OVERFLOW_CELL_PADDING = 10;
const TAB_BAR_OVERFLOW_PANEL_PADDING = 8;
const TAB_BAR_OVERFLOW_RADIUS = 16;

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
// How far the item content may dip into the home-indicator gesture zone (vp).
// The zone's tap-eating region starts below its top edge, so a small overlap
// is safe and keeps the bar visually snug to the screen bottom.
const TAB_BAR_GESTURE_ZONE_OVERLAP = 6;

/**
 * Bottom padding of a horizontal TabBar. The home-indicator inset keeps
 * icons/labels above the gesture zone (which eats taps) and REPLACES the
 * regular bottom padding rather than adding to it, so the bar grows only by
 * what the gesture zone actually requires.
 */
export function tabBarBottomPadding(bottomInset: number, transparent: boolean): number {
  // The gesture zone only eats taps in its lower part; overlapping it a
  // little keeps the items visually anchored to the bottom.
  const inset = Math.max(0, bottomInset - TAB_BAR_GESTURE_ZONE_OVERLAP);
  if (transparent) {
    return inset;
  }
  return Math.max(TAB_BAR_PADDING_HORIZONTAL_BOTTOM, inset);
}

/** Height a horizontal TabBar grows beyond its configured dimension. */
export function tabBarExtraBottomHeight(bottomInset: number, transparent: boolean): number {
  const base = transparent ? 0 : TAB_BAR_PADDING_HORIZONTAL_BOTTOM;
  return Math.max(0, tabBarBottomPadding(bottomInset, transparent) - base);
}

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
  // Bottom system-gesture inset (vp): the home-indicator zone swallows taps,
  // so the strip grows and the items sit above it.
  @Prop bottomInset: number = 0;
  // Whether the overflow panel is open above the "more" slot.
  @State private moreVisible: boolean = false;

  // Callback functions
  onTabSelected?: (appId: string, index: number, item: TabBarItem) => void;

  aboutToAppear() {
    hilog.info(DOMAIN, TAG, `TabBar created: ${this.tabbar.items.length} items, vertical: ${this.isVertical()}, transparent: ${this.isTransparent()}, refreshKey: ${this.refreshKey}`);
  }

  build() {
    // The root hugs the bar strip instead of covering the whole screen: the parent
    // Stack's alignContent places it, touches inside the strip are consumed (even
    // when visually transparent), and the area outside has no node to intercept.
    // A full-screen root with HitTestMode.Transparent would leak item taps to the
    // WebView behind — Transparent ancestors pass touches through unconditionally.
    Stack() {
      if (this.isVertical()) {
        this.buildVerticalTabBar();
      } else {
        this.buildHorizontalTabBar();
      }
    }
    .width(this.isVertical() ? this.getTabBarSize() : '100%')
    .height(this.isVertical() ? '100%' : this.getTabBarSize() + this.extraBottomHeight())
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
      bottom: TAB_BAR_PADDING_VERTICAL_BOTTOM + this.bottomInset,
      left: TAB_BAR_PADDING_VERTICAL_LEFT,
      right: TAB_BAR_PADDING_VERTICAL_RIGHT
    })
      .border(this.getVerticalBorder())
      .linearGradient(this.getVerticalGradient())
  }

  @Builder
  private buildHorizontalTabBar() {
    Row() {
      this.buildCenteredHorizontalLayout()
    }
    .width('100%')
    .height(this.getTabBarSize() + this.extraBottomHeight())
    .backgroundColor(this.getBackgroundColor())
    .justifyContent(FlexAlign.SpaceEvenly)
    .alignItems(VerticalAlign.Center)
    .padding(this.getHorizontalPadding())
      .border(this.getHorizontalBorder())
      .linearGradient(this.getHorizontalGradient())
  }

  @Builder
  buildTabItem(item: TabBarItem, index: number) {
    Column() {
      // Tab icon with badge/red dot overlay
      Stack() {
        // The indicator belongs to the bar, not to an item's artwork:
        // whatever is selected gets it.
        if (this.tabbar.selectedIndex === index) {
          Circle()
            .width(TAB_BAR_ACTIVE_INDICATOR_SIZE)
            .height(TAB_BAR_ACTIVE_INDICATOR_SIZE)
            .fill(this.getSelectedColor())
            .opacity(TAB_BAR_ACTIVE_INDICATOR_OPACITY)
        }

        // Tab icon - support full path icons from native
        if (item.iconPath) {
          Image(this.getIconUrl(item))
            .width(TAB_BAR_ITEM_ICON_SIZE)
            .height(TAB_BAR_ITEM_ICON_SIZE)
            .fillColor(this.tabbar.selectedIndex === index ? this.getSelectedColor() : this.getNormalColor())
            .objectFit(ImageFit.Contain)
            .onError((error: ImageError): void => {
              LxLog.e(TAG, `Tab ${index} icon load error: ${JSON.stringify(error)}, path: ${item.iconPath}`);
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
   * Build centered horizontal layout. Rust caps how many items a compact strip
   * shows; past that the last slot becomes the overflow affordance.
   */
  @Builder
  buildCenteredHorizontalLayout() {
    ForEach(this.stripIndices(), (index: number) => {
      this.buildTabItem(this.tabbar.items[index], index)
    })
    if (this.overflowStart() >= 0) {
      this.buildMoreItem()
    }
  }

  /** First folded item index, or -1 when every item has its own slot. */
  private overflowStart(): number {
    const start: number = this.tabbar.overflowStartIndex ?? -1;
    return start >= 0 && start < this.tabbar.items.length ? start : -1;
  }

  /** Item indices that keep a slot of their own in the strip. */
  private stripIndices(): number[] {
    const start: number = this.overflowStart();
    const count: number = start >= 0 ? start : this.tabbar.items.length;
    const indices: number[] = [];
    for (let index = 0; index < count; index++) {
      indices.push(index);
    }
    return indices;
  }

  /** Item indices only the overflow panel can reach. */
  private overflowIndices(): number[] {
    const start: number = this.overflowStart();
    const indices: number[] = [];
    if (start < 0) {
      return indices;
    }
    for (let index = start; index < this.tabbar.items.length; index++) {
      indices.push(index);
    }
    return indices;
  }

  /** The overflow slot stands in for the folded items, selection included. */
  private isMoreSelected(): boolean {
    const start: number = this.overflowStart();
    return start >= 0 && this.tabbar.selectedIndex >= start;
  }

  /** Folded badges still have to surface, so "more" aggregates them to a dot. */
  private overflowHasNotification(): boolean {
    const indices: number[] = this.overflowIndices();
    for (const index of indices) {
      const item: TabBarItem = this.tabbar.items[index];
      if (item.hasRedDot || (item.badge !== undefined && item.badge !== '')) {
        return true;
      }
    }
    return false;
  }

  @Builder
  buildMoreItem() {
    Column() {
      Stack() {
        Row({ space: TAB_BAR_MORE_DOT_SPACING }) {
          Circle()
            .width(TAB_BAR_MORE_DOT_SIZE)
            .height(TAB_BAR_MORE_DOT_SIZE)
            .fill(this.isMoreSelected() ? this.getSelectedColor() : this.getNormalColor())
          Circle()
            .width(TAB_BAR_MORE_DOT_SIZE)
            .height(TAB_BAR_MORE_DOT_SIZE)
            .fill(this.isMoreSelected() ? this.getSelectedColor() : this.getNormalColor())
          Circle()
            .width(TAB_BAR_MORE_DOT_SIZE)
            .height(TAB_BAR_MORE_DOT_SIZE)
            .fill(this.isMoreSelected() ? this.getSelectedColor() : this.getNormalColor())
        }
        .justifyContent(FlexAlign.Center)
        .alignItems(VerticalAlign.Center)
        .width(TAB_BAR_ITEM_ICON_SIZE)
        .height(TAB_BAR_ITEM_ICON_SIZE)

        if (this.overflowHasNotification()) {
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

      Text($r('app.string.lx_tabbar_more'))
        .fontSize(TAB_BAR_ITEM_TEXT_SIZE_HORIZONTAL)
        .fontColor(this.isMoreSelected() ? this.getSelectedColor() : this.getNormalColor())
        .margin({ top: TAB_BAR_ITEM_ICON_TEXT_SPACING })
        .fontWeight(this.isTransparent() ? FontWeight.Medium : FontWeight.Normal)
        .maxLines(TAB_BAR_ITEM_TEXT_MAX_LINES_HORIZONTAL)
        .textAlign(TextAlign.Center)
        .textShadow(this.isTransparent() ? {
          radius: TEXT_SHADOW_RADIUS,
          color: COLOR_TEXT_SHADOW,
          offsetX: TEXT_SHADOW_OFFSET_X,
          offsetY: TEXT_SHADOW_OFFSET_Y
        } : undefined)
    }
    .layoutWeight(1)
    .height('100%')
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .margin({ top: TAB_BAR_ITEM_PADDING, bottom: TAB_BAR_ITEM_PADDING })
    .onClick(() => {
      this.moreVisible = !this.moreVisible;
    })
    .bindPopup(this.moreVisible, {
      builder: (): void => this.buildOverflowPanel(),
      placement: Placement.Top,
      enableArrow: true,
      popupColor: this.getOverflowPanelColor(),
      radius: TAB_BAR_OVERFLOW_RADIUS,
      onStateChange: (event: PopupStateChangeParam): void => {
        // Tapping outside closes the popup natively; mirror that into our state.
        if (!event.isVisible) {
          this.moreVisible = false;
        }
      }
    })
  }

  /**
   * The folded items as a grid. Rows keep the strip's column count so a short
   * final row stays aligned instead of spreading across the panel.
   */
  @Builder
  buildOverflowPanel() {
    Flex({ wrap: FlexWrap.Wrap, justifyContent: FlexAlign.Start }) {
      ForEach(this.overflowIndices(), (index: number) => {
        this.buildOverflowCell(this.tabbar.items[index], index)
      })
    }
    .width(TAB_BAR_OVERFLOW_CELL_WIDTH * Math.min(this.overflowIndices().length, TAB_BAR_OVERFLOW_COLUMNS) +
      TAB_BAR_OVERFLOW_PANEL_PADDING * 2)
    .padding(TAB_BAR_OVERFLOW_PANEL_PADDING)
  }

  @Builder
  buildOverflowCell(item: TabBarItem, index: number) {
    Column() {
      Stack() {
        if (this.tabbar.selectedIndex === index) {
          Circle()
            .width(TAB_BAR_ACTIVE_INDICATOR_SIZE)
            .height(TAB_BAR_ACTIVE_INDICATOR_SIZE)
            .fill(this.getSelectedColor())
            .opacity(TAB_BAR_ACTIVE_INDICATOR_OPACITY)
        }

        Image(this.getIconUrl(item))
          .width(TAB_BAR_ITEM_ICON_SIZE)
          .height(TAB_BAR_ITEM_ICON_SIZE)
          .fillColor(this.tabbar.selectedIndex === index ? this.getSelectedColor() : this.getNormalColor())
          .objectFit(ImageFit.Contain)

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
            .textAlign(TextAlign.Center)
            .position({ x: TAB_BAR_ITEM_BADGE_POSITION_X, y: TAB_BAR_ITEM_BADGE_POSITION_Y })
            .zIndex(1)
            .height(TAB_BAR_ITEM_BADGE_HEIGHT)
        } else if (item.hasRedDot) {
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

      if (item.text) {
        Text(item.text)
          .fontSize(TAB_BAR_ITEM_TEXT_SIZE_HORIZONTAL)
          .fontColor(this.tabbar.selectedIndex === index ? this.getSelectedColor() : this.getNormalColor())
          .margin({ top: TAB_BAR_ITEM_ICON_TEXT_SPACING })
          .maxLines(TAB_BAR_ITEM_TEXT_MAX_LINES_HORIZONTAL)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .textAlign(TextAlign.Center)
      }
    }
    .width(TAB_BAR_OVERFLOW_CELL_WIDTH)
    .padding({ top: TAB_BAR_OVERFLOW_CELL_PADDING, bottom: TAB_BAR_OVERFLOW_CELL_PADDING })
    .alignItems(HorizontalAlign.Center)
    .onClick(() => {
      this.moreVisible = false;
      if (this.onTabSelected) {
        this.onTabSelected(this.appId, index, item);
      }
    })
  }

  /** Popup plate colour; follows the bar so the panel reads as part of it. */
  private getOverflowPanelColor(): ResourceColor {
    const background: number = this.tabbar.backgroundColor ?? 0;
    // A transparent/immersive bar has no plate of its own to borrow.
    return (background >>> 24) === 0 ? this.toResourceColor(0xFFFFFFFF) : this.toResourceColor(background);
  }

  /**
   * Get icon URL with proper file:// prefix for absolute paths
   */
  private getIconUrl(item: TabBarItem): string {
    const iconPath: string | undefined = item.iconPath;

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
  /**
   * The inset keeps icons/labels above the home-indicator gesture zone,
   * which eats taps; the strip still covers the zone so nothing leaks
   * through to the WebView. The inset REPLACES the regular bottom padding
   * (rather than adding to it) so the bar grows only by what the gesture
   * zone actually requires.
   */
  private getBottomPadding(): number {
    return tabBarBottomPadding(this.bottomInset, this.isTransparent());
  }

  private extraBottomHeight(): number {
    return tabBarExtraBottomHeight(this.bottomInset, this.isTransparent());
  }

  private getHorizontalPadding(): Padding {
    return {
      top: TAB_BAR_PADDING_HORIZONTAL_TOP,
      bottom: this.getBottomPadding(),
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
