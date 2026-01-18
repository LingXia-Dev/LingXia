import { hilog } from '@kit.PerformanceAnalysisKit';

/**
 * Toast icon types
 */
export enum ToastIcon {
  Success = 0,
  Error = 1,
  Loading = 2,
  None = 3
}

/**
 * Toast position types
 */
export enum ToastPosition {
  Top = 0,
  Center = 1,
  Bottom = 2
}

/**
 * Toast configuration structure
 */
export interface ToastConfig {
  title: string;
  icon: ToastIcon;
  image?: string;
  duration: number;
  mask: boolean;
}

const DOMAIN = 0x0000;
const TAG = 'LingXia.Toast';

/**
 * Toast Manager - Handles toast display and lifecycle
 */
export class LxAppToast {
  private static currentToastId: number = 0;
  private static currentTimer: number | null = null;
  private static toastManagerRef: ToastManager | null = null;

  public static normalizeNativeArgs(rawArgs: Object[]): Object[] | null {
    if (!Array.isArray(rawArgs) || rawArgs.length < 6) {
      return null;
    }

    const iconMap: Record<string, number> = { 'success': 0, 'error': 1, 'loading': 2, 'none': 3 };
    const positionMap: Record<string, number> = { 'top': 0, 'center': 1, 'bottom': 2 };

    const parseIcon = (v: Object): number => {
      const s = String(v ?? '').toLowerCase().trim();
      const mapped = iconMap[s];
      if (mapped !== undefined) { return mapped; }
      const n = Number(s);
      if (Number.isFinite(n) && n >= 0 && n <= 3) { return Math.floor(n); }
      return 3;
    };

    const parsePosition = (v: Object): number => {
      const s = String(v ?? '').toLowerCase().trim();
      const mapped = positionMap[s];
      if (mapped !== undefined) { return mapped; }
      const n = Number(s);
      if (Number.isFinite(n) && n >= 0 && n <= 2) { return Math.floor(n); }
      return 1;
    };

    const parseBool = (v: Object): boolean => {
      const s = String(v ?? '').toLowerCase().trim();
      if (s === 'true' || s === '1') { return true; }
      if (s === 'false' || s === '0' || s === '') { return false; }
      return !!v;
    };

    const parseDurationSeconds = (v: Object): number => {
      const num = Number(String(v ?? '').trim());
      if (!Number.isFinite(num) || num < 0) { return 1.5; }
      return num >= 50 ? (num / 1000) : num;
    };

    return [
      String(rawArgs[0] ?? ''),
      parseIcon(rawArgs[1]),
      String(rawArgs[2] ?? ''),
      parseDurationSeconds(rawArgs[3]),
      parseBool(rawArgs[4]),
      parsePosition(rawArgs[5])
    ];
  }

  /**
   * Show toast with specified configuration
   * @param title - Toast message content
   * @param icon - Icon type (success, error, loading, none)
   * @param image - Custom image path (overrides icon parameter)
   * @param duration - Display duration in seconds (default: 1.5)
   * @param mask - Whether to show transparent mask to prevent touch through
   * @param position - Toast position (default: center)
   */
  public static showToast(
    title: string,
    icon: ToastIcon = ToastIcon.Success,
    image?: string,
    duration: number = 1.5,
    mask: boolean = false,
    position: ToastPosition = ToastPosition.Center
  ): void {
    // Hide any existing toast first
    LxAppToast.hideToast();

    const config: ToastConfig = {
      title,
      icon,
      image,
      duration,
      mask
    };

    if (LxAppToast.toastManagerRef) {
      LxAppToast.toastManagerRef.showToast(config, position);

      // Auto-hide after duration
      if (duration > 0) {
        LxAppToast.currentTimer = setTimeout(() => {
          LxAppToast.hideToast();
        }, duration * 1000);
      }
    } else {
      hilog.warn(DOMAIN, TAG, 'Toast manager not initialized');
    }
  }

  /**
   * Hide current toast immediately
   */
  public static hideToast(): void {
    hilog.info(DOMAIN, TAG, 'Hiding toast');

    if (LxAppToast.currentTimer) {
      clearTimeout(LxAppToast.currentTimer);
      LxAppToast.currentTimer = null;
    }

    if (LxAppToast.toastManagerRef) {
      LxAppToast.toastManagerRef.hideToast();
    }
  }

  /**
   * Set toast manager reference (called by ToastManager component)
   */
  public static setManagerRef(manager: ToastManager): void {
    LxAppToast.toastManagerRef = manager;
  }

