import { hilog } from '@kit.PerformanceAnalysisKit';

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

@Component
export struct NavigationBar {
  @Prop config: NavigationBarState;
  @Prop appId: string = '';
  @Prop currentPath: string = '';

  onBackPressed?: (appId: string, isHomeButton: boolean) => boolean;

  build() {
    Stack() {
      // Background extends to status bar
      Column()
        .width('100%')
        .height('100%')
        .backgroundColor(this.getBackgroundColor())

      // NavigationBar content area (below status bar) - use Stack for absolute positioning
      Stack() {
        // Left side: Back button (priority over home button)
        if (this.config.showBackButton) {
          Row() {
            Button() {
              Text('←')
                .fontSize(20)
                .fontColor(this.getTextColor())
            }
            .width(40)
            .height(40)
            .backgroundColor(Color.Transparent)
            .onClick(() => {
              hilog.info(DOMAIN, TAG, `Back button pressed for appId: ${this.appId}`);
              if (this.onBackPressed) {
                this.onBackPressed(this.appId, false); // false = back button, not home
              }
            })
            Blank()
          }
          .width('100%')
          .height('100%')
        } else if (this.config.showHomeButton) {
          // Home button in same position as back button
          Row() {
            Button() {
              Text('🏠')
                .fontSize(18)
                .fontColor(this.getTextColor())
            }
            .width(40)
            .height(40)
            .backgroundColor(Color.Transparent)
            .onClick(() => {
              hilog.info(DOMAIN, TAG, `Home button pressed for appId: ${this.appId}`);
              if (this.onBackPressed) {
                this.onBackPressed(this.appId, true); // true = home button
              }
            })
            Blank()
          }
          .width('100%')
          .height('100%')
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
      .padding({ left: 12, right: 12 })
    }
    .width('100%')
    .height(88)
    .alignContent(Alignment.Bottom)
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
