import { hilog } from '@kit.PerformanceAnalysisKit';
import { window } from '@kit.ArkUI';
import { asideCommand, getCurrentLxapp, getLxAppInfo, getTabBar, LxAppInfo, TabBarState } from 'liblingxia.so';

const DOMAIN = 0x0000;
const TAG = 'LingXia.Aside';

const STATE_KEY = 'lxAsideState';
const RATIO_KEY = 'lxAsideDockRatio';
const COMPACT_MAX_VP = 600;
const STRIP_VP = 44;
const TAB_VP = 32;
const CONTROL_VP = 36;
const SIDE_MIN_VP = 320;
const DEFAULT_RATIO = 0.5;
const SNAP_VP = 24;
const MIN_TRAVEL_VP = 48;
const DIVIDER_HIT_VP = 24;
const TONGUE_HIT_W_VP = 28;
const TONGUE_HIT_W_COUNT_VP = 32;
const TONGUE_HIT_H_VP = 88;
const PULL_VP = 16;
const DEFAULT_ACCENT = 'rgba(22,119,255,1)';

function argbCss(color: number): string {
  const c = color >>> 0;
  const alpha = ((c >>> 24) & 0xFF) / 255;
  return `rgba(${(c >>> 16) & 0xFF},${(c >>> 8) & 0xFF},${c & 0xFF},${alpha})`;
}

/** The core plan's aside slot, as `presentLayout` hands it over. */
export interface AsideSlotPlan {
  kind: string;
  children: string[];
  activeChild?: string | null;
  visible: boolean;
  overlay: boolean;
  collapsed: boolean;
  maximized?: boolean;
}

export interface AsideTab {
  id: string;
  title: string;
  isUrl: boolean;
}

/** Everything the region and the surfaces in it draw from; replaced whole on every change. */
export interface AsideState {
  shown: boolean;
  overlay: boolean;
  /** The docked width the main gives up; zero when nothing docks. */
  dockWidth: number;
  panelX: number;
  panelWidth: number;
  stripTop: number;
  contentTop: number;
  height: number;
  active: string;
  tabs: AsideTab[];
  maximizeVisible: boolean;
  maximized: boolean;
  canResize: boolean;
  tongueShown: boolean;
  tongueCount: number;
  tongueLabel: string;
  /** The app's accent, which the tongue takes. */
  accent: string;
  windowWidth: number;
  /** The scheme of the lxapp on screen, which the panel's chrome takes. */
  dark: boolean;
}

/** The panel's chrome colours in light and dark. */
interface AsidePalette {
  panel: ResourceColor;
  line: string;
  title: string;
  muted: string;
  chip: string;
  grip: string;
  tongue: string;
  tongueEdge: string;
}

const LIGHT: AsidePalette = {
  panel: Color.White, line: '#1F000000', title: '#E6000000', muted: '#99000000', chip: '#0F000000',
  grip: '#59000000', tongue: '#FFFFFF', tongueEdge: 'rgba(0,0,0,0.14)'
};

const DARK: AsidePalette = {
  panel: '#1C1C1E', line: '#2EFFFFFF', title: '#E6FFFFFF', muted: '#99FFFFFF', chip: '#1FFFFFFF',
  grip: '#66FFFFFF', tongue: '#2C2C2E', tongueEdge: 'rgba(255,255,255,0.18)'
};

export function asidePalette(dark: boolean): AsidePalette {
  return dark ? DARK : LIGHT;
}

function emptyState(): AsideState {
  return {
    shown: false,
    overlay: false,
    dockWidth: 0,
    panelX: 0,
    panelWidth: 0,
    stripTop: 0,
    contentTop: 0,
    height: 0,
    active: '',
    tabs: [],
    maximizeVisible: false,
    maximized: false,
    canResize: false,
    tongueShown: false,
    tongueCount: 0,
    tongueLabel: '',
    accent: DEFAULT_ACCENT,
    windowWidth: 0,
    dark: false
  };
}

/** A child the surface layer owns and closes, such as a URL surface. */
class AsideChild {
  id: string;
  ownerAppId: string;
  isUrl: boolean;
  title: () => string;
  close: () => void;

