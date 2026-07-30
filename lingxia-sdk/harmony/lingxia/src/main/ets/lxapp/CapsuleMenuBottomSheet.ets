/**
 * Capsule Menu Bottom Sheet - Shows LxApp info and action buttons
 * Displays when clicking the 3-dots capsule button
 */

import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxLog } from '../app/LxLog';
import { getLxAppInfo, getLxAppMoreActions, onLxappEvent, UiEventType, type LxAppInfo } from 'liblingxia.so';
import {
  CAPSULE_ACTION_CLEAN_CACHE_RESTART,
  CAPSULE_ACTION_RESTART,
  CAPSULE_ACTION_UNINSTALL
} from '../app/NativeBridge';

const TAG = 'CapsuleMenuBottomSheet';
const DOMAIN = 0xFF00;

interface MenuAction {
  iconPath: string;
  titleKey?: string;
  title?: string;
  action: string;
  isDestructive?: boolean;
  bundled?: boolean;
}

interface MoreActionItem {
  label: string;
  iconPath: string;
}

interface MoreActionSnapshot {
  generation: number;
  items: MoreActionItem[];
}

interface ReleaseBadge {
  text: string;
  textColor: string;
  backgroundColor: string;
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
    LxLog.e(TAG, 'CapsuleMenuManager: managerRef is null');
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
  @State customActions: MenuAction[] = [];

  private readonly actions: MenuAction[] = [
    {
      iconPath: 'icons/icon_clean_cache.svg',
      titleKey: 'lx_capsule_clean_cache',
      action: CAPSULE_ACTION_CLEAN_CACHE_RESTART,
      bundled: true
    },
    {
      iconPath: 'icons/icon_restart.svg',
      titleKey: 'lx_capsule_restart',
      action: CAPSULE_ACTION_RESTART,
      bundled: true
    },
    {
      iconPath: 'icons/icon_uninstall.svg',
      titleKey: 'lx_capsule_uninstall',
      action: CAPSULE_ACTION_UNINSTALL,
      bundled: true
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

            Row() {
              Text(this.appInfo.version)
                .fontSize(14)
                .fontColor('#999999')

              if (this.releaseBadge()) {
                Text(this.releaseBadge()?.text ?? '')
                  .fontSize(10)
                  .fontWeight(FontWeight.Medium)
                  .fontColor(this.releaseBadge()?.textColor ?? '#000000')
                  .backgroundColor(this.releaseBadge()?.backgroundColor ?? '#FFFFFF')
                  .borderRadius(8)
                  .padding({ left: 6, right: 6, top: 2, bottom: 2 })
                  .margin({ left: 4, top: -8 })
              }
            }
            .alignItems(VerticalAlign.Center)
          }
          .width('100%')
          .padding({ left: 20, right: 20, top: 16, bottom: 12 })
          .alignItems(VerticalAlign.Center)

          // Separator
          Divider()
            .height(1)
            .color('#EEEEEE')

          Column() {
            this.actionRow(this.menuActions().slice(0, 5))
            if (this.menuActions().length > 5) {
              this.actionRow(this.menuActions().slice(5, 10))
            }
          }
          .width('100%')
          .padding({ top: 12, bottom: 12 })
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

  @Builder
  private actionRow(actions: MenuAction[]) {
    Row() {
      ForEach(actions, (item: MenuAction) => {
              Column() {
                if (item.bundled) {
                  Image($rawfile(item.iconPath))
                    .width(24)
                    .height(24)
                    .objectFit(ImageFit.Contain)
                    .margin({ bottom: 6 })
                } else {
                  Image(item.iconPath.startsWith('/') ? `file://${item.iconPath}` : item.iconPath)
                    .width(24)
                    .height(24)
                    .objectFit(ImageFit.Contain)
                    .margin({ bottom: 6 })
                }

                if (item.title) {
                  Text(item.title)
                    .fontSize(13)
                    .fontColor(item.isDestructive ? '#FF3B30' : '#555555')
                    .textAlign(TextAlign.Center)
                    .maxLines(2)
                } else if (item.titleKey) {
                  Text($r(`app.string.${item.titleKey}`))
                    .fontSize(13)
                    .fontColor(item.isDestructive ? '#FF3B30' : '#555555')
                    .textAlign(TextAlign.Center)
                    .maxLines(2)
                }
              }
              .width(`${100 / actions.length}%`)
              .height(88)
              .padding({ left: 4, right: 4, top: 12, bottom: 12 })
              .onClick(() => this.handleAction(item.action))
      })
    }
    .width('100%')
    .justifyContent(FlexAlign.Center)
  }

  public show(appId: string): void {
    hilog.info(DOMAIN, TAG, `Showing capsule menu for app: ${appId}`);

    // Get app info
    const info: LxAppInfo | null = getLxAppInfo(appId);
    if (info) {
      this.appId = appId;
      this.appInfo = info;
      this.customActions = this.parseMoreActions(appId);
      this.isVisible = true;
    } else {
      LxLog.e(TAG, `Failed to get LxApp info for: ${appId}`);
      this.appId = '';
      this.appInfo = null;
      this.customActions = [];
      this.isVisible = false;
    }
  }

  public hide(): void {
    this.isVisible = false;
    this.appId = '';
    this.appInfo = null;
    this.customActions = [];
  }

  private handleAction(action: string): void {
    hilog.info(DOMAIN, TAG, `Action selected: ${action}`);

    const appId: string = this.appId;
    this.hide();
    setTimeout(() => {
      const success: boolean = onLxappEvent(appId, UiEventType.CapsuleClick, action);
      if (!success) {
        LxLog.e(TAG, `Failed to send UI event for action: ${action}`);
      }
    }, 0);
  }

  private parseMoreActions(appId: string): MenuAction[] {
    try {
      const snapshot: MoreActionSnapshot = JSON.parse(getLxAppMoreActions(appId)) as MoreActionSnapshot;
      return snapshot.items.slice(0, 7).map((item: MoreActionItem, index: number): MenuAction => {
        return {
          iconPath: item.iconPath,
          title: item.label,
          action: `more:${snapshot.generation}:${index}`,
          bundled: false
        };
      });
    } catch (_) {
      return [];
    }
  }

  private menuActions(): MenuAction[] {
    return this.actions.concat(this.customActions).slice(0, 10);
  }

  private releaseBadge(): ReleaseBadge | null {
    if (!this.appInfo) {
      return null;
    }

    const releaseType: 'release' | 'preview' | 'developer' = this.appInfo.releaseType;
    if (releaseType === 'release') {
      return {
        text: 'DEV',
        textColor: '#1D4ED8',
        backgroundColor: '#DBEAFE'
      };
    }

    if (releaseType === 'preview') {
      return {
        text: 'PRE',
        textColor: '#B45309',
        backgroundColor: '#FFEDD5'
      };
    }

    return null;
  }
}
