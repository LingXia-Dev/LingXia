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
const CLOSE_VP = 28;
const GLOBE_VP = 16;
/** A control's icon is inset by its size over this, as Android's strip does. */
const ICON_INSET_DIVISOR = 4.4;
const SIDE_MIN_VP = 320;
const DEFAULT_RATIO = 0.5;
const SNAP_VP = 24;
const MIN_TRAVEL_VP = 48;
const DIVIDER_HIT_VP = 24;
const TONGUE_HIT_W_VP = 28;
const TONGUE_HIT_W_COUNT_VP = 32;
const TONGUE_HIT_H_VP = 88;
const PULL_VP = 16;
const STRIP_PAD_START_VP = 8;
const STRIP_PAD_END_VP = 6;
const CHIP_GAP_VP = 4;
/** Globe, gaps, a few glyphs of title and the close: below this a chip says nothing. */
const CHIP_MIN_VP = 96;
/** Android's widest natural chip: a 120-vp title with its globe and close. */
const CHIP_MAX_VP = 186;
/** Narrower than this, a chip that is not selected gives its close's room to the title. */
const CHIP_CLOSE_MIN_VP = 140;
const TONGUE_ICON_VP = 18;
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
  /** The panel's strip and content, clear of a side cutout or navigation bar. */
  innerX: number;
  innerWidth: number;
  /** The tongue's hit box, at the safe edge on the window's right. */
  tongueX: number;
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

// fillColor repaints only SVG fills and the design icons are strokes; a SRC_IN
// matrix recolors any art, like a template image. Takes '#AARRGGBB'.
function iconTint(argb: string): ColorFilter {
  const hex = argb.replace('#', '');
  return tintMatrix(parseInt(hex.substring(2, 4), 16) / 255, parseInt(hex.substring(4, 6), 16) / 255,
    parseInt(hex.substring(6, 8), 16) / 255, parseInt(hex.substring(0, 2), 16) / 255);
}

/** The same tint from a CSS `rgba(r,g,b,a)`, the form the accent is kept in. */
function cssTint(rgba: string): ColorFilter {
  const parts = rgba.replace(/[^0-9.,]/g, '').split(',').map((v: string) => Number(v));
  return tintMatrix(parts[0] / 255, parts[1] / 255, parts[2] / 255, parts.length > 3 ? parts[3] : 1);
}

function tintMatrix(r: number, g: number, b: number, a: number): ColorFilter {
  return new ColorFilter([
    0, 0, 0, r, 0,
    0, 0, 0, g, 0,
    0, 0, 0, b, 0,
    0, 0, 0, a, 0
  ]);
}

/** The layers the side panel draws in: its chrome and tongue, its shown child, its divider. */
export enum AsidePart { Chrome, Child, Divider }

/** A response region that takes no touch, for a full-window layer with nothing to show. */
export const NO_TOUCH: Rectangle[] = [{ x: -1, y: -1, width: 0, height: 0 }];

/** The whole window, for a layer that covers it. */
export const WHOLE_TOUCH: Rectangle[] = [{ x: 0, y: 0, width: '100%', height: '100%' }];

