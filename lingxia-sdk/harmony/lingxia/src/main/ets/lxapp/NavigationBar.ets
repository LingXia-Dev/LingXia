import { hilog } from '@kit.PerformanceAnalysisKit';
import { AnimationType } from './LxAppNavigation';
import { NavigationButton } from './LxAppDrawables';

// NavigationBar layout constants
const NAVIGATION_BAR_TOTAL_HEIGHT = 88;        // Status bar + content area
const NAVIGATION_BAR_CONTENT_HEIGHT = 44;      // Navigation content area height
const NAVIGATION_BAR_LEFT_PADDING = 0;         // Left padding of NavigationBar content
const NAVIGATION_BAR_RIGHT_PADDING = 12;       // Right padding of NavigationBar content
const NAVIGATION_BAR_TITLE_FONT_SIZE = 18;     // Navigation bar title font size

// Navigation button constants
const NAVIGATION_BUTTON_SIZE = 44;                // Navigation button width/height - large touch target

// Common colors
const COLOR_WHITE = '#ffffff';
const COLOR_BLACK = '#000000';
const COLOR_TRANSPARENT = Color.Transparent;


/**
 * NavigationBar state interface
 */
export interface NavigationBarState {
  navigationBarTitleText: string;
  navigationBarBackgroundColor: number;
  navigationBarTextStyle: string;  // "black" | "white"
  showNavbar: boolean;             // Whether to show navbar
  showBackButton: boolean;         // Whether to show back button
  showHomeButton: boolean;         // Whether to show home button
}

const DOMAIN = 0x0000;
const TAG = 'LingXia.NavigationBar';

// Export layout constants for external use (backward compatibility)
export const NAVBAR_TOTAL_HEIGHT = NAVIGATION_BAR_TOTAL_HEIGHT;
export const NAVBAR_CONTENT_HEIGHT = NAVIGATION_BAR_CONTENT_HEIGHT;
export const NAVBAR_LEFT_PADDING = NAVIGATION_BAR_LEFT_PADDING;
export { NAVIGATION_BUTTON_SIZE };

/**
 * Convert ARGB color value to ResourceColor
 */
