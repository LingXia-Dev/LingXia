import { hilog } from '@kit.PerformanceAnalysisKit';
import { onCallback } from 'liblingxia.so';

/**
 * Picker configuration structure
 */
export interface PickerConfig {
  columns: string[][];
  cascading?: boolean;
  cascadingData?: Record<string, string[]>;
  cancelText: string;
  cancelButtonColor: string;
  cancelTextColor: string;
  confirmText: string;
  confirmButtonColor: string;
  confirmTextColor: string;
}

/**
 * Picker result structure
 */
export interface PickerResult {
  index: number | number[];
  cancel?: boolean;
  confirm?: boolean;
}

/**
 * Picker scroll result structure
 */
interface PickerScrollResult {
  index: number | number[];
}

const DOMAIN = 0x0000;
const TAG = 'LingXia.Picker';

/**
 * Picker Manager - Handles picker display and lifecycle
 */
export class LxAppPicker {
  private static pickerManagerRef: PickerManager | null = null;

  /**
   * Show picker with individual parameters (matching Rust interface)
   */
  public static showPicker(
    columns: string,
    cancelText: string,
    cancelButtonColor: string,
    cancelTextColor: string,
    confirmText: string,
    confirmButtonColor: string,
    confirmTextColor: string,
    callbackId: string
  ): void {
    try {
      const parsedData: (string[] | Record<string, string[]>)[] = JSON.parse(columns);

      let config: PickerConfig;

      // Check if this is cascading picker (second element is object, not array)
      if (Array.isArray(parsedData) && parsedData.length === 2 &&
          Array.isArray(parsedData[0]) &&
          typeof parsedData[1] === 'object' && !Array.isArray(parsedData[1])) {
        // Cascading picker
        config = {
          columns: [parsedData[0]], // Only first column initially
          cascading: true,
          cascadingData: parsedData[1],
          cancelText: cancelText,
          cancelButtonColor: cancelButtonColor,
          cancelTextColor: cancelTextColor,
          confirmText: confirmText,
          confirmButtonColor: confirmButtonColor,
          confirmTextColor: confirmTextColor
        };
      } else {
        // Regular picker (single or dual column)
        const parsedColumns: string[][] = parsedData as string[][];
        config = {
          columns: parsedColumns,
          cancelText: cancelText,
          cancelButtonColor: cancelButtonColor,
          cancelTextColor: cancelTextColor,
          confirmText: confirmText,
          confirmButtonColor: confirmButtonColor,
          confirmTextColor: confirmTextColor
        };
      }

      if (LxAppPicker.pickerManagerRef) {
        LxAppPicker.pickerManagerRef.showPicker(config, callbackId);
      } else {
        hilog.warn(DOMAIN, TAG, 'Picker manager not initialized, ignoring request');
      }
    } catch (error) {
      hilog.error(DOMAIN, TAG, `Failed to parse columns: ${error}`);
    }
  }

  /**
   * Set picker manager reference
   */
  public static setManagerRef(manager: PickerManager): void {
    LxAppPicker.pickerManagerRef = manager;
  }

  /**
   * Clear picker manager reference
   */
  public static clearManagerRef(): void {
    LxAppPicker.pickerManagerRef = null;
  }
}

/**
 * Picker Manager Component - UI management for pickers
 */
@Component
export struct PickerManager {
  @State isVisible: boolean = false;
  @State config: PickerConfig | null = null;
  @State selectedIndices: number[] = [];
  @State refreshKey: number = 0;
  private scrollDebounceTimer: number | null = null;
  private readonly SCROLL_DEBOUNCE_DELAY = 300;
  private isInitializing: boolean = false;
  private callbackId: string = "";

  aboutToAppear() {
    LxAppPicker.setManagerRef(this);
  }

  aboutToDisappear() {
    LxAppPicker.clearManagerRef();
    if (this.scrollDebounceTimer !== null) {
      clearTimeout(this.scrollDebounceTimer);
      this.scrollDebounceTimer = null;
    }
  }

