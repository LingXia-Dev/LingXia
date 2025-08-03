import { hilog } from '@kit.PerformanceAnalysisKit';

/**
 * NavigationBar configuration interface - synced with native page config
 * Contains all page-level configuration including navigation bar settings
 */
export interface NavigationBarConfig {
  navigationBarTitleText: string;
  navigationBarBackgroundColor: number;
  navigationBarTextStyle: string;  // "black" | "white"
  navigationStyle: number;         // 0=default, 1=custom
  backgroundColor?: string;        // Page background color (local extension)
}

const DOMAIN = 0x0000;
const TAG = 'LingXia.NavigationBar';

@Component
export struct NavigationBar {
  @Prop config: NavigationBarConfig;
  @Prop appId: string = '';
  @Prop currentPath: string = '';
  @Prop showCapsuleButton: boolean = false; // Control capsule button display

  onBackPressed?: (appId: string) => boolean;
  onMenuPressed?: (appId: string) => void;
  onCapsuleMore?: () => void; // Capsule more button callback
  onCapsuleClose?: () => void; // Capsule close button callback

  build() {
    Stack() {
      // Background extends to status bar (only if NavigationBar is default style)
      if (this.config.navigationStyle === 0) {
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(this.getBackgroundColor())
      } else if (this.showCapsuleButton) {
        // When custom style but showing capsule, create transparent background to cover status bar
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(Color.Transparent)
      }

      // NavigationBar content area (below status bar) - use Stack for absolute positioning
      Stack() {
        // Left side: Back button (only if NavigationBar is default style)
        if (this.config.navigationStyle === 0 && this.shouldShowBackButton()) {
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
                this.onBackPressed(this.appId);
              }
            })
            Blank()
          }
          .width('100%')
          .height('100%')
        }

        // Center: Title (absolutely centered, only if NavigationBar is default style)
        if (this.config.navigationStyle === 0) {
          Text(this.config.navigationBarTitleText || '')
            .fontSize(18)
            .fontWeight(FontWeight.Medium)
            .fontColor(this.getTextColor())
            .textAlign(TextAlign.Center)
            .width('100%') // 🚀 FIX: Full width for true center alignment
        }

        // Right side: Capsule button (absolutely positioned)
        if (this.showCapsuleButton) {
          Row() {
            Blank()
            this.buildCapsuleButton()
          }
          .width('100%')
          .height('100%')
        } else if (this.config.navigationStyle === 0 && this.shouldShowMenuButton()) {
          Row() {
            Blank()
            Button() {
              Text('⋯')
                .fontSize(20)
                .fontColor(this.getTextColor())
            }
            .width(40)
            .height(40)
            .backgroundColor(Color.Transparent)
            .onClick(() => {
              hilog.info(DOMAIN, TAG, `Menu button pressed for appId: ${this.appId}`);
              if (this.onMenuPressed) {
                this.onMenuPressed(this.appId);
              }
            })
          }
          .width('100%')
          .height('100%')
        }
      }
      .width('100%')
      .height(44) // NavigationBar content height
      .alignContent(Alignment.Center) // 🚀 FIX: Center all content
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

  private shouldShowBackButton(): boolean {
    return false;
  }

  private shouldShowMenuButton(): boolean {
    return false;
  }

  /**
   * Build capsule button (integrated into NavigationBar)
   */
  @Builder
  buildCapsuleButton(): void {
    Row() {
      // More button with custom drawn three dots
      Button() {
        this.buildThreeDots()
      }
      .width(44)
      .height(36)
      .backgroundColor(Color.Transparent)
      .onClick(() => {
        if (this.onCapsuleMore) {
          this.onCapsuleMore();
        }
      })

      Column()
        .width(0.5)
        .height(18)
        .backgroundColor('#DDDDDD')

      // Close button with custom drawn circle and X
      Button() {
        this.buildCloseIcon()
      }
      .width(44)
      .height(36)
      .backgroundColor(Color.Transparent)
      .onClick(() => {
        if (this.onCapsuleClose) {
          this.onCapsuleClose();
        }
      })
    }
    .backgroundColor('rgba(255,255,255,0.9)')
    .borderRadius(20)
    .border({ width: 0.5, color: '#DDDDDD' })
  }

  /**
   * Build three dots icon
   */
  @Builder
  buildThreeDots(): void {
    Row() {
      Circle().width(3).height(3).fill('#000000')
      Circle().width(5).height(5).fill('#000000').margin({ left: 4 })
      Circle().width(3).height(3).fill('#000000').margin({ left: 4 })
    }
    .justifyContent(FlexAlign.Center)
    .alignItems(VerticalAlign.Center)
  }

  /**
   * Build close icon
   */
  @Builder
  buildCloseIcon(): void {
    Stack() {
      Circle()
        .width(16)
        .height(16)
        .fillOpacity(0)
        .stroke('#000000')
        .strokeWidth(2.5)
      Circle()
        .width(6.5)
        .height(6.5)
        .fill('#000000')
    }
  }
}

export class NavigationBarController {
  private static instance: NavigationBarController | null = null;
  private currentConfig: NavigationBarConfig | null = null;
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

  public updateConfig(appId: string, path: string, config: NavigationBarConfig): void {
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
  public getCurrentConfig(): NavigationBarConfig | null {
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
