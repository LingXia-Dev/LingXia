import { display, window } from '@kit.ArkUI';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { devMark } from 'liblingxia.so';
import { Lingxia } from '../app/Lingxia';
import { LxLog } from '../app/LxLog';

const DOMAIN = 0xA000;
const TAG = 'LingXia.DevMark';
const ROUTE = 'LingXiaDevServiceMark';
const WINDOW_NAME = 'lx_dev_service_mark';

/**
 * Touch-through dev mark. Guest lxapps and the auth sheet are their
 * own sub-windows, so a chip inside the main page paints underneath them.
 * This window sits above those (higher zLevel), is not focusable, and does
 * not take touches.
 */
export class DevServiceMarkWindow {
  private static opened: boolean = false;

  static async open(): Promise<void> {
    if (DevServiceMarkWindow.opened) {
      return;
    }
    let label = '';
    try {
      label = devMark(true);
    } catch (error) {
      LxLog.e(TAG, `dev service mark failed: ${error}`);
      return;
    }
    if (label.length === 0) {
      return;
    }
    const stage = Lingxia.windowStage;
    if (!stage) {
      LxLog.e(TAG, 'windowStage missing');
      return;
    }
    DevServiceMarkWindow.opened = true;
    try {
      const win = await stage.createSubWindowWithOptions(WINDOW_NAME, {
        title: 'DEV',
        decorEnabled: false,
        isModal: false,
        zLevel: 10,
      });
      await win.loadContentByName(ROUTE);
      const info = display.getDefaultDisplaySync();
      await win.resize(info.width, info.height);
      await win.moveWindowTo(0, 0);
      await win.setWindowBackgroundColor('#00000000');
      await win.setWindowFocusable(false);
      await win.setWindowTouchable(false);
      await win.showWindow();
      hilog.info(DOMAIN, TAG, 'dev service mark window shown');
    } catch (error) {
      DevServiceMarkWindow.opened = false;
      LxLog.e(TAG, `open failed: ${error}`);
    }
  }
}

@Entry({ routeName: ROUTE })
@Component
struct DevServiceMarkPage {
  @State label: string = '';

  aboutToAppear(): void {
    this.label = devMark(true);
  }

  private textWidth(): number {
    return this.label.length <= 3 ? 36 : this.label.length * 8 + 4;
  }

  private tabHeight(): number {
    return this.label.length <= 3 ? 60 : this.textWidth() + 28;
  }

  build() {
    Row() {
      Stack() {
        Circle()
          .width(6)
          .height(6)
          .fill(Color.White)
          .position({ x: 9, y: 8 })
        Text(this.label)
          .fontSize(11)
          .fontWeight(FontWeight.Medium)
          .fontColor(Color.White)
          .textAlign(TextAlign.Center)
          .width(this.textWidth())
          .height(16)
          .rotate({ angle: -90 })
          .position({
            x: (24 - this.textWidth()) / 2,
            y: (this.tabHeight() - 16) / 2 + (this.label.length <= 3 ? 7 : 8)
          })
      }
      .width(24)
      .height(this.tabHeight())
      .borderRadius({ topLeft: 0, topRight: 12, bottomRight: 12, bottomLeft: 0 })
      .backgroundColor('#C62828')
    }
    .alignItems(VerticalAlign.Center)
    .justifyContent(FlexAlign.Start)
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Transparent)
    .hitTestBehavior(HitTestMode.None)
  }
}