  /**
   * Show picker with configuration
   */
  public showPicker(config: PickerConfig, callbackId: string): void {
    this.isInitializing = true;
    this.config = config;
    this.callbackId = callbackId;

    // Handle cascading picker initialization
    if (config.cascading && config.cascadingData) {
      // For cascading picker, initialize second column with data from first item
      const firstColumnItem = config.columns[0][0];
      if (firstColumnItem && config.cascadingData[firstColumnItem]) {
        config.columns.push(config.cascadingData[firstColumnItem]);
      } else {
        const firstKey = Object.keys(config.cascadingData)[0];
        config.columns.push(config.cascadingData[firstKey] || []);
      }
    }

    this.selectedIndices = new Array(config.columns.length).fill(0);
    this.refreshKey = 0;
    this.isVisible = true;

    setTimeout(() => {
      this.isInitializing = false;
    }, 100);
  }

  /**
   * Hide picker
   */
  private hidePicker(): void {
    this.isVisible = false;
    this.config = null;
    this.callbackId = "";
  }

  /**
   * Handle cancel button click
   */
  private onCancel(): void {
    let index: number | number[];
    if (this.config && this.config.columns.length === 1) {
      index = this.selectedIndices[0] ?? 0;
    } else {
      index = [...this.selectedIndices];
    }

    const result: PickerResult = {
      index: index,
      cancel: true
    };

    onCallback(this.callbackId, true, JSON.stringify(result));
    this.hidePicker();
  }

  /**
   * Handle confirm button click
   */
  private onConfirm(): void {
    let index: number | number[];
    if (this.config && this.config.columns.length === 1) {
      index = this.selectedIndices[0] ?? 0;
    } else {
      index = [...this.selectedIndices];
    }

    const result: PickerResult = {
      index: index,
      confirm: true
    };

    onCallback(this.callbackId, true, JSON.stringify(result));
    this.hidePicker();
  }

  /**
   * Handle picker value change with debouncing
   */
  private onPickerChange(columnIndex: number, value: number): void {
    this.selectedIndices[columnIndex] = value;

    // Skip sending scroll result during initialization
    if (this.isInitializing) {
      return;
    }

    if (this.scrollDebounceTimer !== null) {
      clearTimeout(this.scrollDebounceTimer);
    }

    this.scrollDebounceTimer = setTimeout(() => {
      this.sendScrollResult();
      this.scrollDebounceTimer = null;
    }, this.SCROLL_DEBOUNCE_DELAY);
  }