function toResourceColor(colorValue: number): ResourceColor {
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
 * Helper function to convert integer to AnimationType enum
 * @param type - Integer animation type (0=None, 1=Forward, 2=Backward)
 * @returns AnimationType enum or null if invalid
 */
export function animationTypeFromInt(type: number): AnimationType | null {
  // Convert to number to handle string parameters from native bridge
  const numType = Number(type);

  switch (numType) {
    case 1: return AnimationType.Forward;
    case 2: return AnimationType.Backward;
    default: return AnimationType.None; // 0 or any other value
  }
}

@Component
export struct NavigationBar {
  @Prop config: NavigationBarState;
  @Prop appId: string = '';
  @Prop currentPath: string = '';
  @Prop translateX: number = 0;
  @Prop statusBarHeight: number = 44; // Add status bar height prop
  @Prop forceShowButtons: boolean = false; // Force show buttons even when navbar is hidden

  onNavigationClick?: (appId: string, isHomeButton: boolean) => boolean;

  build() {
    Stack() {
      // Background layer - only show when navbar should be visible and not transparent
      if (this.shouldShowBackground()) {
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(this.getBackgroundColor())
      }

      // Navigation bar content area (below status bar) - use Row layout
      Row() {
        // Left side: Navigation button (back button takes priority over home button)
        if (this.shouldShowButtons()) {
          NavigationButton({
            isBackButton: this.config.showBackButton,
            onPressed: () => {
              const isBackButton = this.config.showBackButton;
              const isHomeButton = !isBackButton;
              if (this.onNavigationClick) {
                this.onNavigationClick(this.appId, isHomeButton);
              }
            }
          })
          .width(NAVIGATION_BUTTON_SIZE) // Large touch target
          .height(NAVIGATION_BUTTON_SIZE) // Large touch target
          .margin({ left: 4 }) // Button margin for optimal positioning
        } else {
          // Placeholder to maintain layout balance
          Blank()
            .width(NAVIGATION_BUTTON_SIZE)
            .height(NAVIGATION_BUTTON_SIZE)
            .margin({ left: 4 })
        }

        // Center: Title - only show when navbar is visible
        if (this.config.showNavbar) {
          Text(this.config.navigationBarTitleText || '')
            .fontSize(NAVIGATION_BAR_TITLE_FONT_SIZE)
            .fontWeight(FontWeight.Medium)
            .fontColor(this.getTextColor())
            .textAlign(TextAlign.Center)
            .layoutWeight(1) // Take remaining space for center alignment
        } else {
          Blank().layoutWeight(1) // Take space even when no title
        }

        // Right side: Placeholder for balance
        Blank()
          .width(NAVIGATION_BUTTON_SIZE + 10) // Match left side effective width
          .height(NAVIGATION_BUTTON_SIZE)
      }
      .width('100%')
      .height(NAVIGATION_BAR_CONTENT_HEIGHT)
      .justifyContent(FlexAlign.SpaceBetween) // Distribute space evenly
      .alignItems(VerticalAlign.Center) // Center vertically
      .padding({
        left: NAVIGATION_BAR_LEFT_PADDING,
        right: NAVIGATION_BAR_RIGHT_PADDING
      })
    }
    .width('100%')
    .height(NAVIGATION_BAR_TOTAL_HEIGHT)
    .alignContent(Alignment.Bottom)
    .translate({ x: `${this.translateX}%`, y: 0 })
    .visibility(this.shouldShowNavBar() ? Visibility.Visible : Visibility.Hidden)
  }

  /**
   * Check if navbar should be shown at all
   */
  private shouldShowNavBar(): boolean {
    return this.config.showNavbar || this.forceShowButtons ||
           (this.config.showBackButton || this.config.showHomeButton);
  }

  /**
   * Check if buttons should be shown
   */
  private shouldShowButtons(): boolean {
    return this.config.showBackButton || this.config.showHomeButton;
  }

  /**
   * Check if background should be shown
   */
  private shouldShowBackground(): boolean {
    // Only show background when navbar is visible (not in button-only mode)
    if (!this.config.showNavbar) {
      return false;
    }
    const bgColor = this.config.navigationBarBackgroundColor || 0xFFFFFFFF;
    const alpha = (bgColor >>> 24) & 0xFF;
    return alpha > 0; // Only show background if alpha > 0
  }

  /**
   * Get background color
   */
  private getBackgroundColor(): ResourceColor {
    return toResourceColor(this.config.navigationBarBackgroundColor || 0xFFFFFFFF);
  }

  /**
   * Get text color
   */
  private getTextColor(): ResourceColor {
    return this.config.navigationBarTextStyle === 'white' ? COLOR_WHITE : COLOR_BLACK;
  }
}

export class NavigationBarController {
  private static instance: NavigationBarController | null = null;
  private currentConfig: NavigationBarState | null = null;
  private appId: string = '';
  private currentPath: string = '';

  private constructor() {
    hilog.info(DOMAIN, TAG, 'NavigationBarController instance created');
  }

  public static getInstance(): NavigationBarController {
    if (!NavigationBarController.instance) {
      NavigationBarController.instance = new NavigationBarController();
    }
    return NavigationBarController.instance;
  }

  public updateConfig(appId: string, path: string, config: NavigationBarState): void {
    hilog.info(DOMAIN, TAG, `Updating NavigationBar config for appId: ${appId}, path: ${path}`);
    this.appId = appId;
    this.currentPath = path;
    this.currentConfig = config;
  }

  public setNavigationBarTitle(title: string): void {
    hilog.info(DOMAIN, TAG, `setNavigationBarTitle - ${title}`);
    if (this.currentConfig) {
      this.currentConfig.navigationBarTitleText = title;
    }
  }

  /**
   * Set navigation bar color - LxApp compatible API
   * @param backgroundColor Background color (string format like "#FFFFFF" or "transparent")
   * @param textStyle Text style ('black' | 'white')
   */
  public setNavigationBarColor(backgroundColor: string, textStyle?: 'black' | 'white'): void {
    hilog.info(DOMAIN, TAG, `setNavigationBarColor - bg=${backgroundColor}, textStyle=${textStyle}`);

    if (this.currentConfig) {
      this.currentConfig.navigationBarBackgroundColor = this.parseColorToArgb(backgroundColor);
      if (textStyle) {
        this.currentConfig.navigationBarTextStyle = textStyle;
      }
    }
  }

  public showNavigationBarLoading(): void {
    hilog.info(DOMAIN, TAG, 'showNavigationBarLoading');

  }

  public hideNavigationBarLoading(): void {
    hilog.info(DOMAIN, TAG, 'hideNavigationBarLoading');

  }

  /**
   * Get current configuration
   */
  public getCurrentConfig(): NavigationBarState | null {
    return this.currentConfig;
  }

  /**
   * Get current app ID
   */
  public getCurrentAppId(): string {
    return this.appId;
  }

  /**
   * Get current path
   */
  public getCurrentPath(): string {
    return this.currentPath;
  }

  /**
   * Parse color string to ARGB number
   * @param colorStr Color string
   * @param defaultColor Default color value
   * @returns ARGB color value
   */
  private parseColorToArgb(colorStr: string, defaultColor: number = 0xFFFFFFFF): number {
    if (colorStr.toLowerCase() === 'transparent') {
      return 0x00000000;
    }

    if (colorStr.startsWith('#') && colorStr.length === 7) {
      const rgb = parseInt(colorStr.substring(1), 16);
      if (!isNaN(rgb)) {
        return 0xFF000000 | rgb; // Add full alpha channel
      }
    }

    return defaultColor;
  }
}
