import { display, window } from '@kit.ArkUI';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { Lingxia } from '../app/Lingxia';
import { LxLog } from '../app/LxLog';

const DOMAIN = 0xA000;
const TAG = 'LingXia.DevMark';
const ROUTE = 'LingXiaDevServiceMark';
const WINDOW_NAME = 'lx_dev_service_mark';

/**
 * Prod build on the dev service. Guest lxapps and the auth sheet are their
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
  build() {
    Row() {
      Stack() {
        Circle()
          .width(6)
          .height(6)
          .fill(Color.White)
          .position({ x: 9, y: 8 })
        Text('DEV')
          .fontSize(11)
          .fontWeight(FontWeight.Medium)
          .fontColor(Color.White)
          .textAlign(TextAlign.Center)
          .width(36)
          .height(16)
          .rotate({ angle: -90 })
          .position({ x: -6, y: 29 })
      }
      .width(24)
      .height(60)
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