function emptyState(): AsideState {
  return {
    shown: false,
    overlay: false,
    dockWidth: 0,
    panelX: 0,
    panelWidth: 0,
    innerX: 0,
    innerWidth: 0,
    tongueX: 0,
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
  private static insetLeft: number = 0;
  private static insetRight: number = 0;
  private static ratio: number = DEFAULT_RATIO;
  private static showContentFormat: string = 'Show %1$s';

  static emptyState(): AsideState {
    return emptyState();
  }

  /**
   * Where a part's full-window layer takes touches. ArkUI wraps a mounted component in a node
   * of its size that blocks every layer under it, so the layer must claim only what it draws.
   */
  static touchArea(state: AsideState, part: AsidePart): Rectangle[] {
    if (part === AsidePart.Divider) {
      if (!state.shown || state.overlay || !state.canResize) {
        return NO_TOUCH;
      }
      return [{
        x: state.panelX - DIVIDER_HIT_VP / 2, y: state.stripTop,
        width: DIVIDER_HIT_VP, height: Math.max(0, state.height - state.stripTop)
      }];
    }
    if (part === AsidePart.Child) {
      if (!state.shown) {
        return NO_TOUCH;
      }
      return [{
        x: state.innerX, y: state.contentTop,
        width: state.innerWidth, height: Math.max(0, state.height - state.contentTop)
      }];
    }
    if (state.shown) {
      return [{ x: state.panelX, y: 0, width: state.panelWidth, height: state.height }];
    }
    if (state.tongueShown) {
      const width = state.tongueCount > 1 ? TONGUE_HIT_W_COUNT_VP : TONGUE_HIT_W_VP;
      return [{
        x: state.tongueX, y: (state.height - TONGUE_HIT_H_VP) / 2,
        width: width, height: TONGUE_HIT_H_VP
      }];
    }
    return NO_TOUCH;
  }

  /** The window's size in vp, its status bar inset, and its side insets in landscape. */
  static setWindow(width: number, height: number, statusTop: number, insetLeft: number, insetRight: number): void {
    if (width === AsideRegionModel.width && height === AsideRegionModel.height &&
      statusTop === AsideRegionModel.statusTop && insetLeft === AsideRegionModel.insetLeft &&
      insetRight === AsideRegionModel.insetRight) {
      return;
    }
    AsideRegionModel.width = width;
    AsideRegionModel.height = height;
    AsideRegionModel.statusTop = statusTop;
    AsideRegionModel.insetLeft = insetLeft;
    AsideRegionModel.insetRight = insetRight;
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
      const leading = over ? AsideRegionModel.insetLeft : 0;
      state.innerX = state.panelX + leading;
      state.innerWidth = Math.max(0, state.panelWidth - leading - AsideRegionModel.insetRight);
    }
    const ids = (slot?.children ?? []).filter((id: string) => AsideRegionModel.children.has(id));
    state.tabs = ids.map((id: string): AsideTab => {
      return { id: id, title: AsideRegionModel.titleOf(id), isUrl: AsideRegionModel.children.get(id)?.isUrl === true };
    });
    state.tongueShown = !shown && !!slot?.activeChild && (slot?.collapsed === true || slot?.visible === false);
    state.tongueCount = ids.length;
    state.tongueX = width - AsideRegionModel.insetRight -
      (state.tongueCount > 1 ? TONGUE_HIT_W_COUNT_VP : TONGUE_HIT_W_VP);
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
  @StorageProp('lxAsideState') @Watch('revealActive') state: AsideState = emptyState();
  /** The divider is its own layer above the panel's content, so its whole grip takes a drag. */
  @Prop dividerOnly: boolean = false;
  private dividerDragging: boolean = false;
  private chipScroller: Scroller = new Scroller();
  private revealed: string = '';
  private areaWidth: number = 0;
  private areaHeight: number = 0;
  private areaTop: number = 0;
  /** The status bar and cutout inset at the window's top, in vp. */
  private systemTop: number = 0;
  /** The cutout or navigation bar on each side in landscape, in vp. */
  private systemLeft: number = 0;
  private systemRight: number = 0;

  /** The strip starts below the status bar where the region reaches under it. */
  private publishWindow(): void {
    const statusTop = Math.max(0, this.systemTop - this.areaTop);
    AsideRegionModel.setWindow(this.areaWidth, this.areaHeight, statusTop, this.systemLeft, this.systemRight);
  }

  private readSystemTop(): void {
    window.getLastWindow(getContext(this)).then((win: window.Window) => {
      const system = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_SYSTEM);
      const cutout = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_CUTOUT);
      const navigation = win.getWindowAvoidArea(window.AvoidAreaType.TYPE_NAVIGATION_INDICATOR);
      this.systemTop = Math.round(px2vp(Math.max(system.topRect.height, cutout.topRect.height)));
      this.systemLeft = Math.round(px2vp(Math.max(system.leftRect.width, cutout.leftRect.width,
        navigation.leftRect.width)));
      this.systemRight = Math.round(px2vp(Math.max(system.rightRect.width, cutout.rightRect.width,
        navigation.rightRect.width)));
      this.publishWindow();
    }).catch((error: Error) => {
      hilog.warn(DOMAIN, TAG, `no window avoid area: ${error}`);
    });
  }

  /** The strip's width left of its controls, which the chips share. */
  private chipRoom(): number {
    const controls = CONTROL_VP * (this.state.maximizeVisible ? 2 : 1);
    return this.state.innerWidth - STRIP_PAD_START_VP - STRIP_PAD_END_VP - controls;
  }

  /** How many whole chips the room holds at the minimum width. */
  private chipsInView(): number {
    return Math.max(1, Math.floor((this.chipRoom() + CHIP_GAP_VP) / (CHIP_MIN_VP + CHIP_GAP_VP)));
  }

  /**
   * An equal share of the room up to Android's natural width. When the chips do not fit, they
   * are sized so a whole number fills the room, and scrolling snaps to them: no chip is cut.
   */
  private chipWidth(): number {
    const count = Math.max(1, this.state.tabs.length);
    const shown = Math.min(count, this.chipsInView());
    const share = (this.chipRoom() - CHIP_GAP_VP * (shown - 1)) / shown;
    return count > shown ? share : Math.min(CHIP_MAX_VP, share);
  }

  /** When the chips scroll, keeps the selected one, and so its close, in view. */
  revealActive(): void {
    const count = this.state.tabs.length;
    const index = this.state.tabs.findIndex((tab: AsideTab) => tab.id === this.state.active);
    const key = `${this.state.active}:${count}:${this.state.panelWidth}`;
    if (count < 2 || index < 0 || key === this.revealed) {
      return;
    }
    this.revealed = key;
    const shown = this.chipsInView();
    const first = Math.max(0, Math.min(count - shown, index - Math.floor(shown / 2)));
    this.chipScroller.scrollTo({ xOffset: first * (this.chipWidth() + CHIP_GAP_VP), yOffset: 0 });
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
          Scroll(this.chipScroller) {
            Row({ space: CHIP_GAP_VP }) {
              ForEach(this.state.tabs, (tab: AsideTab) => {
                this.TabChip(tab)
              }, (tab: AsideTab) => `${tab.id}:${tab.title}:${tab.id === this.state.active}`)
            }
            .height('100%')
            .alignItems(VerticalAlign.Center)
          }
          .scrollable(ScrollDirection.Horizontal)
          .scrollBar(BarState.Off)
          .scrollSnap({ snapAlign: ScrollSnapAlign.START, snapPagination: this.chipWidth() + CHIP_GAP_VP })
          .layoutWeight(1)
          .height('100%')
          .onAppear(() => {
            this.revealed = '';
            setTimeout(() => this.revealActive(), 0);
          })
        }
        if (this.state.maximizeVisible) {
          this.ControlIcon(
            this.state.maximized ? $rawfile('icons/icon_fullscreen_exit.svg') : $rawfile('icons/icon_fullscreen_enter.svg'),
            CONTROL_VP, 1, this.label(this.state.maximized ? 'lx_aside_restore' : 'lx_aside_maximize'),
            () => AsideRegionModel.toggleMaximized())
        }
        // The panel stays at the right edge in either layout direction, so its arrows never flip.
        this.ControlIcon($rawfile('icons/icon_aside_collapse.svg'), CONTROL_VP, 1,
          this.label('lx_aside_collapse'), () => AsideRegionModel.collapse())
      }
      .width('100%')
      .height(STRIP_VP)
      .padding({
        left: STRIP_PAD_START_VP + this.state.innerX - this.state.panelX,
        right: STRIP_PAD_END_VP + this.state.panelX + this.state.panelWidth - this.state.innerX - this.state.innerWidth
      })
      .alignItems(VerticalAlign.Center)
      .border({ width: { bottom: 0.5 }, color: { bottom: asidePalette(this.state.dark).line } })
    }
    .width(this.state.panelWidth)
    .height('100%')
    .padding({ top: this.state.stripTop })
    .backgroundColor(asidePalette(this.state.dark).panel)
    .position({ x: this.state.panelX, y: 0 })
    .shadow(this.state.overlay ? { radius: 24, color: '#2E000000', offsetX: 0, offsetY: 0 } : { radius: 0, color: Color.Transparent })
    // Default, not Block: Block also keeps the touch from the strip's own buttons.
    .hitTestBehavior(HitTestMode.Default)
  }

  @Builder
  HeaderTab(tab: AsideTab) {
    Row({ space: 6 }) {
      if (tab.isUrl) {
        this.Globe()
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
        this.Globe()
      }
      Text(tab.title)
        .fontSize(13)
        .fontWeight(tab.id === this.state.active ? FontWeight.Medium : FontWeight.Normal)
        .fontColor(tab.id === this.state.active ? asidePalette(this.state.dark).title : asidePalette(this.state.dark).muted)
        .maxLines(1)
        .textOverflow({ overflow: TextOverflow.Ellipsis })
        .layoutWeight(1)
      if (tab.id === this.state.active || this.chipWidth() >= CHIP_CLOSE_MIN_VP) {
        this.CloseButton(tab.id, tab.id === this.state.active)
      }
    }
    .width(this.chipWidth())
    .padding({ left: 10, right: tab.id === this.state.active || this.chipWidth() >= CHIP_CLOSE_MIN_VP ? 0 : 10 })
    .height(TAB_VP)
    .borderRadius(8)
    .backgroundColor(tab.id === this.state.active ? asidePalette(this.state.dark).chip : Color.Transparent)
    .alignItems(VerticalAlign.Center)
    .onClick(() => {
      if (tab.id !== this.state.active) {
        AsideRegionModel.focus(tab.id);
      }
    })
  }

  /** A strip control drawn from the design icon set, at Android's size and inset. */
  @Builder
  ControlIcon(src: Resource, size: number, opacity: number, label: string, action: () => void) {
    Image(src)
      .width(size)
      .height(size)
      .padding(size / ICON_INSET_DIVISOR)
      .objectFit(ImageFit.Contain)
      .colorFilter(iconTint(asidePalette(this.state.dark).muted))
      .opacity(opacity)
      .accessibilityText(label)
      .onClick(action)
  }

  @Builder
  Globe() {
    Image($rawfile('icons/icon_globe.svg'))
      .width(GLOBE_VP)
      .height(GLOBE_VP)
      .objectFit(ImageFit.Contain)
      .colorFilter(iconTint(asidePalette(this.state.dark).muted))
  }

  @Builder
  CloseButton(id: string, selected: boolean) {
    this.ControlIcon($rawfile('icons/icon_close_x.svg'), CLOSE_VP, selected ? 1 : 0.65,
      this.label('lx_common_close'), () => AsideRegionModel.close(id))
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
        x: this.state.tongueX,
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
    if (counted) {
      ctx.fillStyle = this.accent;
      ctx.font = 'bold 11vp sans-serif';
      ctx.textAlign = 'center';
      ctx.textBaseline = 'middle';
      ctx.fillText(this.count > 9 ? '9+' : `${this.count}`, cx, h / 2 + 11);
    }
  }

  build() {
    Stack({ alignContent: Alignment.TopStart }) {
      Canvas(this.ctx)
        .width('100%')
        .height('100%')
        .onReady(() => this.draw())
      // Centred on the tongue's face; with a count it rises to leave the number room.
      Image($rawfile('icons/icon_aside_expand.svg'))
        .width(TONGUE_ICON_VP)
        .height(TONGUE_ICON_VP)
        .objectFit(ImageFit.Contain)
        .colorFilter(cssTint(this.accent))
        .position({
          x: this.faceCenter() - TONGUE_ICON_VP / 2,
          y: TONGUE_HIT_H_VP / 2 - (this.count > 1 ? 7 : 0) - TONGUE_ICON_VP / 2
        })
    }
    .width('100%')
    .height('100%')
  }

  /** The middle of the tongue's face, between its slanted inner side and the window edge. */
  private faceCenter(): number {
    const counted = this.count > 1;
    const outer = counted ? TONGUE_HIT_W_COUNT_VP : TONGUE_HIT_W_VP;
    return (outer + outer - (counted ? 18 : 14)) / 2;
  }
}