  constructor(id: string, ownerAppId: string, isUrl: boolean, title: () => string, close: () => void) {
    this.id = id;
    this.ownerAppId = ownerAppId;
    this.isUrl = isUrl;
    this.title = title;
    this.close = close;
  }
}

/**
 * The side panel, as the core's plan places it: docked beside the main where the window's
 * short side is regular (an unfolded foldable, a tablet), over it otherwise (a phone, or a
 * maximized slot). The same model as Android's `AsideRegion` and the desktop's aside slot.
 */
export class AsideRegionModel {
  private static children: Map<string, AsideChild> = new Map();
  /** Children a plan has listed: one missing from a later plan was taken out of the slot. */
  private static placed: Set<string> = new Set();
  private static dark: boolean = false;
  private static slot: AsideSlotPlan | null = null;
  private static width: number = 0;
  private static height: number = 0;
  private static statusTop: number = 0;
  private static ratio: number = DEFAULT_RATIO;
  private static showContentFormat: string = 'Show %1$s';

  static emptyState(): AsideState {
    return emptyState();
  }

  /** The window's size in vp, its status bar inset, and the strings the tongue speaks. */
  static setWindow(width: number, height: number, statusTop: number): void {
    if (width === AsideRegionModel.width && height === AsideRegionModel.height &&
      statusTop === AsideRegionModel.statusTop) {
      return;
    }
    AsideRegionModel.width = width;
    AsideRegionModel.height = height;
    AsideRegionModel.statusTop = statusTop;
    AsideRegionModel.publish();
  }

  /** The lxapp on screen changed scheme. */
  static setDark(dark: boolean): void {
    if (AsideRegionModel.dark === dark) {
      return;
    }
    AsideRegionModel.dark = dark;
    AsideRegionModel.publish();
  }

  static setShowContentFormat(format: string): void {
    if (format) {
      AsideRegionModel.showContentFormat = format;
    }
  }

  static restoreRatio(saved: number | undefined): void {
    if (saved !== undefined && saved > 0 && saved < 1) {
      AsideRegionModel.ratio = saved;
    }
  }

  static applyPlan(slots: AsideSlotPlan[] | undefined): void {
    let next: AsideSlotPlan | null = null;
    for (const slot of slots ?? []) {
      if (slot.kind === 'panel') {
        next = slot;
        break;
      }
    }
    AsideRegionModel.slot = next;
    const listed: string[] = next?.children ?? [];
    listed.forEach((id: string) => {
      if (AsideRegionModel.children.has(id)) {
        AsideRegionModel.placed.add(id);
      }
    });
    // The plan dropped a child: the surface layer closes it.
    const dropped: string[] = [];
    AsideRegionModel.placed.forEach((id: string) => {
      if (listed.indexOf(id) < 0) {
        dropped.push(id);
      }
    });
    dropped.forEach((id: string) => {
      AsideRegionModel.placed.delete(id);
      AsideRegionModel.children.get(id)?.close();
    });
    AsideRegionModel.publish();
  }

  /** Hosts a surface as the child [id]; the core's next plan places it. */
  static adopt(id: string, ownerAppId: string, isUrl: boolean, title: () => string, close: () => void): void {
    AsideRegionModel.children.set(id, new AsideChild(id, ownerAppId, isUrl, title, close));
    AsideRegionModel.publish();
  }

  /** Lets go of a child; the surface layer tears its view down. */
  static release(id: string): void {
    if (!AsideRegionModel.children.delete(id)) {
      return;
    }
    AsideRegionModel.placed.delete(id);
    AsideRegionModel.publish();
  }

  static hosts(id: string): boolean {
    return AsideRegionModel.children.has(id);
  }

  /** Whether the window keeps the slot beside the main; a phone shows it over the main. */
  private static canDock(): boolean {
    return Math.min(AsideRegionModel.width, AsideRegionModel.height) >= COMPACT_MAX_VP;
  }

  private static showing(): boolean {
    const slot = AsideRegionModel.slot;
    if (slot === null || !slot.visible || slot.collapsed || !slot.activeChild) {
      return false;
    }
    return AsideRegionModel.children.has(slot.activeChild);
  }

