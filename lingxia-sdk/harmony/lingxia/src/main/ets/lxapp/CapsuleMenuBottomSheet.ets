/**
 * Capsule Menu Bottom Sheet - Shows LxApp info and action buttons
 * Displays when clicking the 3-dots capsule button
 */

import { hilog } from '@kit.PerformanceAnalysisKit';
import { getLxAppInfo, onUiEvent, UiEventType, type LxAppInfo } from 'liblingxia.so';
import {
  CAPSULE_ACTION_CLEAN_CACHE_RESTART,
  CAPSULE_ACTION_RESTART,
  CAPSULE_ACTION_UNINSTALL
} from './NativeBridge';

const TAG = 'CapsuleMenuBottomSheet';
const DOMAIN = 0xFF00;

interface MenuAction {
  iconPath: string;
  titleKey: string;
  action: string;
  isDestructive?: boolean;
}

/**
 * Manager class to control the bottom sheet visibility from anywhere
 */
export class CapsuleMenuManager {
  private static managerRef: CapsuleMenuBottomSheetComponent | null = null;

  public static setManagerRef(manager: CapsuleMenuBottomSheetComponent): void {
    CapsuleMenuManager.managerRef = manager;
  }

  public static clearManagerRef(manager: CapsuleMenuBottomSheetComponent): void {
    if (CapsuleMenuManager.managerRef === manager) {
      CapsuleMenuManager.managerRef = null;
    }
  }

  public static show(appId: string): boolean {
    if (CapsuleMenuManager.managerRef) {
      CapsuleMenuManager.managerRef.show(appId);
      return true;
    }
    hilog.error(DOMAIN, TAG, 'CapsuleMenuManager: managerRef is null');
    return false;
  }

  public static hide(): void {
    if (CapsuleMenuManager.managerRef) {
      CapsuleMenuManager.managerRef.hide();
    }
  }
}

/**
 * Bottom sheet component with app info and action buttons
 */
@Component
export struct CapsuleMenuBottomSheetComponent {
  @State isVisible: boolean = false;
  @State appId: string = '';
  @State appInfo: LxAppInfo | null = null;

  private readonly actions: MenuAction[] = [
    {
      iconPath: 'icons/icon_clean_cache.svg',
      titleKey: 'lx_capsule_clean_cache',
      action: CAPSULE_ACTION_CLEAN_CACHE_RESTART
    },
    {
      iconPath: 'icons/icon_restart.svg',
      titleKey: 'lx_capsule_restart',
      action: CAPSULE_ACTION_RESTART
    },
    {
      iconPath: 'icons/icon_uninstall.svg',
      titleKey: 'lx_capsule_uninstall',
      action: CAPSULE_ACTION_UNINSTALL,
      isDestructive: true
    }
  ];

  aboutToAppear() {
    CapsuleMenuManager.setManagerRef(this);
  }

  aboutToDisappear() {
    CapsuleMenuManager.clearManagerRef(this);
  }

  build() {
    if (this.isVisible && this.appInfo) {
      Stack({ alignContent: Alignment.Bottom }) {
        // Semi-transparent overlay mask
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor('rgba(0, 0, 0, 0.5)')
          .onClick(() => this.hide())

        // Bottom sheet container
        Column() {
          // Header: App name and version in one line
          Row() {
            Text(this.appInfo.appName)
              .fontSize(16)
              .fontWeight(FontWeight.Bold)
              .fontColor('#000000')

            Text(' · ')
              .fontSize(16)
              .fontColor('#CCCCCC')
              .margin({ left: 4, right: 4 })

            Text(this.appInfo.version)
              .fontSize(14)
              .fontColor('#999999')
          }
          .width('100%')
          .padding({ left: 20, right: 20, top: 16, bottom: 12 })
          .alignItems(VerticalAlign.Center)

          // Separator
          Divider()
            .height(1)
            .color('#EEEEEE')

          // Horizontal button row
          Row() {
            ForEach(this.actions, (item: MenuAction, index: number) => {
              Column() {
                Image($rawfile(item.iconPath))
                  .width(24)
                  .height(24)
                  .objectFit(ImageFit.Contain)
                  .margin({ bottom: 6 })

                Text($r(`app.string.${item.titleKey}`))
                  .fontSize(13)
                  .fontColor(item.isDestructive ? '#FF3B30' : '#555555')
                  .textAlign(TextAlign.Center)
              }
              .layoutWeight(1)
              .padding(12)
              .onClick(() => this.handleAction(item.action))

              // Spacer between buttons (not after last)
              if (index < this.actions.length - 1) {
                Column()
                  .width(16)
              }
            })
          }
          .width('100%')
          .padding({ top: 12, bottom: 12 })
          .justifyContent(FlexAlign.Center)
        }
        .width('100%')
        .backgroundColor('#FFFFFF')
        .borderRadius({ topLeft: 16, topRight: 16 })
        .padding({ left: 20, right: 20, bottom: 16 })
      }
      .width('100%')
      .height('100%')
      .zIndex(1000)
    }
  }

  public show(appId: string): void {
    hilog.info(DOMAIN, TAG, `Showing capsule menu for app: ${appId}`);

    // Get app info
    const info: LxAppInfo | null = getLxAppInfo(appId);
    if (info) {
      this.appId = appId;
      this.appInfo = info;
      this.isVisible = true;
    } else {
      hilog.error(DOMAIN, TAG, `Failed to get LxApp info for: ${appId}`);
      this.appId = '';
      this.appInfo = null;
      this.isVisible = false;
    }
  }

  public hide(): void {
    this.isVisible = false;
    this.appId = '';
    this.appInfo = null;
  }

  private handleAction(action: string): void {
    hilog.info(DOMAIN, TAG, `Action selected: ${action}`);

    // Send UI event to Rust
    const success: boolean = onUiEvent(this.appId, UiEventType.CapsuleClick, action);

    if (!success) {
      hilog.error(DOMAIN, TAG, `Failed to send UI event for action: ${action}`);
    }

    // Hide the menu
    this.hide();
  }
}
