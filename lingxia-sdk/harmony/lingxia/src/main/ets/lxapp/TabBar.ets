import { hilog } from '@kit.PerformanceAnalysisKit';

// TabBar position constants
export class TabBarPosition {
  static readonly BOTTOM: number = 0;
  static readonly LEFT: number = 1;
  static readonly RIGHT: number = 2;
}

/**
 * TabBar configuration interfaces
 */
export interface TabBarItem {
  pagePath: string;
  text?: string;            // Text is optional - supports icon-only tabs
  iconPath?: string;
  selectedIconPath?: string;
  selected?: boolean;       // Whether this tab is initially selected
  badge?: string;           // Badge text (e.g., "99+", "NEW")
  redDot?: boolean;         // Show red dot indicator
  badgeStyle?: BadgeStyle;  // Badge styling options
  group?: number;           // Group positioning: 0=center (default), 1=start, 2=end
}

export interface BadgeStyle {
  backgroundColor?: string;  // Badge background color
  color?: string;           // Badge text color
  fontSize?: number;        // Badge font size
  borderRadius?: number;    // Badge border radius
}

export interface TabBarConfig {
  color?: number;
  selectedColor?: number;
  backgroundColor?: number;
  borderStyle?: number;     // Border color as ARGB value
  position?: number;        // 0=bottom, 1=left, 2=right
  dimension?: number;       // Dimension in dp: height for bottom, width for left/right
  list: TabBarItem[];
}

const DOMAIN = 0x0000;
const TAG = 'LingXia.TabBar';

/**
 * TabBar component for LxApp
 * Displays bottom tab bar with navigation items
 */
@Component
export struct TabBar {
  @Prop config: TabBarConfig;
  @Prop appId: string = '';
  @Prop selectedIndex: number = 0;  // Use @Prop instead of @State to sync with parent

  // Callback functions
  onTabSelected?: (appId: string, index: number, item: TabBarItem) => void;

  aboutToAppear() {
    hilog.info(DOMAIN, TAG, `TabBar aboutToAppear: ${this.config.list.length} items`);
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
        left: this.config.position === TabBarPosition.LEFT ? 0 : undefined,
        right: this.config.position === TabBarPosition.RIGHT ? 0 : undefined
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
        bottom: this.config.position === TabBarPosition.BOTTOM ? 0 : undefined,
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
            .onError((error) => {
              const currentIconPath = this.selectedIndex === index ?
                (item.selectedIconPath || item.iconPath) :
                item.iconPath;
              hilog.error(DOMAIN, TAG, `Tab ${index} icon load error: ${JSON.stringify(error)}, path: ${currentIconPath}`);
            })
            .onComplete(() => {
              const currentIconPath = this.selectedIndex === index ?
                (item.selectedIconPath || item.iconPath) :
                item.iconPath;
            })
        }

        // Badge or red dot
        if (item.badge) {
          // Badge with text
          Text(item.badge)
            .fontSize(item.badgeStyle?.fontSize || 10)
            .fontColor(item.badgeStyle?.color || '#ffffff')
            .backgroundColor(item.badgeStyle?.backgroundColor || '#ff0000')
            .borderRadius(item.badgeStyle?.borderRadius || 8)
            .padding({ left: 4, right: 4, top: 1, bottom: 1 })
            .position({ x: 16, y: -4 })
            .zIndex(1)
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
      .width(24)
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
    ForEach(this.config.list, (item: TabBarItem, index: number) => {
      this.buildTabItem(item, index)
    })
  }

  /**
   * Build centered horizontal layout (original behavior)
   */
  @Builder
  buildCenteredHorizontalLayout() {
    ForEach(this.config.list, (item: TabBarItem, index: number) => {
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
      this.buildTabItem(item, this.config.list.indexOf(item))
      if (index < this.getStartItems().length - 1) {
        Blank().height(16) // Adaptive spacer
      }
    })

    // Flexible spacer to push end items to the bottom
    Blank()
      .layoutWeight(1)

    // End items
    ForEach(this.getEndItems(), (item: TabBarItem, index: number) => {
      this.buildTabItem(item, this.config.list.indexOf(item))
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
      this.buildTabItem(item, this.config.list.indexOf(item))
      if (index < this.getStartItems().length - 1) {
        Blank().width(24) // Adaptive spacer
      }
    })

    // Flexible spacer to push end items to the right
    Blank()
      .layoutWeight(1)

    // End items
    ForEach(this.getEndItems(), (item: TabBarItem, index: number) => {
      this.buildTabItem(item, this.config.list.indexOf(item))
      if (index < this.getEndItems().length - 1) {
        Blank().width(24) // Adaptive spacer
      }
    })
  }

  /**
   * Check if any item has group field (grouped mode vs centered mode)
   */
  private hasGroupField(): boolean {
    return this.config.list.some(item => (item.group ?? 0) !== 0);
  }

  /**
   * Get start items (group 0 or 1)
   */
  private getStartItems(): TabBarItem[] {
    return this.config.list.filter(item => {
      const group = item.group ?? 0;
      return group !== 2; // 0 (no group) or 1 (start) → all treated as start
    });
  }

  /**
   * Get end items (group 2)
   */
  private getEndItems(): TabBarItem[] {
    return this.config.list.filter(item => {
      const group = item.group ?? 0;
      return group === 2; // end
    });
  }