  /** Shown over the main: a phone's slot, or a maximized one. */
  static isOverlay(): boolean {
    const slot = AsideRegionModel.slot;
    if (slot === null || !AsideRegionModel.showing()) {
      return false;
    }
    return slot.overlay || slot.maximized === true || !AsideRegionModel.canDock();
  }

  private static canResize(): boolean {
    return AsideRegionModel.width - 2 * SIDE_MIN_VP >= MIN_TRAVEL_VP;
  }

  private static clampWidth(width: number): number {
    const window = AsideRegionModel.width;
    if (window < 2 * SIDE_MIN_VP) {
      return Math.round(window / 2);
    }
    return Math.round(Math.min(Math.max(width, SIDE_MIN_VP), window - SIDE_MIN_VP));
  }

  private static dockedWidth(): number {
    return AsideRegionModel.clampWidth(AsideRegionModel.width * AsideRegionModel.ratio);
  }

  /** A drag puts the divider at [x] vp from the window's left; it snaps to the middle near it. */
  static dragDividerTo(x: number): void {
    const window = AsideRegionModel.width;
    if (window <= 0) {
      return;
    }
    let width = AsideRegionModel.clampWidth(window - x);
    if (Math.abs(width - window / 2) < SNAP_VP) {
      width = Math.round(window / 2);
    }
    AsideRegionModel.ratio = width / window;
    AsideRegionModel.publish();
  }

  static ratioValue(): number {
    return AsideRegionModel.ratio;
  }

  static resetRatio(): void {
    AsideRegionModel.ratio = DEFAULT_RATIO;
    AsideRegionModel.publish();
  }

  static close(id: string): void {
    AsideRegionModel.children.get(id)?.close();
  }

  static collapse(): void {
    if (AsideRegionModel.slot?.maximized === true) {
      AsideRegionModel.command('unmaximize', 'panel');
    }
    AsideRegionModel.command('collapse', 'panel');
  }

  static restore(): void {
    AsideRegionModel.command('restore', 'panel');
  }

  static focus(id: string): void {
    AsideRegionModel.command('focus', id);
  }

  static toggleMaximized(): void {
    AsideRegionModel.command(AsideRegionModel.slot?.maximized === true ? 'unmaximize' : 'maximize', 'panel');
  }

  /** The window's graph, which any of its lxapps reaches. */
  private static graphAppId(): string {
    let graph = '';
    try {
      graph = getCurrentLxapp()?.appid ?? '';
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `no current lxapp: ${error}`);
    }
    if (!graph) {
      AsideRegionModel.children.forEach((child: AsideChild) => {
        if (!graph) {
          graph = child.ownerAppId;
        }
      });
    }
    return graph;
  }

  private static command(name: string, arg: string): void {
    const graph = AsideRegionModel.graphAppId();
    if (!graph) {
      return;
    }
    if (!asideCommand(graph, name, arg)) {
      hilog.warn(DOMAIN, TAG, `aside command ${name}(${arg}) refused for ${graph}`);
    }
  }

  private static titleOf(id: string): string {
    const child = AsideRegionModel.children.get(id);
    if (child) {
      return child.title();
    }
    const info: LxAppInfo | null = getLxAppInfo(id);
    const name: string = info ? info.appName : '';
    return name.length > 0 ? name : id;
  }

  private static publish(): void {
    const state = emptyState();
    const slot = AsideRegionModel.slot;
    const width = AsideRegionModel.width;
    const height = AsideRegionModel.height;
    const shown = AsideRegionModel.showing();
    const over = AsideRegionModel.isOverlay();
    state.windowWidth = width;
    state.height = height;
    state.dark = AsideRegionModel.dark;
    state.shown = shown;
    state.overlay = over;
    state.stripTop = AsideRegionModel.statusTop;
    state.contentTop = AsideRegionModel.statusTop + STRIP_VP;
    state.active = slot?.activeChild ?? '';
    state.maximized = slot?.maximized === true;
    state.maximizeVisible = (AsideRegionModel.canDock() && slot?.overlay !== true) || state.maximized;
    state.canResize = AsideRegionModel.canResize();
    if (shown) {
      state.panelWidth = over ? width : AsideRegionModel.dockedWidth();
      state.panelX = width - state.panelWidth;
      state.dockWidth = over ? 0 : state.panelWidth;
    }
    const ids = (slot?.children ?? []).filter((id: string) => AsideRegionModel.children.has(id));
    state.tabs = ids.map((id: string): AsideTab => {
      return { id: id, title: AsideRegionModel.titleOf(id), isUrl: AsideRegionModel.children.get(id)?.isUrl === true };
    });
    state.tongueShown = !shown && !!slot?.activeChild && (slot?.collapsed === true || slot?.visible === false);
    state.tongueCount = ids.length;
    if (state.tongueShown) {
      const title = AsideRegionModel.titleOf(slot?.activeChild ?? '');
      state.tongueLabel = AsideRegionModel.showContentFormat.replace('%1$s', title);
      const graph = AsideRegionModel.graphAppId();
      const tabBar: TabBarState | null = graph ? getTabBar(graph) : null;
      const selected: number | undefined = tabBar?.selectedColor;
      if (selected !== undefined) {
        state.accent = argbCss(selected);
      }
    }
    AppStorage.setOrCreate<AsideState>(STATE_KEY, state);
  }
}

