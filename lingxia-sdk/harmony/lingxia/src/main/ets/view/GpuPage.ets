import { window } from '@kit.ArkUI';
import { common } from '@kit.AbilityKit';
import { Lingxia } from '../app/Lingxia';
import { GpuFeedback, GpuPageModel, GpuRecovery } from './GpuPages';
import { HostOverlays } from '../lxapp/HostOverlays';
import { GpuRect, sampleFacts } from './GpuEnvironment';
import {
  gpuPageMount,
  gpuPageMeasured,
  gpuPageSetVisible,
  gpuPageWindowVisible,
  gpuPageRetry,
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
  /** The page floor, shown until the surface holds a frame: an empty surface is black. */
  @Prop floor: ResourceColor = Color.White;
  private rect: GpuRect = new GpuRect();
  private win: window.Window | null = null;
  /** The failure whose action was pressed: its button waits for the next report. */
  @State retried: GpuRecovery | null = null;
  private messages: Resource[] = [$r('app.string.lx_gpu_update_required'), $r('app.string.lx_gpu_session_failed'),
    $r('app.string.lx_gpu_update_failed'), $r('app.string.lx_gpu_display_failed')];
  private actions: Resource[] = [$r('app.string.lx_gpu_retry'), $r('app.string.lx_gpu_reload')];

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

  /** The recovery panel as automation reads it: window chrome a tap reaches. */
  private recoverySource(): string {
    return `gpu-recovery-${this.page.token}`;
  }

  private text(resource: Resource): string {
    return this.getUIContext().getHostContext()?.resourceManager.getStringSync(resource.id) ?? '';
  }

  private retry(recovery: GpuRecovery): void {
    if (this.retried === recovery) {
      return;
    }
    this.retried = recovery;
    gpuPageRetry(this.page.token);
  }

  @Builder
  recoveryPanel(recovery: GpuRecovery) {
    // Host-owned: page content cannot replace its action. A failure over a kept frame sits at
    // the bottom; one with nothing worth showing covers the page.
    Column() {
      Column({ space: 12 }) {
        Text(this.messages[recovery.message] ?? this.messages[3])
          .fontSize(16)
          .fontColor(this.page.chrome?.dark ? Color.White : '#202226')
          .width('100%')
          .onAreaChange((_: Area, area: Area) => HostOverlays.place(this.recoverySource(), 'message',
            () => this.text(this.messages[recovery.message] ?? this.messages[3]), 'text', area))
        if (recovery.action >= 0) {
          Button(this.retried === recovery ? $r('app.string.lx_gpu_retrying') : this.actions[recovery.action])
            .enabled(this.retried !== recovery)
            .onClick(() => this.retry(recovery))
            .onAreaChange((_: Area, area: Area) => HostOverlays.place(this.recoverySource(), 'action',
              () => this.text(this.actions[recovery.action]), 'button', area, () => this.retry(recovery)))
        }
      }
      .width('100%')
      .padding(16)
      .backgroundColor(this.page.chrome?.dark ? '#202226' : Color.White)
    }
    .width('100%')
    .height('100%')
    .justifyContent(recovery.retained ? FlexAlign.End : FlexAlign.Start)
    .backgroundColor(recovery.retained ? Color.Transparent : (this.page.chrome?.dark ? '#202226' : Color.White))
    .hitTestBehavior(recovery.retained ? HitTestMode.Transparent : HitTestMode.Default)
    .padding({
      top: px2vp(this.chromeInsets[0]), right: px2vp(this.chromeInsets[1]),
      bottom: px2vp(this.chromeInsets[2]), left: px2vp(this.chromeInsets[3]),
    })
    .onAppear(() => HostOverlays.show(this.recoverySource()))
    .onDisAppear(() => {
      HostOverlays.hide(this.recoverySource());
      HostOverlays.forget(this.recoverySource(), 'message');
      HostOverlays.forget(this.recoverySource(), 'action');
    })
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
      if (!this.page.revealed || this.page.holdSurface) {
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(this.floor)
          .hitTestBehavior(HitTestMode.None)
      }
      if (this.page.press !== null) {
        this.feedback(this.page.press)
      }
      if (this.page.focusRing !== null) {
        this.feedback(this.page.focusRing)
      }
      if (this.page.recovery !== null) {
        this.recoveryPanel(this.page.recovery)
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
