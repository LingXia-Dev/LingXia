import { hilog } from '@kit.PerformanceAnalysisKit';
import { TabBarPosition, type TabBarItem, type TabBarState } from 'liblingxia.so';

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
    hilog.info(DOMAIN, TAG, `TabBar created: ${this.tabbar.items.length} items, refreshKey: ${this.refreshKey}`);
  }

  build() {
    if (this.isVertical()) {
      // Vertical layout for left/right position
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
      .padding(this.getVerticalPadding())
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
      // Horizontal layout for bottom position
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
            .width(24)
            .height(24)
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
            .fontSize(10)
            .fontColor('#ffffff')
            .backgroundColor('#FA5151')
            .borderRadius(8)
            .padding({ left: 6, right: 6, top: 2, bottom: 2 })
            .maxLines(1)
            .textOverflow({ overflow: TextOverflow.None })
            .textAlign(TextAlign.Center)
            .position({ x: 16, y: -8 })
            .zIndex(1)
            .height(16)
        } else if (item.redDot) {
          // Red dot indicator
          Circle()
            .width(8)
            .height(8)
            .fill('#ff0000')
            .position({ x: 16, y: -2 })
            .zIndex(1)
        }
      }
      .width(48)
      .height(24)

      // Tab text - optional, only show if provided
      if (item.text) {
        Text(item.text)
          .fontSize(this.isVertical() ? 10 : 12)
          .fontColor(this.selectedIndex === index ? this.getSelectedColor() : this.getNormalColor())
          .margin({ top: 4 })
          .fontWeight(this.isTransparent() ? FontWeight.Medium : FontWeight.Normal)
          .maxLines(this.isVertical() ? 2 : 1)
          .textAlign(TextAlign.Center)
          // Add text shadow for better readability on transparent background
          .textShadow(this.isTransparent() ? {
            radius: 1,
            color: 'rgba(255,255,255,0.8)',
            offsetX: 0,
            offsetY: 1
          } : undefined)
      }
    }
    // In grouped mode, layoutWeight is handled by the Blank spacer
    // In centered mode, each item gets equal weight to distribute space
    .layoutWeight(this.hasGroupField() ? 0 : 1)
    .width(this.isVertical() ? '100%' : undefined)
    .height(this.isVertical() ? undefined : '100%')
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .margin({ top: 4, bottom: 4 })
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
        Blank().height(16) // Adaptive spacer
      }
    })

    // Flexible spacer to push end items to the bottom
    Blank()
      .layoutWeight(1)

    // End items
    ForEach(this.getEndItems(), (item: TabBarItem, index: number) => {
      this.buildTabItem(item, this.tabbar.items.indexOf(item))
      if (index < this.getEndItems().length - 1) {
        Blank().height(16) // Adaptive spacer
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
        Blank().width(24) // Adaptive spacer
      }
    })

    // Flexible spacer to push end items to the right
    Blank()
      .layoutWeight(1)

    // End items
    ForEach(this.getEndItems(), (item: TabBarItem, index: number) => {
      this.buildTabItem(item, this.tabbar.items.indexOf(item))
      if (index < this.getEndItems().length - 1) {
        Blank().width(24) // Adaptive spacer
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
      return group !== 2; // 0 (no group) or 1 (start) → all treated as start
    });
  }

  /**
   * Get end items (group 2)
   */
  private getEndItems(): TabBarItem[] {
    return this.tabbar.items.filter((item: TabBarItem): boolean => {
      const group: number = item.group ?? 0;
      return group === 2; // end
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
   * Get background color - supports transparent with better integration
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
   * Get border width
   */
  private getBorderWidth(): number {
    return 0.5;
  }

  /**
   * Get tab bar size (height for horizontal, width for vertical)
   */
  private getTabBarSize(): number {
    // Use configured dimension (Rust provides default value, but TypeScript interface might be undefined)
    return this.tabbar.dimension ?? (this.isVertical() ? 64 : 72);
  }

  /**
   * Check if TabBar is vertical (left/right position)
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
   * Convert ARGB color value to ResourceColor
   */
  private toResourceColor(colorValue: number): ResourceColor {
    if (((colorValue >>> 24) & 0xFF) === 0) {
      return Color.Transparent;
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
   * Get padding for horizontal layout
   */
  private getHorizontalPadding(): Padding {
    // Add extra bottom padding for safe area when transparent
    const safeAreaBottom = this.isTransparent() ? 24 : 0; // Increased safe area
    const horizontalPadding = 16; // Add horizontal padding
    return {
      top: 8,
      bottom: 12 + safeAreaBottom,
      left: horizontalPadding,
      right: horizontalPadding
    };
  }

  /**
   * Get padding for vertical layout
   */
  private getVerticalPadding(): Padding {
    // Add minimal top padding for system status bar when TabBar is on left/right
    const statusBarHeight = 12; // Minimal status bar height in vp
    return {
      left: 8,
      right: 8,
      top: 8 + statusBarHeight, // Reduced padding + minimal status bar space
      bottom: 8
    };
  }

  /**
   * Get border for horizontal layout
   */
  private getHorizontalBorder(): BorderOptions {
    return {
      width: { top: this.isTransparent() ? 0 : this.getBorderWidth() },
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
        width: { right: this.isTransparent() ? 0 : this.getBorderWidth() },
        color: this.getBorderColor()
      };
    } else if (position === TabBarPosition.Right) {
      return {
        width: { left: this.isTransparent() ? 0 : this.getBorderWidth() },
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