/** The region's chrome: the panel's backing and strip, the docked divider, and the tongue. */
@Component
export struct AsideRegion {
  @StorageProp('lxAsideState') state: AsideState = emptyState();
  /** The divider is its own layer above the panel's content, so its whole grip takes a drag. */
  @Prop dividerOnly: boolean = false;
  private dividerDragging: boolean = false;
  private areaWidth: number = 0;
  private areaHeight: number = 0;
  private areaTop: number = 0;
  /** The status bar and cutout inset at the window's top, in vp. */
  private systemTop: number = 0;

  /** The strip starts below the status bar where the region reaches under it. */
  private publishWindow(): void {
    const statusTop = Math.max(0, this.systemTop - this.areaTop);
    AsideRegionModel.setWindow(this.areaWidth, this.areaHeight, statusTop);
  }

  private readSystemTop(): void {
    window.getLastWindow(getContext(this)).then((win: window.Window) => {
      const system = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      const cutout = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_CUTOUT);
      this.systemTop = Math.round(px2vp(Math.max(system.topRect.height, cutout.topRect.height)));
      this.publishWindow();
    }).catch((error: Error) => {
      hilog.warn(DOMAIN, TAG, `no window avoid area: ${error}`);
    });
  }

  aboutToAppear(): void {
    if (this.dividerOnly) {
      return;
    }
    this.readSystemTop();
    PersistentStorage.persistProp(RATIO_KEY, DEFAULT_RATIO);
    AsideRegionModel.restoreRatio(AppStorage.get<number>(RATIO_KEY));
    try {
      AsideRegionModel.setShowContentFormat(
        getContext(this).resourceManager.getStringByNameSync('lx_aside_show_content'));
    } catch (error) {
      hilog.warn(DOMAIN, TAG, `missing lx_aside_show_content: ${error}`);
    }
  }

  private label(name: string): string {
    try {
      return getContext(this).resourceManager.getStringByNameSync(name);
    } catch (error) {
      return name;
    }
  }

  private saveRatio(): void {
    AppStorage.setOrCreate<number>(RATIO_KEY, AsideRegionModel.ratioValue());
  }

  build() {
    Stack() {
      if (this.dividerOnly) {
        if (this.state.shown && !this.state.overlay) {
          this.Divider()
        }
      } else {
        if (this.state.shown) {
          this.Panel()
        }
        if (this.state.tongueShown) {
          this.Tongue()
        }
      }
    }
    .width('100%')
    .height('100%')
    .alignContent(Alignment.TopStart)
    .hitTestBehavior(HitTestMode.Transparent)
    .onAreaChange((_: Area, area: Area) => {
      if (this.dividerOnly) {
        return;
      }
      this.areaWidth = Number(area.width) || 0;
      this.areaHeight = Number(area.height) || 0;
      this.areaTop = Math.max(0, Math.round(Number(area.globalPosition?.y ?? 0)));
      this.publishWindow();
      // A rotation or fold moves the status bar.
      this.readSystemTop();
    })
  }

  @Builder
  Panel() {
    Column() {
      Row() {
        if (this.state.tabs.length === 1) {
          this.HeaderTab(this.state.tabs[0])
        } else {
          Scroll() {
            Row({ space: 4 }) {
              ForEach(this.state.tabs, (tab: AsideTab) => {
                this.TabChip(tab)
              }, (tab: AsideTab) => `${tab.id}:${tab.title}:${tab.id === this.state.active}`)
            }
            .height('100%')
            .alignItems(VerticalAlign.Center)
          }
          .scrollable(ScrollDirection.Horizontal)
          .scrollBar(BarState.Off)
          .layoutWeight(1)
          .height('100%')
        }
        if (this.state.maximizeVisible) {
          AsideGlyph({ kind: this.state.maximized ? 'restore' : 'maximize', dark: this.state.dark })
            .width(CONTROL_VP)
            .height(CONTROL_VP)
            .accessibilityText(this.label(this.state.maximized ? 'lx_aside_restore' : 'lx_aside_maximize'))
            .onClick(() => AsideRegionModel.toggleMaximized())
        }
        AsideGlyph({ kind: 'collapse', dark: this.state.dark })
          .width(CONTROL_VP)
          .height(CONTROL_VP)
          .accessibilityText(this.label('lx_aside_collapse'))
          .onClick(() => AsideRegionModel.collapse())
      }
      .width('100%')
      .height(STRIP_VP)
      .padding({ left: 8, right: 6 })
      .alignItems(VerticalAlign.Center)
      .border({ width: { bottom: 0.5 }, color: { bottom: asidePalette(this.state.dark).line } })
    }
    .width(this.state.panelWidth)
    .height('100%')
    .padding({ top: this.state.stripTop })
    .backgroundColor(asidePalette(this.state.dark).panel)
    .position({ x: this.state.panelX, y: 0 })
    .shadow(this.state.overlay ? { radius: 24, color: '#2E000000', offsetX: 0, offsetY: 0 } : { radius: 0, color: Color.Transparent })
    .hitTestBehavior(HitTestMode.Block)
  }

  @Builder
  HeaderTab(tab: AsideTab) {
    Row({ space: 6 }) {
      if (tab.isUrl) {
        AsideGlyph({ kind: 'globe', dark: this.state.dark }).width(16).height(16)
      }
      Text(tab.title)
        .fontSize(15)
        .fontWeight(FontWeight.Medium)
        .fontColor(asidePalette(this.state.dark).title)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .layoutWeight(1)
      this.CloseButton(tab.id, true)
    }
    .layoutWeight(1)
    .height(TAB_VP)
    .alignItems(VerticalAlign.Center)
  }

  @Builder
  TabChip(tab: AsideTab) {
    Row({ space: 6 }) {
      if (tab.isUrl) {
        AsideGlyph({ kind: 'globe', dark: this.state.dark }).width(15).height(15)
      }
      Text(tab.title)
        .fontSize(13)
        .fontWeight(tab.id === this.state.active ? FontWeight.Medium : FontWeight.Normal)
        .fontColor(tab.id === this.state.active ? asidePalette(this.state.dark).title : asidePalette(this.state.dark).muted)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .constraintSize({ maxWidth: 120 })
      this.CloseButton(tab.id, tab.id === this.state.active)
    }
    .height(TAB_VP)
    .padding({ left: 10 })
    .borderRadius(8)
    .backgroundColor(tab.id === this.state.active ? asidePalette(this.state.dark).chip : Color.Transparent)
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      if (tab.id !== this.state.active) {
        AsideRegionModel.focus(tab.id);
      }
    })
  }

  @Builder
  CloseButton(id: string, selected: boolean) {
    Image($rawfile('icons/icon_close_x.svg'))
      .width(12)
      .height(12)
      .fillColor(asidePalette(this.state.dark).muted)
      .padding(8)
      .width(28)
      .height(28)
      .opacity(selected ? 1 : 0.65)
      .accessibilityText(this.label('lx_common_close'))
      .onClick(() => AsideRegionModel.close(id))
  }

  @Builder
  Divider() {
    AsideDivider({ resizable: this.state.canResize, dark: this.state.dark })
      .width(DIVIDER_HIT_VP)
      .height(Math.max(0, this.state.height - this.state.stripTop))
      .position({ x: this.state.panelX - DIVIDER_HIT_VP / 2, y: this.state.stripTop })
      .hitTestBehavior(this.state.canResize ? HitTestMode.Block : HitTestMode.Transparent)
      .onTouch((event: TouchEvent) => {
        if (!this.state.canResize || !event.touches || event.touches.length === 0) {
          return;
        }
        const x = event.touches[0].windowX;
        if (event.type === TouchType.Down) {
          this.dividerDragging = true;
        } else if (event.type === TouchType.Move && this.dividerDragging) {
          AsideRegionModel.dragDividerTo(x);
        } else if (event.type === TouchType.Up || event.type === TouchType.Cancel) {
          this.dividerDragging = false;
          this.saveRatio();
        }
      })
      .gesture(TapGesture({ count: 2 }).onAction(() => {
        AsideRegionModel.resetRatio();
        this.saveRatio();
      }))
  }

  @Builder
  Tongue() {
    AsideTongue({ count: this.state.tongueCount, accent: this.state.accent, dark: this.state.dark })
      .width(this.state.tongueCount > 1 ? TONGUE_HIT_W_COUNT_VP : TONGUE_HIT_W_VP)
      .height(TONGUE_HIT_H_VP)
      .position({
        x: this.state.windowWidth - (this.state.tongueCount > 1 ? TONGUE_HIT_W_COUNT_VP : TONGUE_HIT_W_VP),
        y: (this.state.height - TONGUE_HIT_H_VP) / 2
      })
      .accessibilityText(this.state.tongueLabel)
      .hitTestBehavior(HitTestMode.Block)
      .onClick(() => AsideRegionModel.restore())
      .gesture(PanGesture({ direction: PanDirection.Left, distance: PULL_VP }).onActionStart(() => {
        AsideRegionModel.restore();
      }))
  }
}

