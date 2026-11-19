import { window } from '@kit.ArkUI';
import { webview } from '@kit.ArkWeb';
import { hilog } from '@kit.PerformanceAnalysisKit';
import { common, Configuration, EnvironmentCallback } from '@kit.AbilityKit';
import { Lingxia } from '../app/Lingxia';
import { GpuFeedback, GpuPageModel, GpuRecovery, GpuSelectionHandles, GpuWebViewMount } from './GpuPages';
import { GpuFrameTextMenu, GpuFrameTextMenuTargetKinds } from './generated/GpuFrame';
import { HostOverlays } from '../lxapp/HostOverlays';
import { LingXiaWebView } from '../lxapp/LingXiaWebView';
import { WebViewInfo, WebViewManager } from '../lxapp/WebViewBridge';
import { GpuRect, sampleFacts } from './GpuEnvironment';
import {
  gpuPageMount,
  gpuPageMeasured,
  gpuPageSetVisible,
  gpuPageWindowVisible,
  gpuPageRetry,
  gpuPageBack,
  gpuPageTextMenuAction,
  gpuPageTextMenuDismissed,
  gpuPageTextHandleDrag,
  gpuPageMoveTextSelection,
  getDisplayLanguage,
} from 'liblingxia.so';

/** Core menu actions in `MenuAction` order, and their labels. */
const MENU_ACTIONS: string[] = ['cut', 'copy', 'paste', 'select_all', 'undo', 'redo'];
const MENU_LABELS: Resource[] = [$r('app.string.lx_text_menu_cut'), $r('app.string.lx_text_menu_copy'),
  $r('app.string.lx_text_menu_paste'), $r('app.string.lx_text_menu_select_all'),
  $r('app.string.lx_text_menu_undo'), $r('app.string.lx_text_menu_redo')];
/** Text menu height and its gap to the selection, vp. */
const MENU_HEIGHT = 40;
const MENU_GAP = 8;
/** A handle's knob radius, and the touch target around it, vp. */
const KNOB = 7;
// Window stop/start lines sit with the container's in the device log.
const LOG_DOMAIN = 0x0000;
const LOG_TAG = 'LingXia.Container';
const HANDLE_TOUCH = 40;

interface GpuWebEntry {
  webTag: string;
  controller: webview.WebviewController;
  generation: string;
}

/** One embedded `<webview>` slot positioned underneath `ContentSlot` in `GpuPage`. */
@Component
struct GpuWebSlot {
  @ObjectLink page: GpuPageModel;
  @Prop webTag: string = '';
  @Prop appearanceDark: boolean = false;
  @Prop pageVisible: boolean = true;
  controller: webview.WebviewController = new webview.WebviewController();

  private findMount(): GpuWebViewMount | null {
    for (let i = 0; i < this.page.webViews.length; i++) {
      if (this.page.webViews[i].webTag === this.webTag) {
        return this.page.webViews[i];
      }
    }
    return null;
  }

  private shown(mount: GpuWebViewMount | null): boolean {
    return mount !== null
      && this.pageVisible
      && !mount.degraded
      && mount.visibleBounds[2] > 0
      && mount.visibleBounds[3] > 0;
  }

  private outerPosition(mount: GpuWebViewMount | null): Position {
    if (mount === null) {
      return { x: 0, y: 0 };
    }
    const vis = this.shown(mount);
    return {
      x: vis ? mount.visibleBounds[0] : mount.bounds[0],
      y: vis ? mount.visibleBounds[1] : mount.bounds[1],
    };
  }

  private outerWidth(mount: GpuWebViewMount | null): number {
    if (mount === null) {
      return 1;
    }
    return Math.max(1, this.shown(mount) ? mount.visibleBounds[2] : mount.bounds[2]);
  }

  private outerHeight(mount: GpuWebViewMount | null): number {
    if (mount === null) {
      return 1;
    }
    return Math.max(1, this.shown(mount) ? mount.visibleBounds[3] : mount.bounds[3]);
  }

  private innerPosition(mount: GpuWebViewMount | null): Position {
    if (mount === null || !this.shown(mount)) {
      return { x: 0, y: 0 };
    }
    return {
      x: mount.bounds[0] - mount.visibleBounds[0],
      y: mount.bounds[1] - mount.visibleBounds[1],
    };
  }

  private innerWidth(mount: GpuWebViewMount | null): number {
    return mount !== null ? Math.max(1, mount.bounds[2]) : 1;
  }

  private innerHeight(mount: GpuWebViewMount | null): number {
    return mount !== null ? Math.max(1, mount.bounds[3]) : 1;
  }