  /**
   * Clear toast manager reference
   */
  public static clearManagerRef(): void {
    LxAppToast.toastManagerRef = null;
  }
}

/**
 * Toast Manager Component - UI management for toasts
 */
@Component
export struct ToastManager {
  @State isVisible: boolean = false;
  @State config: ToastConfig | null = null;
  @State toastPosition: ToastPosition = ToastPosition.Center;

  aboutToAppear() {
    LxAppToast.setManagerRef(this);
  }

  aboutToDisappear() {
    LxAppToast.clearManagerRef();
  }

  build() {
    if (this.isVisible && this.config) {
      Stack({ alignContent: this.getToastAlignment() }) {
        // Background mask (only if enabled)
        if (this.config.mask) {
          Column()
            .width('100%')
            .height('100%')
            .backgroundColor(Color.Black)
            .opacity(0.3)
        }

        // Toast content
        this.buildToastContent()
      }
      .width('100%')
      .height('100%')
      .zIndex(9999)
      .hitTestBehavior(HitTestMode.Block)
      .onClick(() => {
        if (!this.config?.mask) {
          this.hideToast();
        }
      })
    }
  }

  @Builder
  buildToastContent() {
    Column({ space: this.config?.icon === ToastIcon.None ? 0 : 12 }) {
      // Icon or custom image
      if (this.config?.image) {
        this.buildToastImage(this.config.image)
      } else if (this.config?.icon !== ToastIcon.None) {
        this.buildToastIcon(this.config?.icon || ToastIcon.Success)
      }

      // Title text
      Text(this.config?.title || '')
        .fontSize(16)
        .fontWeight(FontWeight.Medium)
        .fontColor(Color.White)
        .textAlign(TextAlign.Center)
        .maxLines(this.config?.icon === ToastIcon.None ? 3 : 2)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .lineHeight(22)
        .wordBreak(WordBreak.BREAK_ALL)
    }
    .padding({ left: 24, right: 24, top: 16, bottom: 16 })
    .backgroundColor('#CC000000') // 80% black opacity
    .borderRadius(12)
    .constraintSize({ 
      minWidth: 120, 
      maxWidth: 280,
      minHeight: this.config?.icon === ToastIcon.None ? 60 : 80
    })
    .justifyContent(FlexAlign.Center)
    .alignItems(HorizontalAlign.Center)
    .margin(this.getToastMargin())
  }  @Builder
  buildToastIcon(icon: ToastIcon) {
    if (icon === ToastIcon.Loading) {
      LoadingProgress()
        .width(28)
        .height(28)
        .color(Color.White)
    } else {
      // Use simple styled icons
      Text(this.getIconSymbol(icon))
        .fontSize(28)
        .fontColor(this.getIconColor(icon))
        .fontWeight(FontWeight.Medium)
    }
  }

  @Builder
  buildToastImage(imagePath: string) {
    // Only support full path
    Image('file://' + imagePath)
      .width(32)
      .height(32)
      .objectFit(ImageFit.Contain)
      .onError(() => {
        hilog.warn(DOMAIN, TAG, `Failed to load image: ${imagePath}`);
      })
  }

  private getIconSymbol(icon: ToastIcon): string {
    switch (icon) {
      case ToastIcon.Success:
        return '✓';
      case ToastIcon.Error:
        return '✕';
      default:
        return '✓';
    }
  }

  private getIconColor(icon: ToastIcon): ResourceColor {
    switch (icon) {
      case ToastIcon.Success:
        return Color.Green;
      case ToastIcon.Error:
        return Color.Red;
      default:
        return Color.White;
    }
  }

  private getToastAlignment(): Alignment {
    switch (this.toastPosition) {
      case ToastPosition.Top:
        return Alignment.Top;
      case ToastPosition.Bottom:
        return Alignment.Bottom;
      case ToastPosition.Center:
      default:
        return Alignment.Center;
    }
  }

  private getToastMargin(): Margin {
    switch (this.toastPosition) {
      case ToastPosition.Top:
        return { top: 100 };
      case ToastPosition.Bottom:
        return { bottom: 100 };
      case ToastPosition.Center:
      default:
        return { top: 0, bottom: 0 };
    }
  }

  /**
   * Show toast (called by LxAppToast)
   */
  public showToast(config: ToastConfig, position: ToastPosition): void {
    this.config = config;
    this.toastPosition = position;
    this.isVisible = true;
  }

  /**
   * Hide toast (called by LxAppToast)
   */
  public hideToast(): void {
    this.isVisible = false;
    this.config = null;
  }
}
