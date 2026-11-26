import { display, window } from '@kit.ArkUI';
import { inputMethod } from '@kit.IMEKit';
import { gpuOverlayEscape } from 'liblingxia.so';
import { Lingxia } from '../app/Lingxia';
import { LxLog } from '../app/LxLog';
import { GpuPage } from './GpuPage';
import { GpuPageModel } from './GpuPages';

const TAG = 'LingXia.Overlay';
const ROUTE = 'LingXiaGpuOverlay';
const PREFIX = 'lx_gpu_overlay_';

class Overlay {
  readonly page: GpuPageModel;
  win: window.Window | null = null;
  blocking: boolean = false;
  focused: boolean = false;
  closed: boolean = false;

  constructor(page: GpuPageModel) {
    this.page = page;
  }
}

/**
 * Window overlays: a see-through GPU page in a sub-window over the main window, which Rust knows
 * as window 1. Rust owns the page; this makes only the sub-window and its `GpuPage`. A toast's
 * window takes no touches and no focus; a dialog's takes both, and Back cancels it.
 */
export class GpuOverlays {
  private static overlays: Map<number, Overlay> = new Map();

  static open(page: GpuPageModel): void {
    const overlay = new Overlay(page);
    GpuOverlays.overlays.set(page.token, overlay);
    GpuOverlays.create(overlay).catch((error: Error) => LxLog.e(TAG, `open failed: ${error.message}`));
  }

  private static async create(overlay: Overlay): Promise<void> {
    const stage = Lingxia.windowStage;
    if (!stage) {
      throw new Error('windowStage missing');
    }
    const win = await stage.createSubWindowWithOptions(`${PREFIX}${overlay.page.token}`, {
      title: '',
      decorEnabled: false,
      isModal: false,
      // Above lx.surface float sub-windows (zLevel 1), below the dev mark (10).
      zLevel: 5,
    });
    if (overlay.closed) {
      await win.destroyWindow();
      return;
    }
    overlay.win = win;
    await win.loadContentByName(ROUTE);
    const info = display.getDefaultDisplaySync();
    await win.resize(info.width, info.height);
    await win.moveWindowTo(0, 0);
    await win.setWindowLayoutFullScreen(true);
    await win.setWindowBackgroundColor('#00000000');
    await win.setWindowFocusable(overlay.focused);
    await win.setWindowTouchable(overlay.blocking);
    await win.showWindow();
    if (overlay.focused) {
      GpuOverlays.shiftFocus(overlay, true);
    }
  }

  static input(token: number, block: boolean): void {
    const overlay = GpuOverlays.overlays.get(token);
    if (overlay === undefined || overlay.blocking === block) {
      return;
    }
    overlay.blocking = block;
    overlay.win?.setWindowTouchable(block).catch((error: Error) => LxLog.e(TAG, `touchable: ${error.message}`));
  }

  static focus(token: number, focused: boolean): void {
    const overlay = GpuOverlays.overlays.get(token);
    if (overlay === undefined || overlay.focused === focused) {
      return;
    }
    overlay.focused = focused;
    if (focused) {
      // Opening a dialog hides the soft keyboard; the editor keeps its focus for when it closes.
      try {
        inputMethod.getController().hideTextInput().catch(() => {});
      } catch (_) {
      }
    }
    const win = overlay.win;
    if (win === null) {
      return;
    }
    win.setWindowFocusable(focused)
      .then(() => GpuOverlays.shiftFocus(overlay, focused))
      .catch((error: Error) => LxLog.e(TAG, `focusable: ${error.message}`));
  }

  /** Moves key focus to the overlay, or back to the main window. */
  private static shiftFocus(overlay: Overlay, focused: boolean): void {
    const stage = Lingxia.windowStage;
    const win = overlay.win;
    if (!stage || win === null) {
      return;
    }
    try {
      const main = stage.getMainWindowSync().getWindowProperties().id;
      const own = win.getWindowProperties().id;
      window.shiftAppWindowFocus(focused ? main : own, focused ? own : main)
        .catch((error: Error) => LxLog.w(TAG, `focus shift: ${error.message}`));
    } catch (error) {
      LxLog.w(TAG, `focus shift: ${error}`);
    }
  }

  static close(token: number): void {
    const overlay = GpuOverlays.overlays.get(token);
    if (overlay === undefined) {
      return;
    }
    GpuOverlays.overlays.delete(token);
    overlay.closed = true;
    if (overlay.focused) {
      GpuOverlays.shiftFocus(overlay, false);
    }
    overlay.win?.destroyWindow().catch((error: Error) => LxLog.e(TAG, `close failed: ${error.message}`));
    overlay.win = null;
  }

  /** The page an overlay window shows, by its window's name. */
  static pageOf(windowName: string | undefined): GpuPageModel | undefined {
    if (windowName === undefined || !windowName.startsWith(PREFIX)) {
      return undefined;
    }
    return GpuOverlays.overlays.get(Number.parseInt(windowName.substring(PREFIX.length)))?.page;
  }

  static focused(token: number): boolean {
    return GpuOverlays.overlays.get(token)?.focused ?? false;
  }
}

@Entry({ routeName: ROUTE })
@Component
struct GpuOverlayPage {
  @State page: GpuPageModel | undefined = undefined;

  aboutToAppear(): void {
    try {
      this.page = GpuOverlays.pageOf(this.getUIContext().getWindowName());
    } catch (error) {
      LxLog.e(TAG, `no overlay page: ${error}`);
    }
  }

  onBackPress(): boolean {
    const page = this.page;
    if (page === undefined || !GpuOverlays.focused(page.token)) {
      return false;
    }
    gpuOverlayEscape(page.token);
    return true;
  }

  build() {
    Stack() {
      if (this.page !== undefined) {
        GpuPage({ page: this.page as GpuPageModel, floor: Color.Transparent })
      }
    }
    .width('100%')
    .height('100%')
    .backgroundColor(Color.Transparent)
  }
}
