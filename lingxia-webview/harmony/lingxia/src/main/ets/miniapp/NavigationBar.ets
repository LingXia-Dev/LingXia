import { hilog } from '@kit.PerformanceAnalysisKit';

/**
 * NavigationBar configuration interface - synced with native page config
 * Contains all page-level configuration including navigation bar settings
 */
export interface NavigationBarConfig {
  navigationBarTitleText?: string;
  navigationBarBackgroundColor?: string;
  navigationBarTextStyle?: string;  // "black" | "white"
  navigationStyle?: string;         // "default" | "custom"
  backgroundColor?: string;         // Page background color
  hidden?: boolean;
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
      // Background extends to status bar (only if NavigationBar is not hidden)
      if (!this.config.hidden) {
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(this.getBackgroundColor())
      } else if (this.showCapsuleButton) {
        // When hidden but showing capsule, create transparent background to cover status bar
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(Color.Transparent)
      }

      // NavigationBar content area (below status bar) - use Stack for absolute positioning
      Stack() {
        // Left side: Back button (only if NavigationBar is not hidden)
        if (!this.config.hidden && this.shouldShowBackButton()) {
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

        // Center: Title (absolutely centered, only if NavigationBar is not hidden)
        if (!this.config.hidden) {
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
        } else if (!this.config.hidden && this.shouldShowMenuButton()) {
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
    if (this.config.navigationBarBackgroundColor) {
      return this.config.navigationBarBackgroundColor;
    }
    return '#0000ff';
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
   * Set navigation bar color - MiniApp compatible API
   * @param backgroundColor Background color
   * @param textStyle Text style ('black' | 'white')
   */
  public setNavigationBarColor(backgroundColor: string, textStyle?: 'black' | 'white'): void {
    hilog.info(DOMAIN, TAG, `setNavigationBarColor - bg=${backgroundColor}, textStyle=${textStyle}`);

    if (this.currentConfig) {
      this.currentConfig.navigationBarBackgroundColor = backgroundColor;
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
}