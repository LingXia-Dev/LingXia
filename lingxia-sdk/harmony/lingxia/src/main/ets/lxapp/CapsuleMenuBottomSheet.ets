/**
 * Capsule Menu Bottom Sheet - Shows LxApp info and action buttons
 * Displays when clicking the 3-dots capsule button
 */

import { hilog } from '@kit.PerformanceAnalysisKit';
import { LxLog } from '../app/LxLog';
import { localizedString } from '../app/DisplayLanguageResources';
import {
  getLxAppInfo,
  getLxAppMoreActions,
  onLxappEvent,
  UiEventType,
  type LxAppInfo
} from 'liblingxia.so';
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

interface OverlayPalette {
  scrim: string;
  surface: string;
  title: string;
  body: string;
  secondaryFill: string;
  secondaryText: string;
  separator: string;
  icon: string;
}

function overlayPalette(dark: boolean): OverlayPalette {
  if (dark) {
    return {
      scrim: 'rgba(0,0,0,0.65)',
      surface: '#1C1C1E',
      title: '#FFFFFF',
      body: '#A1A1AA',
      secondaryFill: '#2C2C2E',
      secondaryText: '#E5E5E7',
      separator: 'rgba(255,255,255,0.12)',
      icon: '#E5E5E7'
    };
  }
  return {
    scrim: 'rgba(0,0,0,0.45)',
    surface: '#FFFFFF',
    title: '#000000',
    body: '#666666',
    secondaryFill: '#F5F5F5',
    secondaryText: '#555555',
    separator: '#EEEEEE',
    icon: '#333333'
  };
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

  public static show(appId: string, appearanceDark: boolean = false): boolean {
    if (CapsuleMenuManager.managerRef) {
      CapsuleMenuManager.managerRef.show(appId, appearanceDark);
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
  @State appearanceDark: boolean = false;

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
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(this.palette().scrim)
          .onClick(() => this.hide())

        Column() {
          Row() {
            Text(this.appInfo.appName)
              .fontSize(16)
              .fontWeight(FontWeight.Bold)
              .fontColor(this.palette().title)

            Text(' · ')
              .fontSize(16)
              .fontColor(this.palette().body)
              .margin({ left: 4, right: 4 })

            Row() {
              Text(this.appInfo.version)
                .fontSize(14)
                .fontColor(this.palette().body)

              if (this.releaseBadge()) {
                Text(this.releaseBadge()?.text ?? '')
                  .fontSize(10)
                  .fontWeight(FontWeight.Medium)
                  .fontColor(this.releaseBadge()?.textColor ?? this.palette().title)
                  .backgroundColor(this.releaseBadge()?.backgroundColor ?? this.palette().secondaryFill)
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

          Divider()
            .height(1)
            .color(this.palette().separator)

          Column() {
            this.actionRow(this.menuActions().slice(0, 5))
            if (this.menuActions().length > 5) {
              this.actionRow(this.menuActions().slice(5, 10))
            }
          }
          .width('100%')
          .padding({ top: 12, bottom: 28 })
        }
        .width('100%')
        .backgroundColor(this.palette().surface)
        .borderRadius({ topLeft: 16, topRight: 16 })
        .padding({ left: 20, right: 20, bottom: 16 })
      }
      .width('100%')
      .height('100%')
      .zIndex(15000)
    }
  }

  private palette(): OverlayPalette {
    return overlayPalette(this.appearanceDark);
  }

  private actionIconTint(item: MenuAction): string {
    return item.isDestructive ? '#FF3B30' : this.palette().icon;
  }

  // SRC_IN: keep source alpha, replace RGB. Image.fillColor blobs stroke-only
  // SVGs (the same assets Android color-filters and iOS templates).
  private iconColorFilter(hex: string): ColorFilter {
    const n: string = hex.replace('#', '');
    const r: number = parseInt(n.substring(0, 2), 16) / 255;
    const g: number = parseInt(n.substring(2, 4), 16) / 255;
    const b: number = parseInt(n.substring(4, 6), 16) / 255;
    return new ColorFilter([
      0, 0, 0, r, 0,
      0, 0, 0, g, 0,
      0, 0, 0, b, 0,
      0, 0, 0, 1, 0
    ]);
  }

  private actionIconSrc(item: MenuAction): string | Resource {
    if (item.bundled) {
      return $rawfile(item.iconPath);
    }
    return item.iconPath.startsWith('/') ? `file://${item.iconPath}` : item.iconPath;
  }

  @Builder
  private actionIcon(item: MenuAction): void {
    Image(this.actionIconSrc(item))
      .width(24)
      .height(24)
      .objectFit(ImageFit.Contain)
      .colorFilter(this.iconColorFilter(this.actionIconTint(item)))
  }

  @Builder
  private actionRow(actions: MenuAction[]) {
    Row() {
      ForEach(actions, (item: MenuAction) => {
              Column() {
                this.actionIcon(item)
                if (item.title) {
                  Text(item.title)
                    .fontSize(13)
                    .fontColor(item.isDestructive ? '#FF3B30' : this.palette().secondaryText)
                    .textAlign(TextAlign.Center)
                    .maxLines(2)
                    .margin({ top: 6 })
                } else if (item.titleKey) {
                  Text(localizedString(getContext(this).resourceManager, item.titleKey))
                    .fontSize(13)
                    .fontColor(item.isDestructive ? '#FF3B30' : this.palette().secondaryText)
                    .textAlign(TextAlign.Center)
                    .maxLines(2)
                    .margin({ top: 6 })
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

  public show(appId: string, appearanceDark: boolean = false): void {
    hilog.info(DOMAIN, TAG, `Showing capsule menu for app: ${appId}`);
    this.appearanceDark = appearanceDark;

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

    const releaseType: 'release' | 'preview' | 'draft' = this.appInfo.releaseType;
    const dark: boolean = this.appearanceDark;
    if (releaseType === 'draft') {
      return {
        text: 'DFT',
        textColor: dark ? '#93C5FD' : '#1D4ED8',
        backgroundColor: dark ? '#1E3A5F' : '#DBEAFE'
      };
    }

    if (releaseType === 'preview') {
      return {
        text: 'PRE',
        textColor: dark ? '#FBBF24' : '#B45309',
        backgroundColor: dark ? '#3D2E00' : '#FFEDD5'
      };
    }

    return null;
  }
}