  /**
   * Get icon URL with proper file:// prefix for absolute paths
   */
  private getIconUrl(item: TabBarItem, index: number): string {
    const iconPath = this.selectedIndex === index ?
      (item.selectedIconPath || item.iconPath) :
      item.iconPath;

    if (!iconPath) {
      return '';
    }

    // Add file:// prefix for absolute paths
    const fileUrl = iconPath.startsWith('/') ? `file://${iconPath}` : iconPath;
    return fileUrl;
  }

  /**
   * Get normal text/icon color
   */
  private getNormalColor(): ResourceColor {
    return this.toResourceColor(this.config.color ?? 0xFF666666);
  }

  /**
   * Get selected text/icon color
   */
  private getSelectedColor(): ResourceColor {
    return this.toResourceColor(this.config.selectedColor ?? 0xFF1677FF);
  }

  /**
   * Get background color - supports transparent with better integration
   */
  private getBackgroundColor(): ResourceColor {
    const bgColor = this.config.backgroundColor ?? 0xFFFFFFFF;
    return this.toResourceColor(bgColor);
  }

  /**
   * Get border color
   */
  private getBorderColor(): ResourceColor {
    return this.toResourceColor(this.config.borderStyle ?? 0xFFF0F0F0);
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
    return this.config.dimension ?? (this.isVertical() ? 64 : 72);
  }

  /**
   * Check if TabBar is vertical (left/right position)
   */
  private isVertical(): boolean {
    return this.config.position === TabBarPosition.LEFT || this.config.position === TabBarPosition.RIGHT;
  }

  /**
   * Check if TabBar is transparent using bit operations
   */
  private isTransparent(): boolean {
    const bgColor = this.config.backgroundColor ?? 0xFFFFFFFF;
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
    const position = this.config.position;
    if (position === TabBarPosition.LEFT) {
      return {
        width: { right: this.isTransparent() ? 0 : this.getBorderWidth() },
        color: this.getBorderColor()
      };
    } else if (position === TabBarPosition.RIGHT) {
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
  private currentConfig: TabBarConfig | null = null;
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
   * Update tab bar configuration
   */
  public updateConfig(appId: string, config: TabBarConfig): void {
    hilog.info(DOMAIN, TAG, `Updating TabBar config for appId: ${appId}`);
    this.appId = appId;
    this.currentConfig = config;
  }

  /**
   * Set tab bar badge
   * @param index Tab index
   * @param text Badge text (e.g., "99+", "NEW")
   */
  public setTabBarBadge(index: number, text: string): void {
    if (this.currentConfig && index >= 0 && index < this.currentConfig.list.length) {
      this.currentConfig.list[index].badge = text;
      this.currentConfig.list[index].redDot = false; // Clear red dot when setting badge
      hilog.info(DOMAIN, TAG, `Set tab ${index} badge: ${text}`);
    }
  }

  /**
   * Remove tab bar badge
   * @param index Tab index
   */
  public removeTabBarBadge(index: number): void {
    if (this.currentConfig && index >= 0 && index < this.currentConfig.list.length) {
      this.currentConfig.list[index].badge = undefined;
      hilog.info(DOMAIN, TAG, `Removed tab ${index} badge`);
    }
  }

  /**
   * Show tab bar red dot
   * @param index Tab index
   */
  public showTabBarRedDot(index: number): void {
    if (this.currentConfig && index >= 0 && index < this.currentConfig.list.length) {
      this.currentConfig.list[index].redDot = true;
      this.currentConfig.list[index].badge = undefined; // Clear badge when showing red dot
      hilog.info(DOMAIN, TAG, `Show tab ${index} red dot`);
    }
  }

  /**
   * Hide tab bar red dot
   * @param index Tab index
   */
  public hideTabBarRedDot(index: number): void {
    if (this.currentConfig && index >= 0 && index < this.currentConfig.list.length) {
      this.currentConfig.list[index].redDot = false;
      hilog.info(DOMAIN, TAG, `Hide tab ${index} red dot`);
    }
  }

  /**
   * Set tab bar style
   * @param style Style configuration
   */
  public setTabBarStyle(style: Partial<TabBarConfig>): void {
    if (!this.currentConfig) return;

    if (style.color) this.currentConfig.color = style.color;
    if (style.selectedColor) this.currentConfig.selectedColor = style.selectedColor;
    if (style.backgroundColor) this.currentConfig.backgroundColor = style.backgroundColor;
    if (style.borderStyle) this.currentConfig.borderStyle = style.borderStyle;

    hilog.info(DOMAIN, TAG, 'Updated tab bar style');
  }

  /**
   * Switch to tab by page path
   */
  public switchToTab(pagePath: string): boolean {
    if (!this.currentConfig) {
      return false;
    }

    const index = this.currentConfig.list.findIndex(item => item.pagePath === pagePath);
    if (index >= 0) {
      this.selectedIndex = index;
      hilog.info(DOMAIN, TAG, `Switched to tab ${index} with path: ${pagePath}`);
      return true;
    }
    return false;
  }

  /**
   * Show tab bar
   */
  public showTabBar(): void {
    hilog.info(DOMAIN, TAG, 'Show TabBar');
    // TODO: Implement show logic
  }

  /**
   * Hide tab bar
   */
  public hideTabBar(): void {
    hilog.info(DOMAIN, TAG, 'Hide TabBar');
    // TODO: Implement hide logic
  }
}