/** The strip's small glyphs, drawn so the panel needs no bundled icons beyond close. */
@Component
struct AsideGlyph {
  @Prop @Watch('redraw') kind: string = 'collapse';
  @Prop @Watch('redraw') dark: boolean = false;
  private settings: RenderingContextSettings = new RenderingContextSettings(true);
  private ctx: CanvasRenderingContext2D = new CanvasRenderingContext2D(this.settings);

  redraw(): void {
    this.draw();
  }

  private draw(): void {
    const ctx = this.ctx;
    const w = ctx.width;
    const h = ctx.height;
    if (!w || !h) {
      return;
    }
    ctx.clearRect(0, 0, w, h);
    ctx.strokeStyle = asidePalette(this.dark).muted;
    ctx.lineWidth = 1.5;
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    const cx = w / 2;
    const cy = h / 2;
    const s = Math.min(w, h) >= 30 ? 7 : Math.min(w, h) / 2 - 1;
    ctx.beginPath();
    if (this.kind === 'maximize') {
      // Arrows out to two corners.
      ctx.moveTo(cx - 2, cy - 2);
      ctx.lineTo(cx - s, cy - s);
      ctx.moveTo(cx - s, cy - s + 4);
      ctx.lineTo(cx - s, cy - s);
      ctx.lineTo(cx - s + 4, cy - s);
      ctx.moveTo(cx + 2, cy + 2);
      ctx.lineTo(cx + s, cy + s);
      ctx.moveTo(cx + s, cy + s - 4);
      ctx.lineTo(cx + s, cy + s);
      ctx.lineTo(cx + s - 4, cy + s);
    } else if (this.kind === 'restore') {
      // Arrows in from two corners.
      ctx.moveTo(cx - s, cy - s);
      ctx.lineTo(cx - 2, cy - 2);
      ctx.moveTo(cx - 2, cy - 6);
      ctx.lineTo(cx - 2, cy - 2);
      ctx.lineTo(cx - 6, cy - 2);
      ctx.moveTo(cx + s, cy + s);
      ctx.lineTo(cx + 2, cy + 2);
      ctx.moveTo(cx + 2, cy + 6);
      ctx.lineTo(cx + 2, cy + 2);
      ctx.lineTo(cx + 6, cy + 2);
    } else if (this.kind === 'globe') {
      ctx.arc(cx, cy, s, 0, Math.PI * 2);
      ctx.moveTo(cx - s, cy);
      ctx.lineTo(cx + s, cy);
      ctx.moveTo(cx, cy - s);
      ctx.bezierCurveTo(cx + s * 0.7, cy - s * 0.5, cx + s * 0.7, cy + s * 0.5, cx, cy + s);
      ctx.bezierCurveTo(cx - s * 0.7, cy + s * 0.5, cx - s * 0.7, cy - s * 0.5, cx, cy - s);
    } else {
      // A panel outline with its trailing pane: the slot's collapse.
      ctx.rect(cx - s, cy - s + 1, s * 2, s * 2 - 2);
      ctx.moveTo(cx + s * 0.3, cy - s + 1);
      ctx.lineTo(cx + s * 0.3, cy + s - 1);
    }
    ctx.stroke();
  }