  /**
   * Send scroll result (real-time selection updates)
   */
  private sendScrollResult(): void {
    let index: number | number[];
    if (this.config && this.config.columns.length === 1) {
      index = this.selectedIndices[0] ?? 0;
    } else {
      index = [...this.selectedIndices];
    }

    const result: PickerScrollResult = {
      index: index
    };

    hilog.info(DOMAIN, TAG, `Scroll result: ${JSON.stringify(result)}`);
    // TODO: Currently not sending scroll results
    // onCallback(this.callbackId, true, JSON.stringify(result));
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
            this.onCancel();
          })

        // Picker content
        this.buildPickerContent()
      }
      .width('100%')
      .height('100%')
      .zIndex(1000)
    }
  }

  @Builder
  private buildPickerContent() {
    // Single container with top rounded corners like Android
    Column({ space: 0 }) {
      // Picker wheels area at top
      this.buildPickerWheels()

      // Separator line
      Divider()
        .height(0.5)
        .color('#F0F0F0')

      // Bottom buttons container
      Row() {
        // Cancel button with gray background like Android
        Text(this.config?.cancelText || 'Cancel')
          .fontSize(18)
          .fontColor(this.config?.cancelTextColor || '#007AFF')
          .fontWeight(FontWeight.Normal)
          .textAlign(TextAlign.Center)
          .width(120)
          .height(44)
          .backgroundColor(this.config?.cancelButtonColor || '#F2F2F2')
          .borderRadius(8)
          .onClick(() => {
            this.onCancel();
          })

        // Spacer
        Column()
          .width(16)

        // Confirm button with blue background like Android
        Text(this.config?.confirmText || 'Confirm')
          .fontSize(18)
          .fontColor(this.config?.confirmTextColor || '#FFFFFF')
          .fontWeight(FontWeight.Medium)
          .textAlign(TextAlign.Center)
          .width(120)
          .height(44)
          .backgroundColor(this.config?.confirmButtonColor || '#007AFF')
          .borderRadius(8)
          .onClick(() => {
            this.onConfirm();
          })
      }
      .width('100%')
      .height(80)
      .backgroundColor('#FFFFFF')
      .justifyContent(FlexAlign.Center)
      .alignItems(VerticalAlign.Center)
      .padding({ left: 20, right: 20 })
    }
    .width('100%')
    .backgroundColor(Color.White)
    .borderRadius({ topLeft: 12, topRight: 12, bottomLeft: 0, bottomRight: 0 })
  }

  @Builder
  private buildPickerWheels() {
    Column() {
      // Single column picker
      if (this.config && this.config.columns.length === 1) {
        TextPicker({
          range: this.config.columns[0],
          selected: this.selectedIndices[0]
        })
          .onChange((value: string | string[], index: number | number[]) => {
            const selectedIndex = Array.isArray(index) ? index[0] : index;
            this.onPickerChange(0, selectedIndex);
          })
          .height(200)
          .defaultPickerItemHeight(40)
          .canLoop(false)
          .width('100%')
          .selectedTextStyle({
            color: '#333333',
            font: { size: 18, weight: FontWeight.Regular }
          })
          .disappearTextStyle({
            color: '#999999',
            font: { size: 16, weight: FontWeight.Regular }
          })
      }

      // Multi column picker
      if (this.config && this.config.columns.length > 1) {
        Row({ space: 0 }) {
          // First column
          if (this.config.columns.length >= 1) {
            TextPicker({
              range: this.config.columns[0],
              selected: this.selectedIndices[0]
            })
              .onChange((value: string | string[], index: number | number[]) => {
                const selectedIndex = Array.isArray(index) ? index[0] : index;
                this.onPickerChange(0, selectedIndex);

                // Handle cascading for first column
                if (this.config && this.config.cascading && this.config.cascadingData) {
                  this.updateCascadingSecondColumn(selectedIndex);
                }
              })
              .height(200)
              .defaultPickerItemHeight(40)
              .canLoop(false)
              .layoutWeight(1)
              .padding({ left: 10, right: 10 })
              .selectedTextStyle({
                color: '#333333',
                font: { size: 18, weight: FontWeight.Regular }
              })
              .disappearTextStyle({
                color: '#999999',
                font: { size: 16, weight: FontWeight.Regular }
              })
          }

          // Second column (with refresh key for cascading)
          if (this.config.columns.length >= 2 && Array.isArray(this.config.columns[1]) && this.config.columns[1].length > 0) {
            TextPicker({
              range: this.config.columns[1],
              selected: this.selectedIndices[1] || 0
            })
              .onChange((value: string | string[], index: number | number[]) => {
                const selectedIndex = Array.isArray(index) ? index[0] : index;
                this.onPickerChange(1, selectedIndex);
              })
              .height(200)
              .defaultPickerItemHeight(40)
              .canLoop(false)
              .layoutWeight(1)
              .padding({ left: 10, right: 10 })
              .key(`second-picker-${this.refreshKey}`) // Force refresh for cascading
              .selectedTextStyle({
                color: '#333333',
                font: { size: 18, weight: FontWeight.Regular }
              })
              .disappearTextStyle({
                color: '#999999',
                font: { size: 16, weight: FontWeight.Regular }
              })
          }
        }
        .width('100%')
        .height(200)
      }
    }
    .width('100%')
    .height(200)
    .padding({ left: 20, right: 20, top: 20, bottom: 20 })
  }

  /**
   * Update cascading second column when first column changes
   */
  private updateCascadingSecondColumn(firstColumnIndex: number): void {
    if (!this.config || !this.config.cascading || !this.config.cascadingData || !this.config.columns[0]) {
      hilog.warn(DOMAIN, TAG, 'Cascading update skipped: missing config or data');
      return;
    }

    const firstColumnItem = this.config.columns[0][firstColumnIndex];
    if (firstColumnItem && this.config.cascadingData[firstColumnItem]) {
      // Update second column data
      const newSecondColumnData = this.config.cascadingData[firstColumnItem];
      if (Array.isArray(newSecondColumnData) && newSecondColumnData.length > 0) {
        this.config.columns[1] = newSecondColumnData;
        this.selectedIndices[1] = 0;
        this.refreshKey++;
      } else {
        this.config.columns[1] = [];
        this.selectedIndices[1] = 0;
        this.refreshKey++;
      }
    } else {
      const firstKey = Object.keys(this.config.cascadingData)[0];
      if (firstKey && this.config.cascadingData[firstKey]) {
        this.config.columns[1] = this.config.cascadingData[firstKey];
        this.selectedIndices[1] = 0;
        this.refreshKey++;
      }
    }
  }
}
