import { hilog } from '@kit.PerformanceAnalysisKit';
import { NavigationType } from './LxAppNavigation';
import { NavigationButton } from './LxAppDrawables';

// NavigationBar layout constants
const NAVIGATION_BAR_TOTAL_HEIGHT = 88;        // Status bar + content area
const NAVIGATION_BAR_CONTENT_HEIGHT = 44;      // Navigation content area height
const NAVIGATION_BAR_LEFT_PADDING = 4;         // Left padding of NavigationBar content
const NAVIGATION_BAR_RIGHT_PADDING = 12;       // Right padding of NavigationBar content
const NAVIGATION_BAR_TITLE_FONT_SIZE = 18;     // Navigation bar title font size

// Navigation button constants
const NAVIGATION_BUTTON_SIZE = 36;                // Navigation button width/height
const NAVIGATION_BUTTON_BACK_MARGIN_LEFT = 12;    // Back button left margin
const NAVIGATION_BUTTON_HOME_MARGIN_LEFT = 4;     // Home button left margin
const NAVIGATION_BUTTON_Z_INDEX = 1000;           // Navigation button z-index
const NAVIGATION_BUTTON_Y_POSITION = 45;          // Navigation button vertical position (empirical value)

// Calculated absolute positions
const NAVIGATION_BUTTON_POSITION_BACK_LEFT = 16;  // LEFT_PADDING(4) + BACK_MARGIN_LEFT(12)
const NAVIGATION_BUTTON_POSITION_HOME_LEFT = 8;   // LEFT_PADDING(4) + HOME_MARGIN_LEFT(4)

// Common colors
const COLOR_WHITE = '#ffffff';
const COLOR_BLACK = '#000000';
const COLOR_TRANSPARENT = Color.Transparent;

// Frosted glass effect colors
const FROSTED_GLASS_LIGHT = 'rgba(255, 255, 255, 0.2)';  // For dark navbar backgrounds
const FROSTED_GLASS_DARK = 'rgba(0, 0, 0, 0.1)';          // For light navbar backgrounds

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
export const BACK_BUTTON_MARGIN = NAVIGATION_BUTTON_BACK_MARGIN_LEFT;
export const HOME_BUTTON_MARGIN = NAVIGATION_BUTTON_HOME_MARGIN_LEFT;
export const BACK_BUTTON_LEFT_POSITION = NAVIGATION_BUTTON_POSITION_BACK_LEFT;
export const HOME_BUTTON_LEFT_POSITION = NAVIGATION_BUTTON_POSITION_HOME_LEFT;

/**
 * Calculate the Y position for navigation buttons in absolute positioning
 * Empirically determined: 45px is the correct position that aligns buttons with navbar content
 */
export function calculateNavigationButtonYPosition(statusBarHeight: number): number {
  return NAVIGATION_BUTTON_Y_POSITION;
}

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
 * Get frosted glass background color for navigation buttons
 */
function getFrostedGlassColor(isWhiteText: boolean): ResourceColor {
  return isWhiteText ? FROSTED_GLASS_LIGHT : FROSTED_GLASS_DARK;
}

/**
 * Helper function to convert integer to NavigationType enum
 * @param type - Integer navigation type (0=Launch, 1=Forward, 2=Backward, 3=Replace, 4=SwitchTab)
 * @returns NavigationType enum or null if invalid
 */
export function navigationTypeFromInt(type: number): NavigationType | null {
  // Convert to number to handle string parameters from native bridge
  const numType = Number(type);

  switch (numType) {
    case 0: return NavigationType.Launch;
    case 1: return NavigationType.Forward;
    case 2: return NavigationType.Backward;
    case 3: return NavigationType.Replace;
    case 4: return NavigationType.SwitchTab;
    default: return null;
  }
}

@Component
export struct NavigationBar {
  @Prop config: NavigationBarState;
  @Prop appId: string = '';
  @Prop currentPath: string = '';
  @Prop translateX: number = 0;
  @Prop statusBarHeight: number = 44; // Add status bar height prop
  @Prop showNavigationButtons: boolean = true; // Control whether to show navigation buttons

  onBackPressed?: (appId: string, isHomeButton: boolean) => boolean;

  build() {
    Stack() {
      // Background layer - only show when not transparent
      if (this.shouldShowBackground()) {
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(this.getBackgroundColor())
      }

      // Navigation bar content area (below status bar) - use Stack for absolute positioning
      Stack() {
        // Left side: Navigation button (back button takes priority over home button)
        if (this.showNavigationButtons && (this.config.showBackButton || this.config.showHomeButton)) {
          this.buildNavigationButton()
        }

        // Center: Title
        Text(this.config.navigationBarTitleText || '')
          .fontSize(NAVIGATION_BAR_TITLE_FONT_SIZE)
          .fontWeight(FontWeight.Medium)
          .fontColor(this.getTextColor())
          .textAlign(TextAlign.Center)
          .width('100%') // Full width for true center alignment
      }
      .width('100%')
      .height(NAVIGATION_BAR_CONTENT_HEIGHT)
      .alignContent(Alignment.Center) // Center all content
      .padding({
        left: NAVIGATION_BAR_LEFT_PADDING,
        right: NAVIGATION_BAR_RIGHT_PADDING
      })
    }
    .width('100%')
    .height(NAVIGATION_BAR_TOTAL_HEIGHT)
    .alignContent(Alignment.Bottom)
    .translate({ x: `${this.translateX}%`, y: 0 })
  }

  @Builder
  buildNavigationButton(): void {
    Row() {
      NavigationButton({
        isBackButton: this.config.showBackButton,
        color: this.getTextColor(),
        bgColor: this.getButtonBackgroundColor(),
        onPressed: () => {
          const isBackButton = this.config.showBackButton;
          hilog.info(DOMAIN, TAG, `${isBackButton ? 'Back' : 'Home'} button pressed for appId: ${this.appId}`);
          if (this.onBackPressed) {
            this.onBackPressed(this.appId, !isBackButton);
          }
        }
      })
      .width(NAVIGATION_BUTTON_SIZE)
      .height(NAVIGATION_BUTTON_SIZE)
      .margin({
        left: this.config.showBackButton ?
          NAVIGATION_BUTTON_BACK_MARGIN_LEFT :
          NAVIGATION_BUTTON_HOME_MARGIN_LEFT
      })
      Blank()
    }
    .width('100%')
    .height('100%')
    .alignItems(VerticalAlign.Center)
  }

  /**
   * Check if background should be shown
   */
  private shouldShowBackground(): boolean {
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

  /**
   * Get button background color (frosted glass effect)
   */
  private getButtonBackgroundColor(): ResourceColor {
    const isWhiteText = this.config.navigationBarTextStyle === 'white';
    return getFrostedGlassColor(isWhiteText);
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
