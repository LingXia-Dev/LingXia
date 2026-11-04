import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';
import { Lingxia } from '../app/Lingxia';
import { GpuFeedback, GpuPageModel } from './GpuPages';
import { GpuRect, sampleFacts } from './GpuEnvironment';
import {
  gpuPageMount,
  gpuPageMeasured,
  gpuPageSetVisible,
  gpuPageWindowVisible,
  getDisplayLanguage,
} from 'liblingxia.so';

/**
 * A GPU page: the `ContentSlot` Rust mounts its XComponent node into, and the chrome a
 * native renderer cannot draw itself (press feedback, text menu, recovery). Spans the
 * window; host bars overlap it and arrive as `chromeInsets` in device pixels.
 */
@Component
export struct GpuPage {
  @ObjectLink page: GpuPageModel;
  /** Host bars over the page: top, right, bottom, left, device pixels. */
  @Prop @Watch('remeasure') chromeInsets: number[] = [0, 0, 0, 0];
  @Prop @Watch('visibilityChanged') pageVisible: boolean = true;
  private rect: GpuRect = new GpuRect();
  private win: window.Window | null = null;

  aboutToAppear(): void {
    if (!this.page.mounted) {
      this.page.mounted = gpuPageMount(this.page.token, this.page.content);
    }
    // The host starts hidden; `@Watch` only reports later changes.
    gpuPageSetVisible(this.page.token, this.pageVisible);
    const context = Lingxia.context as common.UIAbilityContext;
    window.getLastWindow(context).then((win: window.Window) => {
      this.win = win;
      win.on('avoidAreaChange', () => this.remeasure());
      win.on('windowVisibilityChange', (visible: boolean) => gpuPageWindowVisible(this.page.token, visible));
      this.remeasure();
    });
  }

  aboutToDisappear(): void {
    this.win?.off('avoidAreaChange');
    this.win?.off('windowVisibilityChange');
  }

  visibilityChanged(): void {
    gpuPageSetVisible(this.page.token, this.pageVisible);
  }

  remeasure(): void {
    if (this.win === null || this.rect.width === 0) {
      return;
    }
    const context = Lingxia.context as common.UIAbilityContext;
    const facts = sampleFacts(context, this.win, this.rect, this.chromeInsets, getDisplayLanguage());
    gpuPageMeasured(this.page.token, JSON.stringify(facts), this.rect.x, this.rect.y);
  }

  @Builder
  feedback(shape: GpuFeedback) {
    // Shapes are 12 floats each: x, y, width, height, then four corner radii pairs; the
    // first is the node, the rest clip it.
    Rect()
      .position({ x: shape.shape[0], y: shape.shape[1] })
      .width(shape.shape[2])
      .height(shape.shape[3])
      .radius([[shape.shape[4], shape.shape[5]], [shape.shape[6], shape.shape[7]],
        [shape.shape[8], shape.shape[9]], [shape.shape[10], shape.shape[11]]])
      .fill(shape.ring ? Color.Transparent : shape.color)
      .stroke(shape.ring ? shape.color : Color.Transparent)
      .strokeWidth(shape.width)
      .hitTestBehavior(HitTestMode.None)
  }

  build() {
    Stack() {
      ContentSlot(this.page.content)
      if (this.page.press !== null) {
        this.feedback(this.page.press)
      }
      if (this.page.focusRing !== null) {
        this.feedback(this.page.focusRing)
      }
    }
    .width('100%')
    .height('100%')
    .onAreaChange((_: Area, area: Area) => {
      const vp2px = (v: Length) => this.getUIContext().vp2px(Number(v));
      this.rect.x = vp2px(area.globalPosition.x ?? 0);
      this.rect.y = vp2px(area.globalPosition.y ?? 0);
      this.rect.width = vp2px(area.width);
      this.rect.height = vp2px(area.height);
      this.remeasure();
    })
  }
}