  private radii(mount: GpuWebViewMount | null): BorderRadiuses {
    if (mount === null) {
      return { topLeft: 0, topRight: 0, bottomLeft: 0, bottomRight: 0 };
    }
    if (mount.radii !== null && mount.radii !== undefined && mount.radii.length >= 4) {
      return {
        topLeft: Math.min(mount.radii[0][0], mount.radii[0][1]),
        topRight: Math.min(mount.radii[1][0], mount.radii[1][1]),
        bottomRight: Math.min(mount.radii[2][0], mount.radii[2][1]),
        bottomLeft: Math.min(mount.radii[3][0], mount.radii[3][1]),
      };
    }
    const r = mount.radius > 0 ? mount.radius : 0;
    return { topLeft: r, topRight: r, bottomRight: r, bottomLeft: r };
  }

  private hasRadius(mount: GpuWebViewMount | null): boolean {
    if (mount === null) {
      return false;
    }
    if (mount.radius > 0) {
      return true;
    }
    if (mount.radii !== null && mount.radii !== undefined && mount.radii.length >= 4) {
      return mount.radii[0][0] > 0 || mount.radii[1][0] > 0
        || mount.radii[2][0] > 0 || mount.radii[3][0] > 0;
    }
    return false;
  }

  build() {
    Stack({ alignContent: Alignment.TopStart }) {
      Column() {
        LingXiaWebView({
          webTag: this.webTag,
          controller: this.controller,
          syncRender: true,
          appearanceDark: this.findMount()?.dark ?? this.appearanceDark,
          renderVisibility: Visibility.Visible,
          renderWidth: this.innerWidth(this.findMount()),
          renderHeight: this.innerHeight(this.findMount()),
          renderBorderRadius: this.radii(this.findMount()),
          renderClip: this.hasRadius(this.findMount()),
          renderHitTest: this.shown(this.findMount()) ? HitTestMode.Default : HitTestMode.None,
        })
      }
      .position(this.innerPosition(this.findMount()))
      .width(this.innerWidth(this.findMount()))
      .height(this.innerHeight(this.findMount()))
    }
    .position(this.outerPosition(this.findMount()))
    .width(this.outerWidth(this.findMount()))
    .height(this.outerHeight(this.findMount()))
    .clip(true)
    .opacity(this.shown(this.findMount()) ? 1 : 0)
    .hitTestBehavior(this.shown(this.findMount()) ? HitTestMode.Default : HitTestMode.None)
    .accessibilityLevel((!this.shown(this.findMount()) || (this.findMount()?.accessibilityHidden === true))
      ? 'no-hide-descendants' : 'auto')
  }
}

