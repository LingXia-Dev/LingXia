import { hilog } from '@kit.PerformanceAnalysisKit';
import { onCallback } from 'liblingxia.so';

/**
 * ActionSheet configuration structure
 */
export interface ActionSheetConfig {
  options: string[];
  cancelText: string;
  itemColor: string;
}

/**
 * ActionSheet result structure
 */
export interface ActionSheetResult {
  tapIndex: number;
}

const DOMAIN = 0x0000;
const TAG = 'LingXia.ActionSheet';

/**
 * ActionSheet Manager - Handles ActionSheet display and lifecycle
 */
export class LxAppActionSheet {
  private static actionSheetManagerRef: ActionSheetManager | null = null;

  /**
   * Show ActionSheet
   * @param options - Array of option strings
   * @param cancelText - Cancel button text
   * @param itemColor - Color for option items
   * @param callbackId - Callback ID for result
   */
  public static showActionSheet(options: string[], cancelText: string, itemColor: string, callbackId: string): void {
    const config: ActionSheetConfig = {
      options: options,
      cancelText: cancelText,
      itemColor: itemColor
    };

    if (LxAppActionSheet.actionSheetManagerRef) {
      LxAppActionSheet.actionSheetManagerRef.showActionSheet(config, callbackId);
    } else {
      hilog.warn(DOMAIN, TAG, 'ActionSheet manager not initialized, ignoring request');
    }
  }

  /**
   * Set ActionSheet manager reference (called by ActionSheetManager component)
   */
  public static setManagerRef(manager: ActionSheetManager): void {
    LxAppActionSheet.actionSheetManagerRef = manager;
  }

  /**
   * Clear ActionSheet manager reference
   */
  public static clearManagerRef(): void {
    LxAppActionSheet.actionSheetManagerRef = null;
  }
}

/**
 * ActionSheet Manager Component - UI management for ActionSheets
 */
@Component
export struct ActionSheetManager {
  @State isVisible: boolean = false;
  @State config: ActionSheetConfig | null = null;
  private callbackId: string = "";

  aboutToAppear() {
    LxAppActionSheet.setManagerRef(this);
  }

  aboutToDisappear() {
    LxAppActionSheet.clearManagerRef();
  }

  build() {
    if (this.isVisible && this.config) {
      Stack({ alignContent: Alignment.Bottom }) {
        // Background mask
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor('rgba(0, 0, 0, 0.5)')
          .onClick(() => {
            this.handleCancel();
          })

        // ActionSheet content - single container like Android
        Column({ space: 0 }) {
          // Options
          ForEach(this.config?.options || [], (option: string, index: number) => {
            Text(option)
              .fontSize(18)
              .fontColor(this.config?.itemColor || '#007AFF')
              .textAlign(TextAlign.Center)
              .width('100%')
              .height(56)
              .onClick(() => {
                this.handleOptionClick(index);
              })
              // Add top border radius only for first item
              .borderRadius(index === 0 ? { topLeft: 16, topRight: 16, bottomLeft: 0, bottomRight: 0 } : 0)

            if (index < (this.config?.options?.length || 0) - 1) {
              Divider()
                .height(0.5)
                .color('#E0E0E0')
            }
          })

          // Thick separator before cancel button (8dp like Android)
          Column()
            .width('100%')
            .height(8)
            .backgroundColor('#E0E0E0')

          // Cancel button - part of same container, no separate styling
          Text(this.config?.cancelText || '')
            .fontSize(18)
            .fontColor('#666666')
            .textAlign(TextAlign.Center)
            .width('100%')
            .height(56)
            .onClick(() => {
              this.handleCancel();
            })
        }
        .width('100%')
        .backgroundColor('#FFFFFF')
        // Top rounded corners - bottom attached to edge
        .borderRadius({ topLeft: 16, topRight: 16, bottomLeft: 0, bottomRight: 0 })
      }
      .width('100%')
      .height('100%')
      .zIndex(1000)
    }
  }

  private handleOptionClick(index: number): void {
    const result: ActionSheetResult = {
      tapIndex: index
    };

    onCallback(this.callbackId, true, JSON.stringify(result));

    this.hideActionSheet();
  }

  private handleCancel(): void {
    // User cancelled = error 2000
    onCallback(this.callbackId, false, '2000');

    this.hideActionSheet();
  }

  private hideActionSheet(): void {
    this.isVisible = false;
    this.config = null;
    this.callbackId = "";
  }

  /**
   * Show ActionSheet (called by LxAppActionSheet)
   */
  public showActionSheet(config: ActionSheetConfig, callbackId: string): void {
    this.config = config;
    this.callbackId = callbackId;
    this.isVisible = true;
  }
}