  build() {
    Canvas(this.ctx)
      .width('100%')
      .height('100%')
      .onReady(() => this.draw())
  }
}

/** The docked divider: a hairline with a grip, dragged to size the slot. */
@Component
struct AsideDivider {
  @Prop resizable: boolean = true;
  @Prop dark: boolean = false;

  build() {
    Stack() {
      Column()
        .width(0.5)
        .height('100%')
        .backgroundColor(asidePalette(this.dark).line)
      if (this.resizable) {
        Column()
          .width(4)
          .height(36)
          .borderRadius(2)
          .backgroundColor(asidePalette(this.dark).grip)
      }
    }
    .width('100%')
    .height('100%')
  }
}

/**
 * The desktop's collapsed-slot tongue: a trapezoid whose long side lies on the edge, with a
 * chevron toward the main and, from two children, their count.
 */
@Component
struct AsideTongue {
  @Prop @Watch('redraw') count: number = 0;
  @Prop @Watch('redraw') accent: string = DEFAULT_ACCENT;
  @Prop @Watch('redraw') dark: boolean = false;
  private settings: RenderingContextSettings = new RenderingContextSettings(true);
  private ctx: CanvasRenderingContext2D = new CanvasRenderingContext2D(this.settings);

  redraw(): void {
    this.draw();
  }

