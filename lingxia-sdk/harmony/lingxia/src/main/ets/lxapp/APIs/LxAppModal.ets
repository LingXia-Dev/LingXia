import { hilog } from '@kit.PerformanceAnalysisKit';
import { onCallback } from 'liblingxia.so';

/**
 * Modal configuration structure
 */
export interface ModalConfig {
  title: string;
  content: string;
  showCancel: boolean;
  cancelText: string;
  confirmText: string;
  confirmColor?: string;
}

/**
 * Modal result structure
 */
export interface ModalResult {
  confirm: boolean;
  cancel: boolean;
}

const DOMAIN = 0x0000;
const TAG = 'LingXia.Modal';

/**
 * Modal Manager - Handles modal display and lifecycle
 */
export class LxAppModal {
  private static modalManagerRef: ModalManager | null = null;

  public static normalizeNativeArgs(rawArgs: Object[]): Object[] | null {
    if (!Array.isArray(rawArgs)) {
      return null;
    }

    // Legacy format may have 9 args:
    // title, content, showCancel, cancelText, confirmText, editable, placeholder, confirmColor, callbackId
    if (rawArgs.length === 9) {
      return [
        rawArgs[0], // title
        rawArgs[1], // content
        rawArgs[2], // showCancel
        rawArgs[3], // cancelText
        rawArgs[4], // confirmText
        rawArgs[7], // confirmColor
        rawArgs[8]  // callbackId
      ];
    }

    // New format with 7 args:
    // title, content, showCancel, cancelText, confirmText, confirmColor, callbackId
    if (rawArgs.length >= 7) {
      return rawArgs.slice(0, 7);
    }

    return null;
  }

  /**
   * Show modal with individual string parameters (refactored for direct parameter passing)
   * @param title - Modal title (required)
   * @param content - Modal content (required)
   * @param showCancel - Whether to show cancel button (required)
   * @param cancelText - Cancel button text (required)
   * @param confirmText - Confirm button text (required)
   * @param confirmColor - Confirm button color (required, can be empty string)
   * @param callbackId - Callback ID for result handling (required)
   */
  public static showModal(
    title: string,
    content: string,
    showCancel: string,
    cancelText: string,
    confirmText: string,
    confirmColor: string,
    callbackId: string
  ): void {
    const cb = String(callbackId ?? '').trim();
    if (!cb) {
      hilog.warn(DOMAIN, TAG, 'showModal: missing callbackId');
      return;
    }

    const config: ModalConfig = {
      title: title,
      content: content,
      showCancel: showCancel === "true",
      cancelText: cancelText,
      confirmText: confirmText,
      confirmColor: confirmColor || undefined
    };

    if (LxAppModal.modalManagerRef) {
      LxAppModal.modalManagerRef.showModal(config, cb);
    } else {
      hilog.warn(DOMAIN, TAG, 'Modal manager not initialized');
      onCallback(cb, false, '1000');
    }
  }

  /**
   * Set modal manager reference (called by ModalManager component)
   */
  public static setManagerRef(manager: ModalManager): void {
    LxAppModal.modalManagerRef = manager;
  }

  /**
   * Clear modal manager reference
   */
  public static clearManagerRef(): void {
    LxAppModal.modalManagerRef = null;
  }
}

/**
 * Modal Manager Component - UI management for modals
 */
@Component
export struct ModalManager {
  @State isVisible: boolean = false;
  @State config: ModalConfig | null = null;
  private callbackId: string = "";

  aboutToAppear() {
    LxAppModal.setManagerRef(this);
  }

  aboutToDisappear() {
    LxAppModal.clearManagerRef();
  }

  build() {
    if (this.isVisible && this.config) {
      Stack() {
        // Background mask - no onClick to prevent dismissing by clicking outside
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(Color.Black)
          .opacity(0.5)

        // Modal content
        this.buildModalContent()
      }
      .width('100%')
      .height('100%')
      .zIndex(10000)
    }
  }

  @Builder
  buildModalContent() {
    Column({ space: 20 }) {
      // Title (only show if not empty)
      if (this.config?.title && this.config.title.trim() !== '') {
        Text(this.config.title)
          .fontSize(18)
          .fontWeight(FontWeight.Bold)
          .fontColor(Color.Black)
          .textAlign(TextAlign.Center)
          .maxLines(2)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
      }

      // Content
      if (this.config?.content) {
        Text(this.config.content)
          .fontSize(16)
          .fontColor('#666666')
          .textAlign(TextAlign.Center)
          .maxLines(4)
          .textOverflow({ overflow: TextOverflow.Ellipsis })
          .lineHeight(24)
      }

      // Buttons
      this.buildButtons()
    }
    .width(280)
    .padding(24)
    .backgroundColor(Color.White)
    .borderRadius(12)
    .shadow({
      radius: 20,
      color: Color.Black,
      offsetX: 0,
      offsetY: 4
    })
  }

  @Builder
  buildButtons() {
    if (this.config?.showCancel) {
      // Two buttons layout
      Row({ space: 12 }) {
        Button(this.config?.cancelText || '')
          .layoutWeight(1)
          .height(44)
          .fontSize(16)
          .fontColor('#666666')
          .backgroundColor('#F5F5F5')
          .borderRadius(8)
          .onClick(() => {
            this.handleCancel();
          })

        Button(this.config?.confirmText || '')
          .layoutWeight(1)
          .height(44)
          .fontSize(16)
          .fontColor(Color.White)
          .backgroundColor(this.config.confirmColor || '#007AFF')
          .borderRadius(8)
          .onClick(() => {
            this.handleConfirm();
          })
      }
      .width('100%')
    } else {
      // Single button layout
      Button(this.config?.confirmText || '')
        .width('100%')
        .height(44)
        .fontSize(16)
        .fontColor(Color.White)
        .backgroundColor(this.config?.confirmColor || '#007AFF')
        .borderRadius(8)
        .onClick(() => {
          this.handleConfirm();
        })
    }
  }

  private handleConfirm(): void {
    hilog.info(DOMAIN, TAG, 'Modal confirmed');

    // Call callback with success result
    const result = JSON.stringify({
      confirm: true,
      cancel: false
    });
    onCallback(this.callbackId, true, result);

    this.hideModal();
  }

  private handleCancel(): void {
    hilog.info(DOMAIN, TAG, 'Modal cancelled');

    // Call callback with cancel result (user cancelled = error 2000)
    onCallback(this.callbackId, false, '2000');

    this.hideModal();
  }

  private hideModal(): void {
    this.isVisible = false;
    this.config = null;
    this.callbackId = "";
  }

  /**
   * Show modal (called by LxAppModal)
   */
  public showModal(config: ModalConfig, callbackId: string): void {
    // Force reset state first to ensure clean state
    this.isVisible = false;
    this.config = null;
    this.callbackId = "";

    // Then set new state
    this.config = config;
    this.callbackId = callbackId;
    this.isVisible = true;
  }
}