/**
 * A GPU page: the `ContentSlot` Rust mounts its XComponent node into, and the chrome a
 * native renderer cannot draw itself (press feedback, selection handles and text menu,
 * recovery). Spans the window; host bars overlap it and arrive as `chromeInsets` in device
 * pixels.
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
  /** This page's own window listeners: `off` without them drops every page's on the window. */
  private onAvoidArea: ((options: window.AvoidAreaOptions) => void) | null = null;
  private onWindowVisible: ((visible: boolean) => void) | null = null;
  private onKeyboardShown: ((info: window.KeyboardInfo) => void) | null = null;
  private onKeyboardHidden: ((info: window.KeyboardInfo) => void) | null = null;
  /** The top of the keyboard panel on screen, device pixels; `null` while it is hidden. */
  private keyboardTop: number | null = null;
  /** Set once the page goes, so a window lookup still in flight subscribes nothing. */
  private gone: boolean = false;
  /** The newest system configuration (font size, appearance) and the subscription that brings it. */
  private config: Configuration | null = null;
  private environment: number = -1;
  /** The failure whose action was pressed: its button waits for the next report. */
  @State retried: GpuRecovery | null = null;
  @State backed: GpuRecovery | null = null;
  @State menuWidth: number = 0;
  /** The page's size, vp, for keeping the menu on screen. */
  @State pageWidth: number = 0;
  /** Bumped when child WebView controllers attach or detach so `renderableWebEntries` refreshes. */
  @State childWebRev: number = 0;
  private webViewUiCallback: (action: string, info: WebViewInfo) => void = (_action: string, info: WebViewInfo) => {
    if (info.webtag.indexOf('~child~') >= 0) {
      this.childWebRev += 1;
    }
  };
  /** The grabbed handle: which end, and the finger's offset from its line middle. */
  private grabbed: number = -1;
  private grabOffset: number[] = [0, 0];
  private messages: Resource[] = [$r('app.string.lx_gpu_update_required'), $r('app.string.lx_gpu_session_failed'),
    $r('app.string.lx_gpu_update_failed'), $r('app.string.lx_gpu_display_failed')];
  private actions: Resource[] = [$r('app.string.lx_gpu_retry'), $r('app.string.lx_gpu_reload')];

  private keyboardFocus: () => boolean = () => this.getUIContext().getFocusController().isActive();

  aboutToAppear(): void {
    this.gone = false;
    this.page.keyboardFocus = this.keyboardFocus;
    WebViewManager.addUiCallback(this.webViewUiCallback);
    if (!this.page.mounted) {
      this.page.mounted = gpuPageMount(this.page.token, this.page.content);
    }
    // The host starts hidden; `@Watch` only reports later changes.
    gpuPageSetVisible(this.page.token, this.pageVisible);
    const context = Lingxia.context as common.UIAbilityContext;
    // A system font size change reaches the page as new text-scale facts.
    const callback: EnvironmentCallback = {
      onConfigurationUpdated: (config: Configuration): void => {
        this.config = config;
        this.remeasure();
      },
      onMemoryLevel: (_level: number): void => {}
    };
    this.environment = context.getApplicationContext().on('environment', callback);
    // This component's own window: the last-created one is a float's sub-window while it exists.
    const named: window.Window | undefined = this.ownWindow();
    (named !== undefined ? Promise.resolve(named) : window.getLastWindow(context)).then((win: window.Window) => {
      if (this.gone) {
        return;
      }
      this.win = win;
      const onAvoidArea = (_options: window.AvoidAreaOptions): void => {
        this.remeasure();
      };
      const onWindowVisible = (visible: boolean): void => {
        hilog.info(LOG_DOMAIN, LOG_TAG, `gpu window visible token=${this.page.token} visible=${visible}`);
        gpuPageWindowVisible(this.page.token, visible);
      };
      // Both fire again when the panel changes height: a candidate row, another input method.
      const onKeyboardShown = (info: window.KeyboardInfo): void => {
        this.keyboardTop = info.endRect.height > 0 ? info.endRect.top : null;
        hilog.info(LOG_DOMAIN, LOG_TAG,
          `gpu keyboard panel token=${this.page.token} top=${info.endRect.top} height=${info.endRect.height}`);
        this.remeasure();
      };
      const onKeyboardHidden = (_info: window.KeyboardInfo): void => {
        this.keyboardTop = null;
        this.remeasure();
      };
      this.onAvoidArea = onAvoidArea;
      this.onWindowVisible = onWindowVisible;
      this.onKeyboardShown = onKeyboardShown;
      this.onKeyboardHidden = onKeyboardHidden;
      win.on('avoidAreaChange', onAvoidArea);
      win.on('windowVisibilityChange', onWindowVisible);
      try {
        win.on('keyboardWillShow', onKeyboardShown);
        win.on('keyboardDidShow', onKeyboardShown);
        win.on('keyboardWillHide', onKeyboardHidden);
      } catch (_) {
        // A device without these events keeps the avoid area alone.
      }
      this.remeasure();
    });
  }

  private ownWindow(): window.Window | undefined {
    try {
      const name: string | undefined = this.getUIContext().getWindowName();
      return name ? window.findWindow(name) : undefined;
    } catch (_) {
      return undefined;
    }
  }

  aboutToDisappear(): void {
    WebViewManager.removeUiCallback(this.webViewUiCallback);
    if (this.environment >= 0) {
      (Lingxia.context as common.UIAbilityContext).getApplicationContext().off('environment', this.environment);
      this.environment = -1;
    }
    this.gone = true;
    // A replacement component may already have set its own.
    if (this.page.keyboardFocus === this.keyboardFocus) {
      this.page.keyboardFocus = null;
    }
    const win = this.win;
    const onAvoidArea = this.onAvoidArea;
    const onWindowVisible = this.onWindowVisible;
    if (win !== null && onAvoidArea !== null) {
      win.off('avoidAreaChange', onAvoidArea);
    }
    if (win !== null && onWindowVisible !== null) {
      win.off('windowVisibilityChange', onWindowVisible);
    }
    if (win !== null && this.onKeyboardShown !== null && this.onKeyboardHidden !== null) {
      try {
        win.off('keyboardWillShow', this.onKeyboardShown);
        win.off('keyboardDidShow', this.onKeyboardShown);
        win.off('keyboardWillHide', this.onKeyboardHidden);
      } catch (_) {
        // Never subscribed on this device.
      }
    }
    this.onKeyboardShown = null;
    this.onKeyboardHidden = null;
    this.onAvoidArea = null;
    this.onWindowVisible = null;
    this.win = null;
    this.page.onScreen = null;
  }

  visibilityChanged(): void {
    gpuPageSetVisible(this.page.token, this.pageVisible);
    this.trackOnScreen();
  }

  private trackOnScreen(): void {
    this.page.onScreen = this.pageVisible && this.rect.width > 0 ? this.rect : null;
  }

  remeasure(): void {
    if (this.win === null || this.rect.width === 0) {
      return;
    }
    const context = Lingxia.context as common.UIAbilityContext;
    const facts = sampleFacts(context, this.win, this.rect, this.chromeInsets, getDisplayLanguage(), this.config,
      this.keyboardTop);
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

  private recoveryBack(recovery: GpuRecovery): void {
    if (this.backed === recovery) {
      return;
    }
    this.backed = recovery;
    if (!gpuPageBack(this.page.token)) {
      this.backed = null;
    }
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
        if (recovery.back) {
          Button($r('app.string.lx_gpu_back'))
            .enabled(this.backed !== recovery)
            .onClick(() => this.recoveryBack(recovery))
            .onAreaChange((_: Area, area: Area) => HostOverlays.place(this.recoverySource(), 'back',
              () => this.text($r('app.string.lx_gpu_back')), 'button', area, () => this.recoveryBack(recovery)))
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

  /** Static-text menus only: an editor's actions run through its input connection, not here. */
  private menuShown(): GpuFrameTextMenu | null {
    const menu = this.page.textMenu;
    return menu !== null && menu.target.kind === GpuFrameTextMenuTargetKinds.TEXT ? menu : null;
  }

  /** Above the selection, or below it when the top inset leaves no room; kept on the page. */
  private menuPosition(menu: GpuFrameTextMenu): Position {
    const x = menu.anchor[0], y = menu.anchor[1], width = menu.anchor[2], height = menu.anchor[3];
    const top = px2vp(this.chromeInsets[0]);
    const above = y - MENU_GAP - MENU_HEIGHT;
    const left = Math.min(Math.max(x + width / 2 - this.menuWidth / 2, 8), Math.max(this.pageWidth - this.menuWidth - 8, 8));
    return { x: left, y: above >= top ? above : y + height + MENU_GAP };
  }

  private menuContains(menu: GpuFrameTextMenu, x: number, y: number): boolean {
    const at = this.menuPosition(menu);
    const left = Number(at.x), top = Number(at.y);
    return x >= left && x <= left + this.menuWidth && y >= top && y <= top + MENU_HEIGHT;
  }

  @Builder
  textMenu(menu: GpuFrameTextMenu) {
    Row() {
      ForEach(menu.actions.filter((action: string) => MENU_ACTIONS.indexOf(action) >= 0), (action: string) => {
        Text(MENU_LABELS[MENU_ACTIONS.indexOf(action)])
          .fontSize(14)
          .fontColor(this.page.chrome?.dark ? Color.White : '#202226')
          .padding({ left: 12, right: 12 })
          .height(MENU_HEIGHT)
          .onClick(() => gpuPageTextMenuAction(this.page.token, MENU_ACTIONS.indexOf(action)))
      }, (action: string) => action)
    }
    .height(MENU_HEIGHT)
    .borderRadius(MENU_HEIGHT / 2)
    .backgroundColor(this.page.chrome?.dark ? '#2C2E33' : Color.White)
    .shadow(ShadowStyle.OUTER_DEFAULT_SM)
    .position(this.menuPosition(menu))
    .onAreaChange((_: Area, area: Area) => {
      this.menuWidth = Number(area.width);
    })
  }

  @Builder
  handle(handles: GpuSelectionHandles, which: number, end: number[]) {
    // The knob hangs below the line end; the touch target is centred on the knob.
    Stack() {
      Column()
        .width(2)
        .height(KNOB + 1)
        .backgroundColor(handles.color)
        .position({ x: HANDLE_TOUCH / 2 - 1, y: HANDLE_TOUCH / 2 - KNOB - 9 })
      Circle()
        .width(KNOB * 2)
        .height(KNOB * 2)
        .fill(handles.color)
        .position({ x: HANDLE_TOUCH / 2 - KNOB, y: HANDLE_TOUCH / 2 - KNOB })
    }
    .width(HANDLE_TOUCH)
    .height(HANDLE_TOUCH)
    .position({ x: end[0] - HANDLE_TOUCH / 2, y: end[1] + 8 - HANDLE_TOUCH / 2 })
    .onTouch((event: TouchEvent) => this.dragHandle(which, end, event))
  }

  /** A touch in page vp, whichever component reports it. */
  private pagePoint(touch: TouchObject): number[] {
    return [touch.windowX - px2vp(this.rect.x), touch.windowY - px2vp(this.rect.y)];
  }

  private dragHandle(which: number, end: number[], event: TouchEvent): void {
    event.stopPropagation();
    const touch = event.touches[0];
    if (touch === undefined) {
      return;
    }
    const point = this.pagePoint(touch);
    const x = point[0], y = point[1];
    switch (event.type) {
      case TouchType.Down:
        this.grabbed = which;
        this.grabOffset = [x - end[0], y - end[2]];
        gpuPageTextHandleDrag(this.page.token, true);
        break;
      case TouchType.Move:
        if (this.grabbed >= 0) {
          gpuPageMoveTextSelection(this.page.token, this.grabbed, x - this.grabOffset[0], y - this.grabOffset[1]);
        }
        break;
      case TouchType.Up:
      case TouchType.Cancel:
        if (this.grabbed >= 0) {
          this.grabbed = -1;
          gpuPageTextHandleDrag(this.page.token, false);
        }
        break;
    }
  }

  private renderableWebEntries(): GpuWebEntry[] {
    void this.childWebRev;
    const entries: GpuWebEntry[] = [];
    for (let i = 0; i < this.page.webViews.length; i++) {
      const mount = this.page.webViews[i];
      if (!mount.webTag) {
        continue;
      }
      const controller = WebViewManager.getController(mount.webTag);
      if (!controller) {
        continue;
      }
      const generation = WebViewManager.getGenerationForController(mount.webTag, controller);
      entries.push({ webTag: mount.webTag, controller: controller, generation: generation });
    }
    return entries;
  }

  build() {
    Stack() {
      ForEach(this.renderableWebEntries(), (entry: GpuWebEntry) => {
        GpuWebSlot({
          page: this.page,
          webTag: entry.webTag,
          controller: entry.controller,
          appearanceDark: this.page.chrome?.dark ?? false,
          pageVisible: this.pageVisible,
        })
      }, (entry: GpuWebEntry) => `${entry.webTag}@${entry.generation}`)
      // Unmounting the slot destroys the XComponent's surface; remounting creates a new one.
      if (this.page.surfaceKept) {
        ContentSlot(this.page.content)
      }
      if (!this.page.revealed || this.page.holdSurface) {
        Column()
          .width('100%')
          .height('100%')
          .backgroundColor(this.floor)
          .hitTestBehavior(HitTestMode.None)
      }
      // Keyed by its shape: a builder's by-value argument is not redrawn when the feedback moves
      // from one node to another without clearing in between, as a focus ring does on each Tab.
      ForEach([this.page.press, this.page.focusRing], (shown: GpuFeedback | null) => {
        if (shown !== null) {
          this.feedback(shown)
        }
      }, (shown: GpuFeedback | null, which: number) =>
        shown === null ? `${which}` : `${which}:${shown.color}:${shown.width}:${shown.shape.join(',')}`)
      if (this.page.handles !== null) {
        ForEach(this.page.handles.ends, (end: number[] | null, which: number) => {
          if (end !== null) {
            this.handle(this.page.handles as GpuSelectionHandles, which, end)
          }
        }, (end: number[] | null, which: number) => `${which}:${end?.join(',') ?? ''}`)
      }
      if (this.menuShown() !== null) {
        this.textMenu(this.menuShown() as GpuFrameTextMenu)
      }
      if (this.page.recovery !== null) {
        this.recoveryPanel(this.page.recovery)
      }
    }
    .id(`lx-gpu-page-${this.page.token}`)
    .width('100%')
    .height('100%')
    .onTouch((event: TouchEvent) => {
      // A touch outside the menu closes it, as the system's floating menus do.
      const menu = this.menuShown();
      const touch = event.touches[0];
      if (event.type === TouchType.Down && menu !== null && touch !== undefined
        && !this.menuContains(menu, this.pagePoint(touch)[0], this.pagePoint(touch)[1])) {
        // The core takes a reported dismissal as already closed and sends no update.
        this.page.textMenu = null;
        gpuPageTextMenuDismissed(this.page.token);
      }
    })
    .onAreaChange((_: Area, area: Area) => {
      const vp2px = (v: Length) => this.getUIContext().vp2px(Number(v));
      this.rect.x = vp2px(area.globalPosition.x ?? 0);
      this.rect.y = vp2px(area.globalPosition.y ?? 0);
      this.rect.width = vp2px(area.width);
      this.rect.height = vp2px(area.height);
      this.pageWidth = Number(area.width);
      this.trackOnScreen();
      this.remeasure();
    })
  }
}