  private draw(): void {
    const ctx = this.ctx;
    const w = ctx.width;
    const h = ctx.height;
    if (!w || !h) {
      return;
    }
    ctx.clearRect(0, 0, w, h);
    const counted = this.count > 1;
    const outer = w;
    const inner = outer - (counted ? 18 : 14);
    const top = (h - 64) / 2;
    const bottom = top + 64;
    const slope = 10;
    ctx.beginPath();
    ctx.moveTo(outer, top);
    ctx.arcTo(inner, top + slope, inner, bottom - slope, 3);
    ctx.arcTo(inner, bottom - slope, outer, bottom, 3);
    ctx.lineTo(outer, bottom);
    ctx.closePath();
    ctx.shadowBlur = 4;
    ctx.shadowOffsetX = -1;
    ctx.shadowColor = 'rgba(0,0,0,0.16)';
    ctx.fillStyle = asidePalette(this.dark).tongue;
    ctx.fill();
    ctx.shadowBlur = 0;
    ctx.shadowOffsetX = 0;
    ctx.shadowColor = 'rgba(0,0,0,0)';
    ctx.lineWidth = 0.75;
    ctx.strokeStyle = asidePalette(this.dark).tongueEdge;
    ctx.stroke();

    const cx = (inner + outer) / 2;
    const cy = h / 2 - (counted ? 7 : 0);
    ctx.beginPath();
    ctx.moveTo(cx + 2.5, cy - 4);
    ctx.lineTo(cx - 2.5, cy);
    ctx.lineTo(cx + 2.5, cy + 4);
    ctx.lineWidth = 1.6;
    ctx.lineCap = 'round';
    ctx.lineJoin = 'round';
    ctx.strokeStyle = this.accent;
    ctx.stroke();
    if (counted) {
      ctx.fillStyle = this.accent;
      ctx.font = 'bold 11vp sans-serif';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(this.count > 9 ? '9+' : `${this.count}`, cx, h / 2 + 11);
    }
  }

  build() {
    Canvas(this.ctx)
      .width('100%')
      .height('100%')
      .onReady(() => this.draw())
  }
}
