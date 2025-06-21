import { hilog } from '@kit.PerformanceAnalysisKit';

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
}

export interface BadgeStyle {
  backgroundColor?: string;  // Badge background color
  color?: string;           // Badge text color
  fontSize?: number;        // Badge font size
  borderRadius?: number;    // Badge border radius
}

export interface TabBarConfig {
  color?: string;
  selectedColor?: string;
  backgroundColor?: string;
  borderStyle?: string;     // Border style: "black" | "white"
  position?: string;
  list: TabBarItem[];
}

const DOMAIN = 0x0000;
const TAG = 'LingXia.TabBar';

/**
 * TabBar component for MiniApp
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
    this.config.list.forEach((item, index) => {
      hilog.info(DOMAIN, TAG, `Tab ${index}: path=${item.pagePath}, text=${item.text || 'none'}`);
    });
  }

  build() {
    if (this.isVertical()) {
      // Vertical layout for left/right position
      Column() {
        ForEach(this.config.list, (item: TabBarItem, index: number) => {
          this.buildTabItem(item, index)
        })
      }
      .width(this.getTabBarSize())
      .height('100%')
      .backgroundColor(this.getBackgroundColor())
      .justifyContent(FlexAlign.SpaceEvenly)
      .alignItems(HorizontalAlign.Center)
      .padding(this.getVerticalPadding())
      .border(this.getVerticalBorder())
      .linearGradient(this.getVerticalGradient())
    } else {
      // Horizontal layout for bottom/top position
      Row() {
        ForEach(this.config.list, (item: TabBarItem, index: number) => {
          this.buildTabItem(item, index)
        })
      }
      .width('100%')
      .height(this.getTabBarSize())
      .backgroundColor(this.getBackgroundColor())
      .justifyContent(FlexAlign.SpaceEvenly)
      .alignItems(VerticalAlign.Center)
      .padding(this.getHorizontalPadding())
      .border(this.getHorizontalBorder())
      .linearGradient(this.getHorizontalGradient())
    }
  }

  @Builder
  buildTabItem(item: TabBarItem, index: number) {
    Column() {
      // Tab icon with badge/red dot overlay
      Stack() {
        // Tab icon - support full path icons from native
        if (item.iconPath || item.selectedIconPath) {
          Image(this.selectedIndex === index ?
            (item.selectedIconPath || item.iconPath) :
            item.iconPath)
            .width(24)
            .height(24)
            .fillColor(this.selectedIndex === index ? this.getSelectedColor() : this.getNormalColor())
            .objectFit(ImageFit.Contain)
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
    .layoutWeight(1)
    .width(this.isVertical() ? '100%' : undefined)
    .height(this.isVertical() ? undefined : '100%')
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .onClick(() => {
      hilog.info(DOMAIN, TAG, `Tab clicked: ${index}, current: ${this.selectedIndex}, appId: ${this.appId}, path: ${item.pagePath}`);

      // Always trigger callback - let parent decide if action is needed
      if (this.onTabSelected) {
        this.onTabSelected(this.appId, index, item);
      }
    })
  }

  /**
   * Get normal text/icon color
   */
  private getNormalColor(): ResourceColor {
    return this.config.color || '#666666';
  }

  /**
   * Get selected text/icon color
   */
  private getSelectedColor(): ResourceColor {
    return this.config.selectedColor || '#1677FF';
  }

  /**
   * Get background color - supports transparent with better integration
   */
  private getBackgroundColor(): ResourceColor {
    const bgColor = this.config.backgroundColor || '#ffffff';
    if (bgColor === 'transparent') {
      // Use completely transparent base, rely on gradient for visual effect
      return Color.Transparent;
    }
    return bgColor;
  }

  /**
   * Get border color
   */
  private getBorderColor(): ResourceColor {
    if (this.config.borderStyle === 'white') {
      return '#ffffff';
    }
    return '#e0e0e0';
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
    return this.isVertical() ? 64 : 72; // Narrower width for left/right position
  }

  /**
   * Check if TabBar is vertical (left/right position)
   */
  private isVertical(): boolean {
    return this.config.position === 'left' || this.config.position === 'right';
  }

  /**
   * Check if TabBar is transparent
   */
  private isTransparent(): boolean {
    return this.config.backgroundColor === 'transparent';
  }

  /**
   * Get padding for horizontal layout
   */
  private getHorizontalPadding(): Padding {
    return { top: 8, bottom: 12 };
  }

  /**
   * Get padding for vertical layout
   */
  private getVerticalPadding(): Padding {
    return { left: 8, right: 8, top: 12, bottom: 12 };
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
    if (position === 'left') {
      return {
        width: { right: this.isTransparent() ? 0 : this.getBorderWidth() },
        color: this.getBorderColor()
      };
    } else if (position === 'right') {
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

    return {
      angle: 180, // Top to bottom
      colors: [
        ['rgba(255,255,255,0.0)', 0.0],
        ['rgba(255,255,255,0.4)', 0.4],
        ['rgba(255,255,255,0.85)', 1.0]
      ]
    };
  }

  /**
   * Get gradient for vertical layout
   */
  private getVerticalGradient(): LinearGradient | undefined {
    if (!this.isTransparent()) return undefined;

    const position = this.config.position;
    if (position === 'left') {
      return {
        angle: 90, // Left to right
        colors: [
          ['rgba(255,255,255,0.0)', 0.0],
          ['rgba(255,255,255,0.4)', 0.4],
          ['rgba(255,255,255,0.85)', 1.0]
        ]
      };
    } else if (position === 'right') {
      return {
        angle: 270, // Right to left
        colors: [
          ['rgba(255,255,255,0.0)', 0.0],
          ['rgba(255,255,255,0.4)', 0.4],
          ['rgba(255,255,255,0.85)', 1.0]
        ]
      };
    }
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
