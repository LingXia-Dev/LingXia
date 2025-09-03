import { hilog } from '@kit.PerformanceAnalysisKit';
import { NavigationType } from './LxAppNavigation';
import { NavigationButton } from './LxAppDrawables';

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

// NavigationBar layout constants
export const NAVBAR_TOTAL_HEIGHT = 88; // Status bar + content area
export const NAVBAR_CONTENT_HEIGHT = 44; // Navigation content area height
export const NAVIGATION_BUTTON_SIZE = 36; // Navigation button width/height

// NavigationBar internal positioning (matches implementation below)
export const NAVBAR_LEFT_PADDING = 4; // Left padding of NavigationBar content
export const BACK_BUTTON_MARGIN = 12; // Back button left margin
export const HOME_BUTTON_MARGIN = 4; // Home button left margin

// Calculated absolute positions for external use
export const BACK_BUTTON_LEFT_POSITION = NAVBAR_LEFT_PADDING + BACK_BUTTON_MARGIN; // = 16
export const HOME_BUTTON_LEFT_POSITION = NAVBAR_LEFT_PADDING + HOME_BUTTON_MARGIN; // = 8

/**
 * Calculate the Y position for navigation buttons in absolute positioning
 *
 * Empirically determined: 45px is the correct position that aligns buttons with navbar content
 * Theory: statusBarHeight + (content_height - button_height) / 2 = 44 + 4 = 48px
 * Reality: 45px works perfectly, suggesting navbar content area starts ~3px higher than expected
 * This could be due to padding, margins, or coordinate system differences in the Stack layout
 */
export function calculateNavigationButtonYPosition(statusBarHeight: number): number {
  // Empirically verified correct position - aligns perfectly with navbar text
  return 45; // Works for statusBarHeight=44, may need adjustment for other devices
}

/**
 * Helper function to convert integer to NavigationType enum
 * @param type - Integer navigation type (0=Launch, 1=Forward, 2=Backward, 3=Replace, 4=SwitchTab)
 * @returns NavigationType enum or null if invalid
 */
export function navigationTypeFromInt(type: number): NavigationType | null {
  switch (type) {
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
      // Background extends to status bar - only show if not transparent
      if (this.shouldShowBackground()) {
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(this.getBackgroundColor())
      }

      // NavigationBar content area (below status bar) - use Stack for absolute positioning
      Stack() {
        // Left side: Navigation button (back takes priority over home)
        if (this.showNavigationButtons && (this.config.showBackButton || this.config.showHomeButton)) {
          this.buildNavigationButton()
        }

        // Center: Title
        Text(this.config.navigationBarTitleText || '')
          .fontSize(18)
          .fontWeight(FontWeight.Medium)
          .fontColor(this.getTextColor())
          .textAlign(TextAlign.Center)
          .width('100%') // Full width for true center alignment


      }
      .width('100%')
      .height(44) // NavigationBar content height
      .alignContent(Alignment.Center) // Center all content
      .padding({ left: 4, right: 12 }) // Reduce left padding to match home icon position
    }
    .width('100%')
    .height(88) // Fixed height: status bar + navbar content
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
      .width(36)
      .height(36)
      .margin({ left: this.config.showBackButton ? 12 : 4 })
      Blank()
    }
    .width('100%')
    .height('100%')
    .alignItems(VerticalAlign.Center)
  }

  private shouldShowBackground(): boolean {
    const bgColor = this.config.navigationBarBackgroundColor || 0xFFFFFFFF;
    const alpha = (bgColor >>> 24) & 0xFF;
    return alpha > 0; // Only show background if alpha > 0
  }

  private getBackgroundColor(): ResourceColor {
    return this.toResourceColor(this.config.navigationBarBackgroundColor || 0xFFFFFFFF);
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

  private getTextColor(): ResourceColor {
    if (this.config.navigationBarTextStyle === 'white') {
      return '#ffffff';
    }
    return '#000000';
  }

  private getButtonBackgroundColor(): ResourceColor {
    // Create a semi-transparent background for the button similar to iOS frosted glass effect
    const textColor = this.getTextColor();
    if (textColor === '#ffffff') {
      // Dark background, use light frosted effect
      return 'rgba(255, 255, 255, 0.2)';
    } else {
      // Light background, use dark frosted effect
      return 'rgba(0, 0, 0, 0.1)';
    }
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
   */
  private parseColorToArgb(colorStr: string, defaultColor: number = 0xFFFFFFFF): number {
    if (colorStr.toLowerCase() === 'transparent') {
      return 0x00000000;
    }

    if (colorStr.startsWith('#') && colorStr.length === 7) {
      const rgb = parseInt(colorStr.substring(1), 16);
      if (!isNaN(rgb)) {
        return 0xFF000000 | rgb; // Add full alpha
      }
    }

    return defaultColor;
  }
}
